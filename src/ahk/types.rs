use evdev::KeyCode;

#[derive(Debug, Clone)]
pub struct AhkConfig {
    pub hotkeys: Vec<AhkHotkey>,
    pub hotstrings: Vec<AhkHotstring>,
}

#[derive(Debug, Clone)]
pub struct AhkHotkey {
    pub modifiers: Vec<KeyCode>,
    pub key: KeyCode,
    pub action: AhkAction,
    pub context: Option<String>,
    pub is_wildcard: bool,
}

#[derive(Debug, Clone)]
pub struct AhkHotstring {
    pub trigger: String,
    pub replacement: String,
    pub immediate: bool,
    pub case_sensitive: bool,
    pub omit_char: bool,
    pub execute: bool,
    pub context: Option<String>,
}
#[derive(Debug, Clone)]
pub enum AhkAction {
    Run(Vec<String>),
    Send(String),
    Remap(Vec<KeyCode>),
    Sleep(u64),
}
