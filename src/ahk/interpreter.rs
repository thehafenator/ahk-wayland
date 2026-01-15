use crate::action::Action;
use crate::ahk::types::{AhkAction, WindowCriteria};
use crate::client::WMClient;
use crate::event::{KeyEvent, KeyValue};
use evdev::KeyCode as Key;
use std::error::Error;
use std::time::Duration;
use std::collections::HashSet;

pub struct AhkInterpreter<'a> {
    wm_client: &'a mut WMClient,
    application_cache: Option<String>,
    title_cache: Option<String>,
    active_virtual_modifiers: HashSet<Key>,
}

impl<'a> AhkInterpreter<'a> {
    pub fn new(wm_client: &'a mut WMClient) -> Self {
        Self {
            wm_client,
            application_cache: None,
            title_cache: None,
            active_virtual_modifiers: HashSet::new(),
        }
    }

    pub fn set_virtual_modifiers(&mut self, modifiers: &[Key]) {
        self.active_virtual_modifiers = modifiers.iter().copied().collect();
        eprintln!("DEBUG: Set active virtual modifiers: {:?}", self.active_virtual_modifiers);
    }

    pub fn execute(&mut self, action: &AhkAction) -> Result<Vec<Action>, Box<dyn Error>> {
        let mut actions = Vec::new();
        self.execute_into(action, &mut actions)?;
        Ok(actions)
    }

    fn execute_into(&mut self, action: &AhkAction, actions: &mut Vec<Action>) -> Result<(), Box<dyn Error>> {
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
                actions.push(Action::Command(cmd));
            }

            AhkAction::Send(keys) => {
                eprintln!("DEBUG INTERPRETER: Converting Send('{}') with virtual modifiers: {:?}", 
                    keys, self.active_virtual_modifiers);
                
                for modifier in &self.active_virtual_modifiers {
                    eprintln!("DEBUG: Releasing virtual modifier: {:?}", modifier);
                    actions.push(Action::KeyEvent(KeyEvent::new(*modifier, KeyValue::Release)));
                }
                
                let send_actions = self.convert_send_to_actions(keys);
                actions.extend(send_actions);
                
                for modifier in &self.active_virtual_modifiers {
                    eprintln!("DEBUG: Re-pressing virtual modifier: {:?}", modifier);
                    actions.push(Action::KeyEvent(KeyEvent::new(*modifier, KeyValue::Press)));
                }
            }

            AhkAction::Remap(target_keys) => {
                for modifier in &self.active_virtual_modifiers {
                    actions.push(Action::KeyEvent(KeyEvent::new(*modifier, KeyValue::Release)));
                }
                
                for key in target_keys {
                    actions.push(Action::KeyEvent(KeyEvent::new(*key, KeyValue::Press)));
                    actions.push(Action::KeyEvent(KeyEvent::new(*key, KeyValue::Release)));
                }
                
                for modifier in &self.active_virtual_modifiers {
                    actions.push(Action::KeyEvent(KeyEvent::new(*modifier, KeyValue::Press)));
                }
            }

            AhkAction::Sleep(ms) => {
                actions.push(Action::Delay(Duration::from_millis(*ms)));
            }

            AhkAction::Shell(script) => {
                actions.push(Action::Command(vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    script.clone(),
                ]));
            }

            AhkAction::Block(block_actions) => {
                for block_action in block_actions {
                    self.execute_into(block_action, actions)?;
                }
            }

            AhkAction::WinActivate(criteria) => {
                let cmd = self.build_kdotool_command("windowactivate", criteria);
                actions.push(Action::Command(cmd));
            }

            AhkAction::WinClose(criteria) => {
                let cmd = self.build_kdotool_command("windowclose", criteria);
                actions.push(Action::Command(cmd));
            }

            AhkAction::IfWinActive { criteria, then_actions, else_actions } => {
                eprintln!("DEBUG INTERPRETER: Evaluating IfWinActive at runtime");
                
                let is_active = self.check_window_active(criteria)?;
                eprintln!("DEBUG INTERPRETER: Window check result: {}", is_active);
                
                if is_active {
                    eprintln!("DEBUG INTERPRETER: Executing then_actions ({} actions)", then_actions.len());
                    for then_action in then_actions {
                        self.execute_into(then_action, actions)?;
                    }
                } else if let Some(else_actions) = else_actions {
                    eprintln!("DEBUG INTERPRETER: Executing else_actions ({} actions)", else_actions.len());
                    for else_action in else_actions {
                        self.execute_into(else_action, actions)?;
                    }
                }
            }

            AhkAction::WinWaitActive { criteria, timeout_ms } => {
                let poll_interval_ms = 50;
                
                if let Some(timeout) = timeout_ms {
                    let max_attempts = timeout / poll_interval_ms;
                    eprintln!("DEBUG: WinWaitActive - waiting for window (timeout: {}ms)", timeout);
                    
                    for attempt in 0..max_attempts {
                        if self.check_window_active(criteria).unwrap_or(false) {
                            eprintln!("DEBUG: WinWaitActive - window became active after {} ms", attempt * poll_interval_ms);
                            return Ok(());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
                    }
                    
                    eprintln!("DEBUG: WinWaitActive - timed out after {} ms", timeout);
                } else {
                    eprintln!("DEBUG: WinWaitActive - waiting for window (no timeout)");
                    let mut elapsed = 0u64;
                    
                    loop {
                        if self.check_window_active(criteria).unwrap_or(false) {
                            eprintln!("DEBUG: WinWaitActive - window became active after {} ms", elapsed);
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
                        elapsed += poll_interval_ms;
                    }
                }
            }
        }

        Ok(())
    }

    fn check_window_active(&mut self, criteria: &WindowCriteria) -> Result<bool, Box<dyn Error>> {
        self.application_cache = None;
        self.title_cache = None;
        
        std::thread::sleep(std::time::Duration::from_millis(50));

        match criteria {
            WindowCriteria::Exe(exe) => {
    let mut window_class = self.wm_client.current_application();
    
    #[cfg(feature = "kde")]
    {
        window_class = window_class.or_else(|| {
            std::process::Command::new("kdotool")
                .arg("getactivewindow")
                .arg("getwindowclassname")
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
        });
    }
    
    let window_class = window_class.unwrap_or_default();
    eprintln!("DEBUG: Checking if '{}' == '{}'", window_class, exe);
    Ok(window_class == *exe)
}

WindowCriteria::Class(class) => {
    let mut window_class = self.wm_client.current_application();
    
    #[cfg(feature = "kde")]
    {
        window_class = window_class.or_else(|| {
            std::process::Command::new("kdotool")
                .arg("getactivewindow")
                .arg("getwindowclassname")
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
        });
    }
    
    let window_class = window_class.unwrap_or_default();
    eprintln!("DEBUG: Checking if '{}' == '{}'", window_class, class);
    Ok(window_class == *class)
}

WindowCriteria::Title(title) => {
    let mut window_title = self.wm_client.current_window();
    
    #[cfg(feature = "kde")]
    {
        window_title = window_title.or_else(|| {
            std::process::Command::new("kdotool")
                .arg("getactivewindow")
                .arg("getwindowname")
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
        });
    }
    
    let window_title = window_title.unwrap_or_default();
    eprintln!("DEBUG: Checking if '{}' == '{}'", window_title, title);
    Ok(window_title == *title)
}
        }
    }

    fn convert_send_to_actions(&self, send_str: &str) -> Vec<Action> {
        use crate::ahk::send_parser::{parse_send_string, SendToken};
        use crate::event::{KeyEvent, KeyValue};
        
        let tokens = parse_send_string(send_str);
        let mut actions = Vec::new();
        
        for token in tokens {
            match token {
                SendToken::Text(text) => {
                    for ch in text.chars() {
                        if let Some((key, needs_shift)) = self.char_to_key_with_shift(ch) {
                            if needs_shift {
                                actions.push(Action::KeyEvent(KeyEvent::new(
                                    Key::KEY_LEFTSHIFT, 
                                    KeyValue::Press
                                )));
                            }
                            
                            actions.push(Action::KeyEvent(KeyEvent::new(key, KeyValue::Press)));
                            actions.push(Action::KeyEvent(KeyEvent::new(key, KeyValue::Release)));
                            
                            if needs_shift {
                                actions.push(Action::KeyEvent(KeyEvent::new(
                                    Key::KEY_LEFTSHIFT, 
                                    KeyValue::Release
                                )));
                            }
                        }
                    }
                }
                SendToken::Key { key, modifiers } => {
                    for modifier in &modifiers {
                        actions.push(Action::KeyEvent(KeyEvent::new(*modifier, KeyValue::Press)));
                    }
                    actions.push(Action::KeyEvent(KeyEvent::new(key, KeyValue::Press)));
                    actions.push(Action::KeyEvent(KeyEvent::new(key, KeyValue::Release)));
                    for modifier in modifiers.iter().rev() {
                        actions.push(Action::KeyEvent(KeyEvent::new(*modifier, KeyValue::Release)));
                    }
                }
            }
        }
        
        actions
    }

    fn char_to_key_with_shift(&self, ch: char) -> Option<(Key, bool)> {
        match ch {
            'a'..='z' => {
                let key = match ch {
                    'a' => Key::KEY_A, 'b' => Key::KEY_B, 'c' => Key::KEY_C,
                    'd' => Key::KEY_D, 'e' => Key::KEY_E, 'f' => Key::KEY_F,
                    'g' => Key::KEY_G, 'h' => Key::KEY_H, 'i' => Key::KEY_I,
                    'j' => Key::KEY_J, 'k' => Key::KEY_K, 'l' => Key::KEY_L,
                    'm' => Key::KEY_M, 'n' => Key::KEY_N, 'o' => Key::KEY_O,
                    'p' => Key::KEY_P, 'q' => Key::KEY_Q, 'r' => Key::KEY_R,
                    's' => Key::KEY_S, 't' => Key::KEY_T, 'u' => Key::KEY_U,
                    'v' => Key::KEY_V, 'w' => Key::KEY_W, 'x' => Key::KEY_X,
                    'y' => Key::KEY_Y, 'z' => Key::KEY_Z,
                    _ => return None,
                };
                Some((key, false))
            }
            'A'..='Z' => {
                let key = match ch {
                    'A' => Key::KEY_A, 'B' => Key::KEY_B, 'C' => Key::KEY_C,
                    'D' => Key::KEY_D, 'E' => Key::KEY_E, 'F' => Key::KEY_F,
                    'G' => Key::KEY_G, 'H' => Key::KEY_H, 'I' => Key::KEY_I,
                    'J' => Key::KEY_J, 'K' => Key::KEY_K, 'L' => Key::KEY_L,
                    'M' => Key::KEY_M, 'N' => Key::KEY_N, 'O' => Key::KEY_O,
                    'P' => Key::KEY_P, 'Q' => Key::KEY_Q, 'R' => Key::KEY_R,
                    'S' => Key::KEY_S, 'T' => Key::KEY_T, 'U' => Key::KEY_U,
                    'V' => Key::KEY_V, 'W' => Key::KEY_W, 'X' => Key::KEY_X,
                    'Y' => Key::KEY_Y, 'Z' => Key::KEY_Z,
                    _ => return None,
                };
                Some((key, true))
            }
            '0' => Some((Key::KEY_0, false)),
            '1' => Some((Key::KEY_1, false)),
            '2' => Some((Key::KEY_2, false)),
            '3' => Some((Key::KEY_3, false)),
            '4' => Some((Key::KEY_4, false)),
            '5' => Some((Key::KEY_5, false)),
            '6' => Some((Key::KEY_6, false)),
            '7' => Some((Key::KEY_7, false)),
            '8' => Some((Key::KEY_8, false)),
            '9' => Some((Key::KEY_9, false)),
            '!' => Some((Key::KEY_1, true)),
            '@' => Some((Key::KEY_2, true)),
            '#' => Some((Key::KEY_3, true)),
            '$' => Some((Key::KEY_4, true)),
            '%' => Some((Key::KEY_5, true)),
            '^' => Some((Key::KEY_6, true)),
            '&' => Some((Key::KEY_7, true)),
            '*' => Some((Key::KEY_8, true)),
            '(' => Some((Key::KEY_9, true)),
            ')' => Some((Key::KEY_0, true)),
            ' ' => Some((Key::KEY_SPACE, false)),
            '.' => Some((Key::KEY_DOT, false)),
            ',' => Some((Key::KEY_COMMA, false)),
            ';' => Some((Key::KEY_SEMICOLON, false)),
            '/' => Some((Key::KEY_SLASH, false)),
            '\'' => Some((Key::KEY_APOSTROPHE, false)),
            '-' => Some((Key::KEY_MINUS, false)),
            '=' => Some((Key::KEY_EQUAL, false)),
            '[' => Some((Key::KEY_LEFTBRACE, false)),
            ']' => Some((Key::KEY_RIGHTBRACE, false)),
            '\\' => Some((Key::KEY_BACKSLASH, false)),
            '`' => Some((Key::KEY_GRAVE, false)),
            ':' => Some((Key::KEY_SEMICOLON, true)),
            '?' => Some((Key::KEY_SLASH, true)),
            '"' => Some((Key::KEY_APOSTROPHE, true)),
            '_' => Some((Key::KEY_MINUS, true)),
            '+' => Some((Key::KEY_EQUAL, true)),
            '{' => Some((Key::KEY_LEFTBRACE, true)),
            '}' => Some((Key::KEY_RIGHTBRACE, true)),
            '|' => Some((Key::KEY_BACKSLASH, true)),
            '~' => Some((Key::KEY_GRAVE, true)),
            '<' => Some((Key::KEY_COMMA, true)),
            '>' => Some((Key::KEY_DOT, true)),
            '\n' => Some((Key::KEY_ENTER, false)),
            '\t' => Some((Key::KEY_TAB, false)),
            _ => None,
        }
    }

    #[cfg(feature = "kde")]
    fn build_kdotool_command(&self, action: &str, criteria: &WindowCriteria) -> Vec<String> {
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

    #[cfg(not(feature = "kde"))]
    fn build_kdotool_command(&self, _action: &str, _criteria: &WindowCriteria) -> Vec<String> {
        vec![]
    }
}
