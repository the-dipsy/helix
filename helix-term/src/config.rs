use crate::keymap;
use crate::keymap::{merge_keys, KeyTrie};
use helix_loader::merge_toml_values;
use helix_view::document::Mode;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::io::Error as IOError;
use toml::de::Error as TomlError;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub workspace_config: bool,
    pub theme: Option<String>,
    pub keys: HashMap<Mode, KeyTrie>,
    pub editor: helix_view::editor::Config,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ConfigRaw {
    pub workspace_config: Option<bool>,
    pub theme: Option<String>,
    pub keys: Option<HashMap<Mode, KeyTrie>>,
    pub editor: Option<toml::Value>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            workspace_config: false,
            theme: None,
            keys: keymap::default(),
            editor: helix_view::editor::Config::default(),
        }
    }
}

#[derive(Debug)]
pub enum ConfigLoadError {
    BadConfig(TomlError),
    Error(IOError),
}

impl Default for ConfigLoadError {
    fn default() -> Self {
        ConfigLoadError::Error(IOError::new(std::io::ErrorKind::NotFound, "place holder"))
    }
}

impl Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLoadError::BadConfig(err) => err.fmt(f),
            ConfigLoadError::Error(err) => err.fmt(f),
        }
    }
}

impl Config {
    pub fn load(
        mut global: ConfigRaw, mut workspace: Option<ConfigRaw>,
    ) -> Result<Config, ConfigLoadError> {
        // Create merged keymap
        let mut keys = keymap::default();
        [Some(&mut global), workspace.as_mut()].into_iter()
            .flatten().filter_map(|c| c.keys.take())
            .for_each(|k| merge_keys(&mut keys, k));

        // Create config
        let config = Config {
            workspace_config: global.workspace_config.unwrap_or(false),
            theme: workspace.as_mut().and_then(|c| c.theme.take()).or(global.theme),
            keys,
            editor: match (global.editor, workspace.and_then(|c| c.editor)) {
                (None, None) => Ok(helix_view::editor::Config::default()),
                (None, Some(editor)) | (Some(editor), None) => editor.try_into(),
                (Some(glob), Some(work)) => merge_toml_values(glob, work, 3).try_into(),
            }.map_err(ConfigLoadError::BadConfig)?,
        };

        Ok(config)
    }

    pub fn load_default() -> Result<Config, ConfigLoadError> {
        // Load and parse global config returning all errors
        let global: ConfigRaw = fs::read_to_string(helix_loader::config_file())
            .map_err(ConfigLoadError::Error)
            .and_then(|c| toml::from_str(&c).map_err(ConfigLoadError::BadConfig))?;

        // Load and parse workspace config if enabled ignoring IO errors
        let workspace = global.workspace_config.unwrap_or(false)
            .then(|| helix_loader::workspace_config_file())
            .and_then(|f| fs::read_to_string(f).ok())
            .map(|c| toml::from_str(&c).map_err(ConfigLoadError::BadConfig))
            .transpose()?;

        // Create merged config
        Config::load(global, workspace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Config {
        fn load_test(file: &str) -> Config {
            Config::load(toml::from_str(file).unwrap(), None).unwrap()
        }
    }

    #[test]
    fn parsing_keymaps_config_file() {
        use crate::keymap;
        use helix_core::hashmap;
        use helix_view::document::Mode;

        let sample_keymaps = r#"
            [keys.insert]
            y = "move_line_down"
            S-C-a = "delete_selection"

            [keys.normal]
            A-F12 = "move_next_word_end"
        "#;

        let mut keys = keymap::default();
        merge_keys(
            &mut keys,
            hashmap! {
                Mode::Insert => keymap!({ "Insert mode"
                    "y" => move_line_down,
                    "S-C-a" => delete_selection,
                }),
                Mode::Normal => keymap!({ "Normal mode"
                    "A-F12" => move_next_word_end,
                }),
            },
        );

        assert_eq!(
            Config::load_test(sample_keymaps),
            Config {
                keys,
                ..Default::default()
            }
        );
    }

    #[test]
    fn keys_resolve_to_correct_defaults() {
        // From serde default
        let default_keys = Config::load_test("").keys;
        assert_eq!(default_keys, keymap::default());

        // From the Default trait
        let default_keys = Config::default().keys;
        assert_eq!(default_keys, keymap::default());
    }
}
