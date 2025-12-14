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

use crate::hotstring::{HotstringMatch, HotstringMatcher};
use crate::config::keymap_action::KeymapAction;
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

use self::{
    key::parse_key,
    keymap::{build_keymap_table, KeymapEntry},
};
use crate::ahk::{parse_ahk_file, AhkAction};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub modmap: Vec<Modmap>,
    pub keymap: Vec<Keymap>,
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(deserialize_with = "deserialize_virtual_modifiers", default = "Vec::new")]
    pub virtual_modifiers: Vec<Key>,
    #[serde(default)]
    pub keypress_delay_ms: u64,
    #[allow(dead_code)]
    #[serde(default)]
    pub shared: IgnoredAny,
    #[serde(skip)]
    pub modify_time: Option<SystemTime>,
    #[serde(skip)]
    pub keymap_table: HashMap<Key, Vec<KeymapEntry>>,
    #[serde(default = "const_true")]
    pub enable_wheel: bool,
    #[serde(skip)]
    pub hotstrings: Vec<HotstringMatch>,
    #[serde(skip)]
    pub hotstring_matcher: Option<HotstringMatcher>,
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
            hotstrings: Vec::new(),
            hotstring_matcher: None,
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

fn parse_ahk_context(context: &str) -> Option<application::OnlyOrNot> {
    use regex::Regex;

    let exe_re = Regex::new(r#"WinActive\("ahk_exe\s+([^"]+)"\)"#).ok()?;
    if let Some(caps) = exe_re.captures(context) {
        let exe = caps.get(1)?.as_str().to_string();
        return Some(application::OnlyOrNot {
            only: Some(vec![application::ApplicationMatcher::Literal(exe)]),
            not: None,
        });
    }

    None
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
    key_names
        .into_iter()
        .map(|name| parse_key(&name).map_err(serde::de::Error::custom))
        .collect()
}

fn const_true() -> bool {
    true
}
fn convert_send_to_actions(send_str: &str) -> Vec<keymap_action::KeymapAction> {
    use crate::ahk::send_parser::{parse_send_string, SendToken};

    let tokens = parse_send_string(send_str);
    let mut actions = Vec::new();

    for token in tokens {
        match token {
            SendToken::Key { key, modifiers } => {
                let key_press = key_press::KeyPress {
                    key,
                    modifiers: modifiers
                        .into_iter()
                        .map(|k| match k {
                            Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => key_press::Modifier::Control,
                            Key::KEY_LEFTALT | Key::KEY_RIGHTALT => key_press::Modifier::Alt,
                            Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => key_press::Modifier::Shift,
                            Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => key_press::Modifier::Windows,
                            k => key_press::Modifier::Key(k),
                        })
                        .collect(),
                };
                actions.push(keymap_action::KeymapAction::KeyPressAndRelease(key_press));
            }
            SendToken::Text(text) => {
                // Convert each character in text to a keypress
                for ch in text.chars() {
                    if let Some(key) = char_to_evdev_key(ch) {
                        let key_press = key_press::KeyPress { key, modifiers: vec![] };
                        actions.push(keymap_action::KeymapAction::KeyPressAndRelease(key_press));
                    }
                }
            }
        }
    }

    actions
}

fn char_to_evdev_key(c: char) -> Option<Key> {
    use crate::ahk::parser::string_to_key;
    match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' => string_to_key(&c.to_lowercase().to_string()),
        ' ' => Some(Key::KEY_SPACE),
        ';' => Some(Key::KEY_SEMICOLON),
        ',' => Some(Key::KEY_COMMA),
        '.' => Some(Key::KEY_DOT),
        '/' => Some(Key::KEY_SLASH),
        '\'' => Some(Key::KEY_APOSTROPHE),
        '-' => Some(Key::KEY_MINUS),
        '=' => Some(Key::KEY_EQUAL),
        '[' => Some(Key::KEY_LEFTBRACE),
        ']' => Some(Key::KEY_RIGHTBRACE),
        '\\' => Some(Key::KEY_BACKSLASH),
        '`' => Some(Key::KEY_GRAVE),
        _ => None,
    }
}
pub fn load_configs(filenames: &[PathBuf]) -> Result<Config, Box<dyn error::Error>> {
    let config_contents = fs::read_to_string(&filenames[0])?;

    let mut config: Config = match get_file_ext(&filenames[0]) {
        ConfigFiletype::Ahk => {
            let ahk_config = parse_ahk_file(&filenames[0]).map_err(|e| format!("AHK parse error: {}", e))?;
            let extracted_hotstrings = crate::ahk::transpiler::extract_hotstrings(&ahk_config);

            let mut config = Config::new();
            let hotkey_count = ahk_config.hotkeys.len();

            // Add CapsLock as virtual modifier for AHK configs
            config.virtual_modifiers.push(Key::KEY_CAPSLOCK);

            for hotkey in ahk_config.hotkeys {
                let keymap = convert_ahk_hotkey_to_keymap(hotkey);
                if keymap.window.is_some() {
                    println!("DEBUG: Loaded hotkey with window filter: {:?}", keymap.window);
                }
                config.keymap.push(keymap);
            }

            config.hotstrings = extracted_hotstrings;
            if !config.hotstrings.is_empty() {
                config.hotstring_matcher = Some(HotstringMatcher::new(config.hotstrings.clone()));
            }

            println!("Loaded {} AHK hotkeys", hotkey_count);
            println!("Loaded {} AHK hotstrings", config.hotstrings.len());
            config
        }
        ConfigFiletype::Yaml => serde_yaml::from_str(&config_contents)?,
        ConfigFiletype::Toml => toml::from_str(&config_contents)?,
    };

    for filename in &filenames[1..] {
    let config_contents = fs::read_to_string(filename)?;
    let c: Config = match get_file_ext(filename) {
        ConfigFiletype::Ahk => {
    let ahk_config = parse_ahk_file(filename).map_err(|e| format!("AHK parse error: {}", e))?;
    let extracted_hotstrings = crate::ahk::transpiler::extract_hotstrings(&ahk_config);

    let mut cfg = Config::new();

    // <-- ADD THIS LINE -->
    // cfg.virtual_modifiers.push(Key::KEY_CAPSLOCK);

    let hotkey_count = ahk_config.hotkeys.len();
    println!("DEBUG: Parsed {} hotkeys from additional AHK file", hotkey_count);

    for hotkey in ahk_config.hotkeys {
        let keymap = convert_ahk_hotkey_to_keymap(hotkey);
        cfg.keymap.push(keymap);
    }

    cfg.hotstrings = extracted_hotstrings;
    if !cfg.hotstrings.is_empty() {
        cfg.hotstring_matcher = Some(HotstringMatcher::new(cfg.hotstrings.clone()));
    }

    println!("Loaded {} AHK hotkeys (additional file)", hotkey_count);
    println!("Loaded {} AHK hotstrings (additional file)", cfg.hotstrings.len());

    cfg
}
        ConfigFiletype::Yaml => serde_yaml::from_str(&config_contents)?,
        ConfigFiletype::Toml => toml::from_str(&config_contents)?,
    };

    config.modmap.extend(c.modmap);
    config.keymap.extend(c.keymap);
    config.virtual_modifiers.extend(c.virtual_modifiers);
    config.hotstrings.extend(c.hotstrings);
}

    config.modify_time = filenames.last().and_then(|path| path.metadata().ok()?.modified().ok());
    config.keymap_table = build_keymap_table(&config.keymap);

    Ok(config)
}
fn convert_ahk_hotkey_to_keymap(hotkey: crate::ahk::AhkHotkey) -> Keymap {
    let mut keymap = Keymap {
        name: String::new(),
        remap: HashMap::new(),
        application: None,
        window: None,
        device: None,
        mode: None,
        exact_match: false,
    };

    if let Some(context) = &hotkey.context {
        if context.contains("ahk_exe") {
            keymap.application = parse_ahk_context(context);
        } else {
            use regex::Regex;
            let title_re = Regex::new(r#"WinActive\("([^"]+)"\)"#).unwrap();
            if let Some(caps) = title_re.captures(context) {
                let window_title = caps[1].to_string();
                keymap.window = Some(application::OnlyOrNot {
                    only: Some(vec![application::ApplicationMatcher::Literal(window_title)]),
                    not: None,
                });
                keymap.exact_match = true;
            }
        }
    }

    let modifiers: Vec<key_press::Modifier> = hotkey
        .modifiers
        .iter()
        .map(|k| match k {
            &Key::KEY_LEFTCTRL | &Key::KEY_RIGHTCTRL => key_press::Modifier::Control,
            &Key::KEY_LEFTALT | &Key::KEY_RIGHTALT => key_press::Modifier::Alt,
            &Key::KEY_LEFTSHIFT | &Key::KEY_RIGHTSHIFT => key_press::Modifier::Shift,
            &Key::KEY_LEFTMETA | &Key::KEY_RIGHTMETA => key_press::Modifier::Windows,
            k => key_press::Modifier::Key(k.clone()),
        })
        .collect();

    let key_press = key_press::KeyPress {
        key: hotkey.key,
        modifiers,
    };

    let actions = match hotkey.action {
        AhkAction::Run(parts) => {
    let mut cmd = Vec::new();
    if parts[0].starts_with("http://") || parts[0].starts_with("https://") {
        cmd.push("xdg-open".to_string());
        cmd.push(parts[0].clone());
    } else {
        cmd.push("/bin/sh".to_string());
        cmd.push("-c".to_string());
        cmd.push(parts.join(" "));
    }
    vec![KeymapAction::Launch(cmd)]
}
        AhkAction::Send(keys) => convert_send_to_actions(&keys),
        AhkAction::Remap(target_keys) => {
            // For simple remaps, create KeyPress without modifiers from the source
            target_keys
                .into_iter()
                .map(|k| {
                    keymap_action::KeymapAction::KeyPressAndRelease(key_press::KeyPress {
                        key: k,
                        modifiers: vec![], // Don't include source modifiers in target
                    })
                })
                .collect()
        }
        AhkAction::Sleep(ms) => {
            vec![keymap_action::KeymapAction::Sleep(ms)]
        }
    };

    keymap.remap.insert(key_press, actions);
    keymap
}
