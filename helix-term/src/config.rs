use crate::keymap;
use crate::keymap::{merge_keys, KeyTrie};
use helix_loader::merge_toml_values;
use helix_view::document::Mode;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
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

impl ConfigRaw {
    fn load(file: PathBuf) -> Result<Self, ConfigLoadError> {
        let source = fs::read_to_string(file).map_err(ConfigLoadError::Error)?;
        toml::from_str(&source).map_err(ConfigLoadError::BadConfig)
    }
}

impl TryFrom<ConfigRaw> for Config {
    type Error = ConfigLoadError;

    fn try_from(config: ConfigRaw) -> Result<Self, Self::Error> {
        Ok(Self {
            workspace_config: config.workspace_config.unwrap_or_default(),
            theme: config.theme,
            keys: match config.keys {
                Some(keys) => merge_keys(keymap::default(), keys),
                None => keymap::default(),
            },
            editor: config.editor
                .map(|e| e.try_into()).transpose()
                .map_err(ConfigLoadError::BadConfig)?
                .unwrap_or_default(),
        })
    }
}

impl Config {
    pub fn load() -> Result<Config, ConfigLoadError> {
        // Load and parse global config returning all errors
        let global: Config = ConfigRaw::load(helix_loader::config_file())?.try_into()?;

        if global.workspace_config {
            global.merge(ConfigRaw::load(helix_loader::workspace_config_file())?)
        } else {
            Ok(global)
        }
    }

    fn merge(self, other: ConfigRaw) -> Result<Self, ConfigLoadError> {
        Ok(Config {
            workspace_config: other.workspace_config.unwrap_or(self.workspace_config),
            theme: other.theme.or(self.theme),
            keys: match other.keys {
                Some(keys) => merge_keys(self.keys, keys),
                None => self.keys,
            },
            editor: match other.editor {
                None => self.editor,
                Some(editor) => merge_toml_values(
                    toml::Value::try_from(self.editor).unwrap(),
                    editor, 3
                ).try_into().map_err(ConfigLoadError::BadConfig)?,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Config {
        fn load_test(file: &str) -> Config {
            let raw: ConfigRaw = toml::from_str(file).unwrap();
            raw.try_into().unwrap()
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

        let keys = merge_keys(
            keymap::default(),
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
