
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
    Shell(String),           // NEW: raw shell script
    Block(Vec<AhkAction>),   // NEW: sequence of actions
    WinActivate(WindowCriteria),
    WinClose(WindowCriteria),
    IfWinActive {
        criteria: WindowCriteria,
        then_actions: Vec<AhkAction>,
        else_actions: Option<Vec<AhkAction>>,
    },
    WinWaitActive { criteria: WindowCriteria, timeout_ms: Option<u64> },

}

#[derive(Debug, Clone)]
pub enum WindowCriteria {
    Title(String),      // WinActivate("Firefox")
    Class(String),      // WinActivate("ahk_class dolphin")
    Exe(String),        // WinActivate("ahk_exe google-chrome")
}

#[derive(Debug, Clone)]
pub enum WindowCommand {
    Activate,
    WaitActive,
    Close,
}