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
use crate::ahk::WindowCriteria;
use crate::config::keymap_action::KeymapAction;
use crate::config::key::parse_key;
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
    // key::Key,
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
    for ch in text.chars() {
        let (key_opt, needs_shift) = if ch.is_ascii_uppercase() {
            (char_to_evdev_key(ch.to_ascii_lowercase()), true)
        } else {
            (char_to_evdev_key(ch), false)
        };
        
        if let Some(key) = key_opt {
            let modifiers = if needs_shift {
                vec![key_press::Modifier::Shift]
            } else {
                vec![]
            };
            let key_press = key_press::KeyPress { key, modifiers };
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
        'a'..='z' => string_to_key(&c.to_string()),
        'A'..='Z' => string_to_key(&c.to_lowercase().to_string()),
        '0'..='9' => string_to_key(&c.to_string()),
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
            
            // // Disable CapsLock toggle functionality
            use crate::config::modmap_action::{ModmapAction, Keys};
            // config.modmap.push(Modmap {
            //     name: "Disable CapsLock toggle".to_string(),
            //     remap: {
            //         let mut map = HashMap::new();
            //         map.insert(Key::KEY_CAPSLOCK, ModmapAction::Keys(Keys::Key(Key::KEY_CAPSLOCK)));
            //         map
            //     },
            //     application: None,
            //     window: None,
            //     device: None,
            //     mode: None,
            // });






            let mut context_hotkeys = Vec::new();
            let mut global_hotkeys = Vec::new();

            for hotkey in ahk_config.hotkeys {
    let keymap = convert_ahk_hotkey_to_keymap(hotkey);
    if keymap.window.is_some() || keymap.application.is_some() {
        context_hotkeys.push(keymap);
    } else {
        global_hotkeys.push(keymap);
    }
}
config.keymap.extend(context_hotkeys);
config.keymap.extend(global_hotkeys);

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
            cfg.virtual_modifiers.push(Key::KEY_CAPSLOCK);

            let hotkey_count = ahk_config.hotkeys.len();
            println!("DEBUG: Parsed {} hotkeys from additional AHK file", hotkey_count);

            // ADD THIS:
            let mut context_hotkeys = Vec::new();
            let mut global_hotkeys = Vec::new();

            for hotkey in ahk_config.hotkeys {
                let keymap = convert_ahk_hotkey_to_keymap(hotkey);
                if keymap.window.is_some() || keymap.application.is_some() {
                    context_hotkeys.push(keymap);
                } else {
                    global_hotkeys.push(keymap);
                }
            }
            cfg.keymap.extend(context_hotkeys);
            cfg.keymap.extend(global_hotkeys);

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

// Update the convert_actions function to detect when to use the interpreter
fn convert_actions(action: AhkAction) -> Vec<KeymapAction> {
    // Check if this action or any nested actions contain Send() or other
    // actions that need the dispatcher
    if needs_interpreter(&action) {
        eprintln!("DEBUG: Using interpreter for action: {:?}", action);
        vec![KeymapAction::AhkInterpreted(action)]
    } else {
        // Use shell script approach for simple actions
        eprintln!("DEBUG: Using shell script for action");
        convert_actions_to_shell(action)
    }
}


// Check if an action needs the interpreter (contains Send, or nested IfWinActive with Send, etc.)
fn needs_interpreter(action: &AhkAction) -> bool {
    match action {
        AhkAction::Send(_) => true,
        AhkAction::Remap(_) => true, // Direct key remap needs dispatcher
        AhkAction::WinWaitActive { .. } => true, // Needs interpreter for blocking wait
        AhkAction::Block(actions) => actions.iter().any(needs_interpreter),
        AhkAction::IfWinActive { then_actions, else_actions, .. } => {
            then_actions.iter().any(needs_interpreter) 
                || else_actions.as_ref().map_or(false, |actions| actions.iter().any(needs_interpreter))
        }
        // These can be handled via shell
        AhkAction::Run(_) 
        | AhkAction::Shell(_) 
        | AhkAction::Sleep(_) 
        | AhkAction::WinActivate(_) 
        | AhkAction::WinClose(_) => false,
    }
}


// Rename the old convert_actions to convert_actions_to_shell
fn convert_actions_to_shell(action: AhkAction) -> Vec<KeymapAction> {
    match action {
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
AhkAction::Send(_keys) => {
                // This shouldn't be reached if needs_interpreter works correctly
            eprintln!("WARNING: Send() in shell context - this won't work!");
            vec![]
        }
        AhkAction::Remap(_) => {
            eprintln!("WARNING: Remap in shell context - this won't work!");
            vec![]
        }
        AhkAction::Sleep(ms) => {
            vec![keymap_action::KeymapAction::Sleep(ms)]
        }
        AhkAction::Shell(script) => {
            vec![KeymapAction::Launch(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                script.trim().to_string(),
            ])]
        }
        AhkAction::Block(actions) => {
            let mut all = Vec::new();
            for a in actions {
                all.extend(convert_actions_to_shell(a));
            }
            all
        }
        AhkAction::WinActivate(criteria) => {
            vec![KeymapAction::Launch(build_kdotool_command("windowactivate", &criteria))]
        }

        AhkAction::WinWaitActive { .. } => {
            // Should never reach here - needs_interpreter returns true for this
            eprintln!("WARNING: WinWaitActive in shell context - should use interpreter!");
            vec![]
        }

        AhkAction::WinClose(criteria) => {
            vec![KeymapAction::Launch(build_kdotool_command("windowclose", &criteria))]
        }
        AhkAction::IfWinActive { criteria, then_actions, else_actions } => {
            // Only simple IfWinActive without Send() should reach here
            let condition_check = build_kdotool_shell(&criteria, "getactivewindow");
            let then_script = actions_to_shell_script(&then_actions);
            
            let mut script = format!("if {} ; then\n{}", condition_check, then_script);
            
            if let Some(else_actions) = else_actions {
                let else_script = actions_to_shell_script(&else_actions);
                script.push_str(&format!("\nelse\n{}", else_script));
            }
            
            script.push_str("\nfi");
            
            vec![KeymapAction::Launch(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                script,
            ])]
        }
    }
}

// Helper function to convert a list of AhkActions to shell script
fn actions_to_shell_script(actions: &[AhkAction]) -> String {
    let mut script = String::new();
    
    for action in actions {
        match action {
            AhkAction::Run(parts) => {
                if parts[0].starts_with("http://") || parts[0].starts_with("https://") {
                    script.push_str(&format!("  xdg-open '{}'\n", parts[0].replace("'", "'\\''")));
                } else {
                    script.push_str(&format!("  {}\n", parts.join(" ")));
                }
            }
            AhkAction::Shell(shell_script) => {
                script.push_str(&format!("  {}\n", shell_script));
            }
            AhkAction::Sleep(ms) => {
                script.push_str(&format!("  sleep {}\n", (*ms as f64) / 1000.0));
            }
            AhkAction::WinActivate(criteria) => {
                let cmd = build_kdotool_command("windowactivate", criteria);
                script.push_str(&format!("  {}\n", cmd.join(" ")));
            }
            AhkAction::WinClose(criteria) => {
                let cmd = build_kdotool_command("windowclose", criteria);
                script.push_str(&format!("  {}\n", cmd.join(" ")));
            }
            AhkAction::Send(_) => {
                // Send commands can't easily be converted to shell
                // We'd need to emit key events, which requires the dispatcher
                script.push_str("  # Send command not supported in shell context\n");
            }
            AhkAction::Block(nested_actions) => {
                script.push_str(&actions_to_shell_script(nested_actions));
            }
            _ => {
                script.push_str("  # Unsupported action in shell context\n");
            }
        }
    }
    
    script
}

fn build_kdotool_command(action: &str, criteria: &WindowCriteria) -> Vec<String> {
    let mut cmd = vec!["kdotool".to_string(), "search".to_string()];
    
    match criteria {
        WindowCriteria::Title(title) => {
            cmd.push("--name".to_string());
            cmd.push(title.clone());
        }
        WindowCriteria::Class(class) => {
            cmd.push("--class".to_string());
            cmd.push(class.clone());
        }
        WindowCriteria::Exe(exe) => {
            cmd.push("--classname".to_string());
            cmd.push(exe.clone());
        }
    }
    
    cmd.push(action.to_string());
    cmd
}

fn build_kdotool_shell(criteria: &WindowCriteria, action: &str) -> String {
    if action == "getactivewindow" {
        match criteria {
            WindowCriteria::Exe(exe) => {
                format!(
                    "test \"$(kdotool getactivewindow getwindowclassname 2>/dev/null || echo '__NONE__')\" = '{}'",
                    exe.replace("'", "'\\''")
                )
            }
            WindowCriteria::Class(class) => {
                format!(
                    "test \"$(kdotool getactivewindow getwindowclassname 2>/dev/null || echo '__NONE__')\" = '{}'",
                    class.replace("'", "'\\''")
                )
            }
            WindowCriteria::Title(title) => {
                format!(
                    "test \"$(kdotool getactivewindow getwindowname 2>/dev/null || echo '__NONE__')\" = '{}'",
                    title.replace("'", "'\\''")
                )
            }
        }
    } else {
        // For other actions (windowactivate), use search
        let search_arg = match criteria {
            WindowCriteria::Title(title) => format!("--name '{}'", title.replace("'", "'\\''")),
            WindowCriteria::Class(class) => format!("--class '{}'", class.replace("'", "'\\''")),
            WindowCriteria::Exe(exe) => format!("--classname '{}'", exe.replace("'", "'\\''")),
        };
        
        format!("kdotool search {} {}", search_arg, action)
    }
}


// Update convert_ahk_hotkey_to_keymap to use the helper:
fn convert_ahk_hotkey_to_keymap(hotkey: crate::ahk::AhkHotkey) -> Keymap {
    let mut keymap = Keymap {
        name: String::new(),
        remap: HashMap::new(),
        application: None,
        window: None,
        device: None,
        mode: None,
        exact_match: true,
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

    let actions = convert_actions(hotkey.action);
    keymap.remap.insert(key_press, actions);
    keymap
    // println!("DEBUG: Hotkey modifiers: {:?}, key: {:?}, context: {:?}", 
    //      hotkey.modifiers, hotkey.key, hotkey.context);
}

