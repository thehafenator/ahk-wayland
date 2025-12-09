pub mod application;
pub mod device;
mod key;
pub mod key_press;
pub mod keymap;
pub mod keymap_action;
mod modmap;
pub mod modmap_action;

pub mod remap;
#[cfg(test)]
mod tests;

extern crate serde_yaml;
extern crate toml;

use evdev::KeyCode as Key;
use keymap::Keymap;
use modmap::Modmap;
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use serde::{de::IgnoredAny, Deserialize, Deserializer};
use std::{
    collections::HashMap,
    error, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::ahk::{AhkAction, parse_ahk_file};
use self::{
    key::parse_key,
    keymap::{build_keymap_table, KeymapEntry},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    // Config interface
    #[serde(default = "Vec::new")]
    pub modmap: Vec<Modmap>,
    #[serde(default = "Vec::new")]
    pub keymap: Vec<Keymap>,
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(deserialize_with = "deserialize_virtual_modifiers", default = "Vec::new")]
    pub virtual_modifiers: Vec<Key>,
    #[serde(default)]
    pub keypress_delay_ms: u64,

    // Data is not used by any part of the application.
    // but can be used with Anchors and Aliases
    #[allow(dead_code)]
    #[serde(default)]
    pub shared: IgnoredAny,

    // Internals
    #[serde(skip)]
    pub modify_time: Option<SystemTime>,
    #[serde(skip)]
    pub keymap_table: HashMap<Key, Vec<KeymapEntry>>,
    #[serde(default = "const_true")]
    pub enable_wheel: bool,
}

impl Config {
    pub fn new() -> Self {
        Config {
            modmap: Vec::new(),
            keymap: Vec::new(),
            default_mode: "default".to_string(),
            virtual_modifiers: Vec::new(),
            keypress_delay_ms: 0,
            shared: IgnoredAny,
            modify_time: None,
            keymap_table: HashMap::new(),
            enable_wheel: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

enum ConfigFiletype {
    Yaml,
    Toml,
    Ahk,
}

fn get_file_ext(filename: &Path) -> ConfigFiletype {
    match filename.extension() {
        Some(f) => {
            let ext = f.to_str().unwrap_or("").to_lowercase();
            if ext == "toml" {
                ConfigFiletype::Toml
            } else if ext == "ahk" {
                ConfigFiletype::Ahk
            } else {
                ConfigFiletype::Yaml
            }
        }
        _ => ConfigFiletype::Yaml,
    }
}

pub fn load_configs(filenames: &[PathBuf]) -> Result<Config, Box<dyn error::Error>> {
    
    // Assumes filenames is non-empty
    let config_contents = fs::read_to_string(&filenames[0])?;

    let mut config: Config = match get_file_ext(&filenames[0]) {
        ConfigFiletype::Ahk => {
            let ahk_config = parse_ahk_file(&filenames[0])
                .map_err(|e| format!("AHK parse error: {}", e))?;
            
            let mut config = Config::new();
            
            // Convert AHK hotkeys to xremap keymaps
            let _hotkey_count = ahk_config.hotkeys.len();
            for hotkey in ahk_config.hotkeys {
                let mut keymap = Keymap {
                    name: String::new(),
                    remap: HashMap::new(),
                    application: None,
                    window: None,
                    device: None,
                    mode: None,
                    exact_match: false,
                };
                
                // Convert modifiers to KeyPress format
                let modifiers: Vec<key_press::Modifier> = hotkey.modifiers.iter().map(|k| {
                    match k {
                        &Key::KEY_LEFTCTRL | &Key::KEY_RIGHTCTRL => key_press::Modifier::Control,
                        &Key::KEY_LEFTALT | &Key::KEY_RIGHTALT => key_press::Modifier::Alt,
                        &Key::KEY_LEFTSHIFT | &Key::KEY_RIGHTSHIFT => key_press::Modifier::Shift,
                        &Key::KEY_LEFTMETA | &Key::KEY_RIGHTMETA => key_press::Modifier::Windows,
                        k => key_press::Modifier::Key(k.clone()),
                    }
                }).collect();
                
                let key_press = key_press::KeyPress { 
                    key: hotkey.key,
                    modifiers,
                };
                
                let actions = match hotkey.action {
                    AhkAction::Run(parts) => vec![keymap_action::KeymapAction::Launch(parts)],
                    _ => vec![],
                };
                
                keymap.remap.insert(key_press, actions);
                config.keymap.push(keymap);
            }
            
            println!("Loaded {} AHK hotkeys", _hotkey_count);
            config
        }
        ConfigFiletype::Yaml => serde_yaml::from_str(&config_contents)?,
        ConfigFiletype::Toml => toml::from_str(&config_contents)?,
    };

    for filename in &filenames[1..] {
        let config_contents = fs::read_to_string(filename)?;
        let c: Config = match get_file_ext(filename) {
            ConfigFiletype::Ahk => {
                let ahk_config = parse_ahk_file(filename)
                    .map_err(|e| format!("AHK parse error: {}", e))?;
                let mut cfg = Config::new();
                let _hotkey_count = ahk_config.hotkeys.len();
                for hotkey in ahk_config.hotkeys {
                    let mut keymap = Keymap {
                        name: String::new(),
                        remap: HashMap::new(),
                        application: None,
                        window: None,
                        device: None,
                        mode: None,
                        exact_match: false,
                    };
                    
                    let modifiers: Vec<key_press::Modifier> = hotkey.modifiers.iter().map(|k| {
                        match k {
                            &Key::KEY_LEFTCTRL | &Key::KEY_RIGHTCTRL => key_press::Modifier::Control,
                            &Key::KEY_LEFTALT | &Key::KEY_RIGHTALT => key_press::Modifier::Alt,
                            &Key::KEY_LEFTSHIFT | &Key::KEY_RIGHTSHIFT => key_press::Modifier::Shift,
                            &Key::KEY_LEFTMETA | &Key::KEY_RIGHTMETA => key_press::Modifier::Windows,
                            k => key_press::Modifier::Key(k.clone()),
                        }
                    }).collect();
                    
                    let key_press = key_press::KeyPress { 
                        key: hotkey.key,
                        modifiers,
                    };
                    
                    let actions = match hotkey.action {
                        AhkAction::Run(parts) => vec![keymap_action::KeymapAction::Launch(parts)],
                        _ => vec![],
                    };
                    
                    keymap.remap.insert(key_press, actions);
                    cfg.keymap.push(keymap);
                }
                cfg
            }
            ConfigFiletype::Yaml => serde_yaml::from_str(&config_contents)?,
            ConfigFiletype::Toml => toml::from_str(&config_contents)?,
        };

        config.modmap.extend(c.modmap);
        config.keymap.extend(c.keymap);
        config.virtual_modifiers.extend(c.virtual_modifiers);
    }

    // Timestamp for --watch=config
    config.modify_time = filenames.last().and_then(|path| path.metadata().ok()?.modified().ok());

    // Convert keymap for efficient keymap lookup
    config.keymap_table = build_keymap_table(&config.keymap);

    Ok(config)
}

pub fn config_watcher(watch: bool, files: &Vec<PathBuf>) -> anyhow::Result<Option<Inotify>> {
    if watch {
        let inotify = Inotify::init(InitFlags::IN_NONBLOCK)?;
        for file in files {
            inotify.add_watch(
                file.parent().expect("config file has a parent directory"),
                AddWatchFlags::IN_CREATE | AddWatchFlags::IN_MOVED_TO,
            )?;
            inotify.add_watch(file, AddWatchFlags::IN_MODIFY)?;
        }
        Ok(Some(inotify))
    } else {
        Ok(None)
    }
}

fn default_mode() -> String {
    "default".to_string()
}

fn deserialize_virtual_modifiers<'de, D>(deserializer: D) -> Result<Vec<Key>, D::Error>
where
    D: Deserializer<'de>,
{
    let key_names = Vec::<String>::deserialize(deserializer)?;
    key_names.into_iter().map(|name| parse_key(&name).map_err(serde::de::Error::custom)).collect()
}

fn const_true() -> bool {
    true
}
