use crate::action::Action;
use crate::ahk::types::{AhkAction, WindowCriteria};
use crate::client::WMClient;
use crate::event::{KeyEvent, KeyValue};  // Add KeyEvent and KeyValue here
use evdev::KeyCode as Key;
use std::error::Error;
use std::time::Duration;

/// The AHK Runtime Interpreter
/// Executes AHK actions at runtime, similar to AutoHotkey's script engine
pub struct AhkInterpreter<'a> {
    wm_client: &'a mut WMClient,
    application_cache: Option<String>,
    title_cache: Option<String>,
}

impl<'a> AhkInterpreter<'a> {
    pub fn new(wm_client: &'a mut WMClient) -> Self {
        Self {
            wm_client,
            application_cache: None,
            title_cache: None,
        }
    }

    /// Execute an AHK action and return the resulting low-level Actions
    pub fn execute(&mut self, action: &AhkAction) -> Result<Vec<Action>, Box<dyn Error>> {
        let mut actions = Vec::new();
        self.execute_into(action, &mut actions)?;
        Ok(actions)
    }

    /// Execute an AHK action, appending results to the actions vector
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

            // AhkAction::Send(keys) => {
            //     // Use ydotool for Wayland compatibility
            //     eprintln!("DEBUG INTERPRETER: Converting Send('{}') to ydotool command", keys);
            //     let ydotool_cmd = self.convert_send_to_ydotool(keys);
            //     actions.push(Action::Command(ydotool_cmd));
            // }
            AhkAction::Send(keys) => {
    eprintln!("DEBUG INTERPRETER: Converting Send('{}') to internal actions", keys);
    let send_actions = self.convert_send_to_actions(keys);
    actions.extend(send_actions);
}

AhkAction::Remap(target_keys) => {
    for key in target_keys {
        actions.push(Action::KeyEvent(KeyEvent::new(*key, KeyValue::Press)));
        actions.push(Action::KeyEvent(KeyEvent::new(*key, KeyValue::Release)));
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
                // Execute a sequence of actions
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
                // THIS IS THE KEY PART - Runtime evaluation!
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
                // WinWaitActive blocks until window is active
                let poll_interval_ms = 50;
                
                if let Some(timeout) = timeout_ms {
                    // Finite timeout
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
                    // Infinite timeout - wait forever
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
    // FORCE fresh check - clear cache
    self.application_cache = None;
    self.title_cache = None;
    
    // Small delay to allow WM to update (if WinActivate was just called)
    std::thread::sleep(std::time::Duration::from_millis(50));

    match criteria {
        WindowCriteria::Exe(exe) => {
            // Get FRESH window class
            let window_class = self.wm_client.current_application()
                .or_else(|| {
                    // Fallback to kdotool
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
                })
                .unwrap_or_default();
            
            eprintln!("DEBUG: Checking if '{}' == '{}'", window_class, exe);
            Ok(window_class == *exe)
        }
        
        WindowCriteria::Class(class) => {
            let window_class = self.wm_client.current_application()
                .or_else(|| {
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
                })
                .unwrap_or_default();

            eprintln!("DEBUG: Checking if '{}' == '{}'", window_class, class);
            Ok(window_class == *class)
        }

        WindowCriteria::Title(title) => {
            let window_title = self.wm_client.current_window()
                .or_else(|| {
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
                })
                .unwrap_or_default();

            eprintln!("DEBUG: Checking if '{}' == '{}'", window_title, title);
            Ok(window_title == *title)
        }
    }
}

    // /// Check if a window matching the criteria is currently active
    // fn check_window_active(&mut self, criteria: &WindowCriteria) -> Result<bool, Box<dyn Error>> {
    //     // Clear cache for fresh check
    //     self.application_cache = None;
    //     self.title_cache = None;

    //     match criteria {
    //         WindowCriteria::Exe(exe) => {
    //             // Get the current window class
    //             let window_class = if let Some(cached) = &self.application_cache {
    //                 cached.clone()
    //             } else {
    //                 let class = self.wm_client.current_application()
    //                     .or_else(|| {
    //                         // Fallback to kdotool
    //                         std::process::Command::new("kdotool")
    //                             .arg("getactivewindow")
    //                             .arg("getwindowclassname")
    //                             .output()
    //                             .ok()
    //                             .and_then(|out| {
    //                                 if out.status.success() {
    //                                     Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    //                                 } else {
    //                                     None
    //                                 }
    //                             })
    //                     })
    //                     .unwrap_or_default();
                    
    //                 self.application_cache = Some(class.clone());
    //                 class
    //             };

    //             Ok(window_class == *exe)
    //         }
            
    //         WindowCriteria::Class(class) => {
    //             // Get the current window class
    //             let window_class = if let Some(cached) = &self.application_cache {
    //                 cached.clone()
    //             } else {
    //                 let wclass = self.wm_client.current_application()
    //                     .or_else(|| {
    //                         // Fallback to kdotool
    //                         std::process::Command::new("kdotool")
    //                             .arg("getactivewindow")
    //                             .arg("getwindowclassname")
    //                             .output()
    //                             .ok()
    //                             .and_then(|out| {
    //                                 if out.status.success() {
    //                                     Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    //                                 } else {
    //                                     None
    //                                 }
    //                             })
    //                     })
    //                     .unwrap_or_default();
                    
    //                 self.application_cache = Some(wclass.clone());
    //                 wclass
    //             };

    //             Ok(window_class == *class)
    //         }

    //         WindowCriteria::Title(title) => {
    //             // Get the current window title
    //             let window_title = if let Some(cached) = &self.title_cache {
    //                 cached.clone()
    //             } else {
    //                 let title = self.wm_client.current_window()
    //                     .or_else(|| {
    //                         // Fallback to kdotool
    //                         std::process::Command::new("kdotool")
    //                             .arg("getactivewindow")
    //                             .arg("getwindowname")
    //                             .output()
    //                             .ok()
    //                             .and_then(|out| {
    //                                 if out.status.success() {
    //                                     Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    //                                 } else {
    //                                     None
    //                                 }
    //                             })
    //                     })
    //                     .unwrap_or_default();
                    
    //                 self.title_cache = Some(title.clone());
    //                 title
    //             };

    //             Ok(window_title == *title)
    //         }
    //     }
    // }


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
                            // Press Shift
                            actions.push(Action::KeyEvent(KeyEvent::new(
                                Key::KEY_LEFTSHIFT, 
                                KeyValue::Press
                            )));
                        }
                        
                        // Press and release the key
                        actions.push(Action::KeyEvent(KeyEvent::new(key, KeyValue::Press)));
                        actions.push(Action::KeyEvent(KeyEvent::new(key, KeyValue::Release)));
                        
                        if needs_shift {
                            // Release Shift
                            actions.push(Action::KeyEvent(KeyEvent::new(
                                Key::KEY_LEFTSHIFT, 
                                KeyValue::Release
                            )));
                        }
                    }
                }
            }
            SendToken::Key { key, modifiers } => {
                // Press modifiers
                for modifier in &modifiers {
                    actions.push(Action::KeyEvent(KeyEvent::new(*modifier, KeyValue::Press)));
                }
                // Press/release main key
                actions.push(Action::KeyEvent(KeyEvent::new(key, KeyValue::Press)));
                actions.push(Action::KeyEvent(KeyEvent::new(key, KeyValue::Release)));
                // Release modifiers
                for modifier in modifiers.iter().rev() {
                    actions.push(Action::KeyEvent(KeyEvent::new(*modifier, KeyValue::Release)));
                }
            }
        }
    }
    
    actions
}

fn char_to_key(&self, ch: char) -> Option<Key> {
    match ch {
        // Letters - lowercase
        'a' => Some(Key::KEY_A),
        'b' => Some(Key::KEY_B),
        'c' => Some(Key::KEY_C),
        'd' => Some(Key::KEY_D),
        'e' => Some(Key::KEY_E),
        'f' => Some(Key::KEY_F),
        'g' => Some(Key::KEY_G),
        'h' => Some(Key::KEY_H),
        'i' => Some(Key::KEY_I),
        'j' => Some(Key::KEY_J),
        'k' => Some(Key::KEY_K),
        'l' => Some(Key::KEY_L),
        'm' => Some(Key::KEY_M),
        'n' => Some(Key::KEY_N),
        'o' => Some(Key::KEY_O),
        'p' => Some(Key::KEY_P),
        'q' => Some(Key::KEY_Q),
        'r' => Some(Key::KEY_R),
        's' => Some(Key::KEY_S),
        't' => Some(Key::KEY_T),
        'u' => Some(Key::KEY_U),
        'v' => Some(Key::KEY_V),
        'w' => Some(Key::KEY_W),
        'x' => Some(Key::KEY_X),
        'y' => Some(Key::KEY_Y),
        'z' => Some(Key::KEY_Z),
        
        // Numbers
        '0' => Some(Key::KEY_0),
        '1' => Some(Key::KEY_1),
        '2' => Some(Key::KEY_2),
        '3' => Some(Key::KEY_3),
        '4' => Some(Key::KEY_4),
        '5' => Some(Key::KEY_5),
        '6' => Some(Key::KEY_6),
        '7' => Some(Key::KEY_7),
        '8' => Some(Key::KEY_8),
        '9' => Some(Key::KEY_9),
        
        // Common punctuation/symbols
        ' ' => Some(Key::KEY_SPACE),
        '.' => Some(Key::KEY_DOT),
        ',' => Some(Key::KEY_COMMA),
        ';' => Some(Key::KEY_SEMICOLON),
        '/' => Some(Key::KEY_SLASH),
        '\'' => Some(Key::KEY_APOSTROPHE),
        '-' => Some(Key::KEY_MINUS),
        '=' => Some(Key::KEY_EQUAL),
        '[' => Some(Key::KEY_LEFTBRACE),
        ']' => Some(Key::KEY_RIGHTBRACE),
        '\\' => Some(Key::KEY_BACKSLASH),
        '`' => Some(Key::KEY_GRAVE),
        '\n' => Some(Key::KEY_ENTER),
        '\t' => Some(Key::KEY_TAB),
        
        // Uppercase letters need Shift modifier
        'A'..='Z' => {
            // For now, return None - we'll handle uppercase differently
            None
        }
        
        // Shifted symbols need special handling
        '!' | '@' | '#' | '$' | '%' | '^' | '&' | '*' | '(' | ')' |
        '_' | '+' | '{' | '}' | '|' | ':' | '"' | '<' | '>' | '?' | '~' => {
            // Return None for now - needs shift handling
            None
        }
        
        _ => None,
    }
}

fn char_to_key_with_shift(&self, ch: char) -> Option<(Key, bool)> {
    // Returns (Key, needs_shift)
    match ch {
        // Lowercase letters - no shift
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
        
        // Uppercase letters - need shift
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
        
        // Numbers - no shift
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
        
        // Shifted number symbols
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
        
        // Punctuation
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
        
        // Shifted punctuation
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

    fn build_kdotool_shell(&self, criteria: &WindowCriteria, action: &str) -> String {
        let search_arg = match criteria {
            WindowCriteria::Title(title) => format!("--name '{}'", title.replace("'", "'\\''")),
            WindowCriteria::Class(class) => format!("--class '{}'", class.replace("'", "'\\''")),
            WindowCriteria::Exe(exe) => format!("--classname '{}'", exe.replace("'", "'\\''")),
        };
        
        format!("kdotool search {} {}", search_arg, action)
    }
}