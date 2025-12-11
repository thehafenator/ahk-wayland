use evdev::KeyCode;

#[derive(Debug, Clone)]
pub enum SendToken {
    Key { key: KeyCode, modifiers: Vec<KeyCode> },
    Text(String),
}

pub fn parse_send_string(input: &str) -> Vec<SendToken> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut current_mods = Vec::new();
    let mut text_buffer = String::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            '^' | '!' | '+' | '#' => {
                // Flush text buffer before processing modifier
                if !text_buffer.is_empty() {
                    tokens.push(SendToken::Text(text_buffer.clone()));
                    text_buffer.clear();
                }
                
                chars.next();
                match ch {
                    '^' => current_mods.push(KeyCode::KEY_LEFTCTRL),
                    '!' => current_mods.push(KeyCode::KEY_LEFTALT),
                    '+' => current_mods.push(KeyCode::KEY_LEFTSHIFT),
                    '#' => current_mods.push(KeyCode::KEY_LEFTMETA),
                    _ => {}
                }
            }
            '{' => {
                // Flush text buffer before processing special key
                if !text_buffer.is_empty() {
                    tokens.push(SendToken::Text(text_buffer.clone()));
                    text_buffer.clear();
                }
                
                chars.next();
                let mut key_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '}' {
                        chars.next();
                        break;
                    }
                    key_name.push(c);
                    chars.next();
                }
                
                if let Some(key) = parse_special_key(&key_name) {
                    tokens.push(SendToken::Key {
                        key,
                        modifiers: current_mods.clone(),
                    });
                    current_mods.clear();
                }
            }
            _ => {
                let c = chars.next().unwrap();
                // If we have modifiers, treat as a key
                if !current_mods.is_empty() {
                    if !text_buffer.is_empty() {
                        tokens.push(SendToken::Text(text_buffer.clone()));
                        text_buffer.clear();
                    }
                    
                    if let Some(key) = char_to_key(c) {
                        tokens.push(SendToken::Key {
                            key,
                            modifiers: current_mods.clone(),
                        });
                        current_mods.clear();
                    }
                } else {
                    // No modifiers - accumulate as text
                    text_buffer.push(c);
                }
            }
        }
    }
    
    // Flush remaining text
    if !text_buffer.is_empty() {
        tokens.push(SendToken::Text(text_buffer));
    }

    tokens
}

fn parse_special_key(name: &str) -> Option<KeyCode> {
    match name.to_lowercase().as_str() {
        "enter" | "return" => Some(KeyCode::KEY_ENTER),
        "tab" => Some(KeyCode::KEY_TAB),
        "space" => Some(KeyCode::KEY_SPACE),
        "backspace" | "bs" => Some(KeyCode::KEY_BACKSPACE),
        "delete" | "del" => Some(KeyCode::KEY_DELETE),
        "escape" | "esc" => Some(KeyCode::KEY_ESC),
        "up" => Some(KeyCode::KEY_UP),
        "down" => Some(KeyCode::KEY_DOWN),
        "left" => Some(KeyCode::KEY_LEFT),
        "right" => Some(KeyCode::KEY_RIGHT),
        "home" => Some(KeyCode::KEY_HOME),
        "end" => Some(KeyCode::KEY_END),
        "pgup" | "pageup" => Some(KeyCode::KEY_PAGEUP),
        "pgdn" | "pagedown" => Some(KeyCode::KEY_PAGEDOWN),
        "a" => Some(KeyCode::KEY_A),
        "b" => Some(KeyCode::KEY_B),
        "c" => Some(KeyCode::KEY_C),
        "d" => Some(KeyCode::KEY_D),
        "e" => Some(KeyCode::KEY_E),
        "f" => Some(KeyCode::KEY_F),
        "g" => Some(KeyCode::KEY_G),
        "h" => Some(KeyCode::KEY_H),
        "i" => Some(KeyCode::KEY_I),
        "j" => Some(KeyCode::KEY_J),
        "k" => Some(KeyCode::KEY_K),
        "l" => Some(KeyCode::KEY_L),
        "m" => Some(KeyCode::KEY_M),
        "n" => Some(KeyCode::KEY_N),
        "o" => Some(KeyCode::KEY_O),
        "p" => Some(KeyCode::KEY_P),
        "q" => Some(KeyCode::KEY_Q),
        "r" => Some(KeyCode::KEY_R),
        "s" => Some(KeyCode::KEY_S),
        "t" => Some(KeyCode::KEY_T),
        "u" => Some(KeyCode::KEY_U),
        "v" => Some(KeyCode::KEY_V),
        "w" => Some(KeyCode::KEY_W),
        "x" => Some(KeyCode::KEY_X),
        "y" => Some(KeyCode::KEY_Y),
        "z" => Some(KeyCode::KEY_Z),
        "f1" => Some(KeyCode::KEY_F1),
        "f2" => Some(KeyCode::KEY_F2),
        "f3" => Some(KeyCode::KEY_F3),
        "f4" => Some(KeyCode::KEY_F4),
        "f5" => Some(KeyCode::KEY_F5),
        "f6" => Some(KeyCode::KEY_F6),
        "f7" => Some(KeyCode::KEY_F7),
        "f8" => Some(KeyCode::KEY_F8),
        "f9" => Some(KeyCode::KEY_F9),
        "f10" => Some(KeyCode::KEY_F10),
        "f11" => Some(KeyCode::KEY_F11),
        "f12" => Some(KeyCode::KEY_F12),
        _ => None,
    }
}

fn char_to_key(c: char) -> Option<KeyCode> {
    match c.to_ascii_lowercase() {
        'a' => Some(KeyCode::KEY_A),
        'b' => Some(KeyCode::KEY_B),
        'c' => Some(KeyCode::KEY_C),
        'd' => Some(KeyCode::KEY_D),
        'e' => Some(KeyCode::KEY_E),
        'f' => Some(KeyCode::KEY_F),
        'g' => Some(KeyCode::KEY_G),
        'h' => Some(KeyCode::KEY_H),
        'i' => Some(KeyCode::KEY_I),
        'j' => Some(KeyCode::KEY_J),
        'k' => Some(KeyCode::KEY_K),
        'l' => Some(KeyCode::KEY_L),
        'm' => Some(KeyCode::KEY_M),
        'n' => Some(KeyCode::KEY_N),
        'o' => Some(KeyCode::KEY_O),
        'p' => Some(KeyCode::KEY_P),
        'q' => Some(KeyCode::KEY_Q),
        'r' => Some(KeyCode::KEY_R),
        's' => Some(KeyCode::KEY_S),
        't' => Some(KeyCode::KEY_T),
        'u' => Some(KeyCode::KEY_U),
        'v' => Some(KeyCode::KEY_V),
        'w' => Some(KeyCode::KEY_W),
        'x' => Some(KeyCode::KEY_X),
        'y' => Some(KeyCode::KEY_Y),
        'z' => Some(KeyCode::KEY_Z),
        '0' => Some(KeyCode::KEY_0),
        '1' => Some(KeyCode::KEY_1),
        '2' => Some(KeyCode::KEY_2),
        '3' => Some(KeyCode::KEY_3),
        '4' => Some(KeyCode::KEY_4),
        '5' => Some(KeyCode::KEY_5),
        '6' => Some(KeyCode::KEY_6),
        '7' => Some(KeyCode::KEY_7),
        '8' => Some(KeyCode::KEY_8),
        '9' => Some(KeyCode::KEY_9),
        ' ' => Some(KeyCode::KEY_SPACE),
        _ => None,
    }
}
