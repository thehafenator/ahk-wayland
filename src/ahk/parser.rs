use crate::ahk::types::*;
use evdev::KeyCode;
use regex::Regex;
use std::path::Path;

pub struct AhkParser {
    // hotif_contexts: Vec<String>,
}

fn unescape_ahk_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '`' {
            if let Some(&next) = chars.peek() {
                chars.next();
                match next {
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    '`' => result.push('`'),
                    '"' => result.push('"'),
                    '\'' => result.push('\''),
                    _ => {
                        result.push('`');
                        result.push(next);
                    }
                }
            } else {
                result.push('`');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

impl AhkParser {
    pub fn new() -> Self {
        Self {
            // hotif_contexts: Vec::new(),
        }
    }

    fn parse_window_criteria(&self, content: &str) -> Result<WindowCriteria, String> {
        let content = content.trim().trim_matches(|c| c == '"' || c == '\'');
        
        if let Some(exe) = content.strip_prefix("ahk_exe ") {
            Ok(WindowCriteria::Exe(exe.trim().to_string()))
        } else if let Some(class) = content.strip_prefix("ahk_class ") {
            Ok(WindowCriteria::Class(class.trim().to_string()))
        } else {
            // Default to title match if no prefix
            Ok(WindowCriteria::Title(content.to_string()))
        }
    }

    pub fn parse_file(&mut self, content: &str) -> Result<AhkConfig, String> {
        let mut hotkeys = Vec::new();
        let mut hotstrings = Vec::new();
        let mut current_context = None;

        let mut lines = content.lines().enumerate().peekable();

        while let Some((_line_num, line)) = lines.next() {
            let line = line.trim();

            if line.is_empty() || line.starts_with(';') {
                continue;
            }

            if line.starts_with("#HotIf") {
                current_context = self.parse_hotif(line)?;
                continue;
            }

            if line == "#HotIf" {
                current_context = None;
                continue;
            }

            if line.starts_with(':') {
                if let Some(hotstring) = self.parse_hotstring(line, current_context.clone())? {
                    hotstrings.push(hotstring);
                    continue;
                } else {
                    return Err(format!("Failed to parse hotstring line: {}", line));
                }
            }

            if line.contains("::") {
                // Check if multiline block
                if line.ends_with("::{") || lines.peek().map(|(_, l)| l.trim()) == Some("{") {
                    // Consume opening brace if on next line
                    if !line.ends_with("::{") {
                        lines.next(); // consume the '{'
                    }
                    
                    let hotkey_def = if line.ends_with("::{") {
                        line.trim_end_matches('{').trim()
                    } else {
                        line
                    };
                    
                    if let Some(hotkey) = self.parse_multiline_hotkey(hotkey_def, &mut lines, current_context.clone())? {
                        hotkeys.push(hotkey);
                    }
                } else {
                    // Single-line hotkey
                    if let Some(hotkey) = self.parse_hotkey(line, current_context.clone())? {
                        hotkeys.push(hotkey);
                    } else {
                        return Err(format!("Failed to parse hotkey line: {}", line));
                    }
                }
            }
        }

        Ok(AhkConfig { hotkeys, hotstrings })
    }

  
fn parse_block_actions<'a>(
    &self,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<Vec<AhkAction>, String> {
    let mut actions = Vec::new();
    let mut depth = 1;
    
    while let Some((_, line)) = lines.next() {
        let trimmed = line.trim();
        
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        
        if trimmed == "}" {
            depth -= 1;
            if depth == 0 {
                break;
            }
            continue;
        }
        
        if trimmed == "{" {
            depth += 1;
            continue;
        }
        
        // Handle nested If blocks (recursive)
        if trimmed.starts_with("If WinActive(") || trimmed.starts_with("If !WinActive(") {
            let is_negated = trimmed.starts_with("If !");
            let prefix = if is_negated { "If !WinActive(" } else { "If WinActive(" };
            
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                if let Some(criteria_str) = rest.strip_suffix("){")
                    .or_else(|| rest.strip_suffix(") {"))
                    .or_else(|| rest.strip_suffix(")")) 
                {
                    let criteria = self.parse_window_criteria(criteria_str)?;
                    
                    let block_on_same_line = trimmed.ends_with("){") || trimmed.ends_with(") {");
                    if !block_on_same_line {
                        if let Some((_, next)) = lines.next() {
                            if next.trim() != "{" {
                                return Err("Expected '{' after If condition".to_string());
                            }
                        }
                    }
                    
                    // Recursively parse then block
                    let then_actions = self.parse_block_actions(lines)?;
                    
                    // Check for else
                    let mut else_actions = None;
                    
                    // Peek at next non-empty line
                    while let Some((_idx, line)) = lines.next() {
                        let peek = line.trim();
                        if peek.is_empty() || peek.starts_with(';') {
                            continue;
                        }
                        
                        if peek.starts_with("else") {
                            // Consume opening brace
                            let has_brace = peek.contains('{');
                            if !has_brace {
                                if let Some((_, brace_line)) = lines.next() {
                                    if brace_line.trim() != "{" {
                                        return Err("Expected '{' after else".to_string());
                                    }
                                }
                            }
                            else_actions = Some(self.parse_block_actions(lines)?);
                        } else {
                            // Not an else, this line belongs to outer scope
                            // We can't put it back, so try to parse it
                            if let Ok(action) = self.parse_action(peek) {
                                actions.push(action);
                            }
                        }
                        break;
                    }
                    
                    let action = if is_negated {
                        AhkAction::IfWinActive {
                            criteria,
                            then_actions: vec![],
                            else_actions: Some(then_actions),
                        }
                    } else {
                        AhkAction::IfWinActive {
                            criteria,
                            then_actions,
                            else_actions,
                        }
                    };
                    
                    actions.push(action);
                    continue;
                }
            }
        }
        
        // Handle Shell{} blocks
        if trimmed.starts_with("Shell{") || trimmed.starts_with("shell{") {
            let mut shell_lines = Vec::new();
            let first_line = trimmed.strip_prefix("Shell{")
                .or_else(|| trimmed.strip_prefix("shell{"))
                .unwrap()
                .trim();
            
            if first_line.ends_with('}') {
                let content = first_line.trim_end_matches('}').trim();
                if !content.is_empty() {
                    actions.push(AhkAction::Shell(content.to_string()));
                }
            } else {
                if !first_line.is_empty() {
                    shell_lines.push(first_line.to_string());
                }
                
                while let Some((_, shell_line)) = lines.next() {
                    let shell_trimmed = shell_line.trim();
                    if shell_trimmed == "}" {
                        break;
                    }
                    shell_lines.push(shell_line.to_string());
                }
                
                if !shell_lines.is_empty() {
                    actions.push(AhkAction::Shell(shell_lines.join("\n")));
                }
            }
            continue;
        }
        
        // Parse regular action
        if let Ok(action) = self.parse_action(trimmed) {
            actions.push(action);
        }
    }
    
    Ok(actions)
}

    fn parse_hotif(&mut self, line: &str) -> Result<Option<String>, String> {
        let re = Regex::new(r#"#HotIf\s+(.+)"#).unwrap();
        if let Some(caps) = re.captures(line) {
            Ok(Some(caps[1].to_string()))
        } else {
            Ok(None)
        }
    }

    fn parse_hotstring(&self, line: &str, context: Option<String>) -> Result<Option<AhkHotstring>, String> {
        let re = Regex::new(r"^(:([*?CcOoPpSsIiKkEeXxTtBbZz0-9]*):)?([^:]+)::(.*)$").unwrap();

        if let Some(caps) = re.captures(line) {
            let options = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let trigger = caps[3].to_string();
            let replacement = caps[4].to_string();

            Ok(Some(AhkHotstring {
                trigger,
                replacement,
                immediate: options.contains('*'),
                case_sensitive: options.contains('C'),
                omit_char: options.contains('O') || options.contains('o'),
                execute: options.contains('X') || options.contains('x'),
                context,
            }))
        } else {
            Ok(None)
        }
    }

    fn parse_hotkey(&self, line: &str, context: Option<String>) -> Result<Option<AhkHotkey>, String> {
        let parts: Vec<&str> = line.splitn(2, "::").collect();
        if parts.len() != 2 {
            return Ok(None);
        }

        let hotkey_def = parts[0].trim();
        let action_str = parts[1].trim();

        let action_str = if let Some(comment_pos) = action_str.find(" ;") {
            &action_str[..comment_pos]
        } else {
            action_str
        };

        let (modifiers, key, is_wildcard) = self.parse_key_combo(hotkey_def)?;
        let action = self.parse_action(action_str)?;

        Ok(Some(AhkHotkey {
            modifiers,
            key,
            action,
            context,
            is_wildcard,
        }))
    }

    fn parse_key_combo(&self, combo: &str) -> Result<(Vec<KeyCode>, KeyCode, bool), String> {
        let mut modifiers = Vec::new();
        let mut is_wildcard = false;
        let mut rest = combo;

        while rest.starts_with('~') || rest.starts_with('*') || rest.starts_with('$') {
            if rest.starts_with('~') {
                is_wildcard = true;
                rest = &rest[1..];
            } else if rest.starts_with('*') {
                is_wildcard = true;
                rest = &rest[1..];
            } else if rest.starts_with('$') {
                rest = &rest[1..];
            }
        }

        loop {
            if rest.starts_with('^') {
                modifiers.push(KeyCode::KEY_LEFTCTRL);
                rest = &rest[1..];
            } else if rest.starts_with('!') {
                modifiers.push(KeyCode::KEY_LEFTALT);
                rest = &rest[1..];
            } else if rest.starts_with('+') {
                modifiers.push(KeyCode::KEY_LEFTSHIFT);
                rest = &rest[1..];
            } else if rest.starts_with('#') {
                modifiers.push(KeyCode::KEY_LEFTMETA);
                rest = &rest[1..];
            } else {
                break;
            }
        }

        if rest.contains(" & ") {
            let parts: Vec<&str> = rest.split(" & ").collect();
            if parts.len() == 2 {
                if let Some(mod_key) = string_to_key(parts[0].trim()) {
                    modifiers.push(mod_key);
                }
                let main_key = string_to_key(parts[1].trim()).ok_or_else(|| format!("Unknown key: {}", parts[1]))?;
                return Ok((modifiers, main_key, is_wildcard));
            }
        }

        let key = string_to_key(rest.trim()).ok_or_else(|| format!("Unknown key: {}", rest))?;

        Ok((modifiers, key, is_wildcard))
    }

    fn parse_multiline_hotkey<'a>(
    &self,
    hotkey_def: &str,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
    context: Option<String>,
) -> Result<Option<AhkHotkey>, String> {
    let parts: Vec<&str> = hotkey_def.splitn(2, "::").collect();
    if parts.len() != 2 {
        return Ok(None);
    }

    let (modifiers, key, is_wildcard) = self.parse_key_combo(parts[0].trim())?;
    
    // Collect block lines
    let mut actions = Vec::new();
    let mut depth = 1; // Already inside one brace level
    
    while let Some((_, line)) = lines.next() {
        let trimmed = line.trim();
        
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        
        // Handle If WinActive() blocks
        if trimmed.starts_with("If WinActive(") || trimmed.starts_with("If !WinActive(") {
            eprintln!("DEBUG PARSER: Found If WinActive line: {}", trimmed);
            let is_negated = trimmed.starts_with("If !");
            let prefix = if is_negated { "If !WinActive(" } else { "If WinActive(" };
            
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                eprintln!("DEBUG PARSER: Stripped prefix, rest: {}", rest);
                if let Some(criteria_str) = rest.strip_suffix("){")
                    .or_else(|| rest.strip_suffix(") {"))
                    .or_else(|| rest.strip_suffix(")")) 
                {
                    eprintln!("DEBUG PARSER: Parsed criteria string: {}", criteria_str);
                    let criteria = self.parse_window_criteria(criteria_str)?;
                    eprintln!("DEBUG PARSER: Parsed criteria: {:?}", criteria);
                    
                    // Check if block starts on same line or next line
                    let block_on_same_line = trimmed.ends_with("){") || trimmed.ends_with(") {");
                    eprintln!("DEBUG PARSER: block_on_same_line: {}", block_on_same_line);
                    if !block_on_same_line {
                        // Consume the opening brace
                        if let Some((_, next)) = lines.next() {
                            eprintln!("DEBUG PARSER: Next line: {}", next.trim());
                            if next.trim() != "{" {
                                return Err("Expected '{' after If condition".to_string());
                            }
                        }
                    }
                    
                    // Collect then_actions until we hit }
                    eprintln!("DEBUG PARSER: About to parse then_actions block");
                    let then_actions = self.parse_block_actions(&mut *lines)?;
                    eprintln!("DEBUG PARSER: Parsed {} then_actions", then_actions.len());
                    
                    // Check for else block
                    let mut else_actions = None;
                    
                    // Peek ahead to see if there's an else
                    eprintln!("DEBUG PARSER: Looking for else block");
                    
                    while let Some((_idx, line)) = lines.next() {
                        let peek_trimmed = line.trim();
                        eprintln!("DEBUG PARSER: Checking line for else: '{}'", peek_trimmed);
                        
                        if peek_trimmed.is_empty() || peek_trimmed.starts_with(';') {
                            continue;
                        }
                        
                        if peek_trimmed.starts_with("else") {
                            eprintln!("DEBUG PARSER: Found else block!");
                            
                            // Consume opening brace
                            let has_brace = peek_trimmed.contains('{');
                            eprintln!("DEBUG PARSER: else has_brace: {}", has_brace);
                            if !has_brace {
                                if let Some((_, brace_line)) = lines.next() {
                                    eprintln!("DEBUG PARSER: else next line: {}", brace_line.trim());
                                    if brace_line.trim() != "{" {
                                        return Err("Expected '{' after else".to_string());
                                    }
                                }
                            }
                            
                            eprintln!("DEBUG PARSER: About to parse else_actions block");
                            else_actions = Some(self.parse_block_actions(&mut *lines)?);
                            eprintln!("DEBUG PARSER: Parsed {} else_actions", else_actions.as_ref().unwrap().len());
                            break;
                        } else {
                            eprintln!("DEBUG PARSER: Not an else, breaking");
                            // Not an else, this is the next statement - we're done with If
                            // We need to process this line, but we can't put it back
                            // For now, try to parse it as an action
if let Ok(_action) = self.parse_action(peek_trimmed) {
                                // Store it to be added after the If block
                                // This is a limitation - we'll lose this line
                            }
                            break;
                        }
                    }
                    
                    // Create IfWinActive action (handle negation)
                    let action = if is_negated {
                        eprintln!("DEBUG PARSER: Creating negated IfWinActive");
                        AhkAction::IfWinActive {
                            criteria,
                            then_actions: vec![],
                            else_actions: Some(then_actions),
                        }
                    } else {
                        eprintln!("DEBUG PARSER: Creating normal IfWinActive with else={:?}", else_actions.is_some());
                        AhkAction::IfWinActive {
                            criteria,
                            then_actions,
                            else_actions,
                        }
                    };
                    
                    eprintln!("DEBUG PARSER: Pushing IfWinActive action to actions list");
                    actions.push(action);
                    continue;
                }
            }
        }
        
        // Handle Shell{} blocks
        if trimmed.starts_with("Shell{") || trimmed.starts_with("shell{") {
            let mut shell_lines = Vec::new();
            let first_line = trimmed.strip_prefix("Shell{")
                .or_else(|| trimmed.strip_prefix("shell{"))
                .unwrap()
                .trim();
            
            if first_line.ends_with('}') {
                // Single-line Shell{}
                let content = first_line.trim_end_matches('}').trim();
                if !content.is_empty() {
                    actions.push(AhkAction::Shell(content.to_string()));
                }
            } else {
                // Multiline Shell{}
                if !first_line.is_empty() {
                    shell_lines.push(first_line.to_string());
                }
                
                while let Some((_, shell_line)) = lines.next() {
                    let shell_trimmed = shell_line.trim();
                    if shell_trimmed == "}" {
                        break;
                    }
                    shell_lines.push(shell_line.to_string());
                }
                
                if !shell_lines.is_empty() {
                    actions.push(AhkAction::Shell(shell_lines.join("\n")));
                }
            }
            continue;
        }
        
        if trimmed == "}" {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        
        if trimmed == "{" {
            depth += 1;
            continue;
        }
        
        // Parse individual action
        if let Ok(action) = self.parse_action(trimmed) {
            actions.push(action);
        }
    }
    
    eprintln!("DEBUG PARSER: Finished parsing hotkey, total actions: {}", actions.len());
    for (i, action) in actions.iter().enumerate() {
        eprintln!("DEBUG PARSER: Action {}: {:?}", i, action);
    }
    
    let action = if actions.len() == 1 {
        actions.into_iter().next().unwrap()
    } else {
        AhkAction::Block(actions)
    };
    
    Ok(Some(AhkHotkey {
        modifiers,
        key,
        action,
        context,
        is_wildcard,
    }))
}

    fn parse_action(&self, action_str: &str) -> Result<AhkAction, String> {
        let s = action_str.trim();

        // Handle WinActivate
        if let Some(rest) = s.strip_prefix("WinActivate(") {
            if let Some(content) = rest.strip_suffix(')') {
                let criteria = self.parse_window_criteria(content)?;
                return Ok(AhkAction::WinActivate(criteria));
            }
        }

        // Handle WinWaitActive with optional timeout: WinWaitActive("criteria", timeout_ms)
        if let Some(rest) = s.strip_prefix("WinWaitActive(") {
            if let Some(content) = rest.strip_suffix(')') {
                let parts: Vec<&str> = content.split(',').map(|s| s.trim()).collect();
                let criteria = self.parse_window_criteria(parts[0])?;
                let timeout_ms = if parts.len() > 1 {
                    parts[1].parse::<u64>().ok()
                } else {
                    None // None = infinite wait
                };
                return Ok(AhkAction::WinWaitActive { criteria, timeout_ms });
            }
        }

        // Handle WinClose
        if let Some(rest) = s.strip_prefix("WinClose(") {
            if let Some(content) = rest.strip_suffix(')') {
                let criteria = self.parse_window_criteria(content)?;
                return Ok(AhkAction::WinClose(criteria));
            }
        }

        // Handle Run with space: Run "command" or Run 'command'
        if let Some(rest) = s.strip_prefix("Run ") {
            let cmd = rest.trim().trim_matches(|c| c == '"' || c == '\'');
            let cmd = unescape_ahk_string(cmd);
            let parts: Vec<String> = cmd.split_whitespace().map(String::from).collect();
            return Ok(AhkAction::Run(parts));
        }

        // Handle Run with parentheses: Run("command")
        if let Some(rest) = s.strip_prefix("Run(") {
            if let Some(cmd) = rest.strip_suffix(')') {
                let cmd = cmd.trim().trim_matches(|c| c == '"' || c == '\'');
                let cmd = unescape_ahk_string(cmd);
                let parts: Vec<String> = cmd.split_whitespace().map(String::from).collect();
                return Ok(AhkAction::Run(parts));
            }
        }

        for prefix in ["SendInput(", "SendEvent(", "Send("] {
            if let Some(rest) = s.strip_prefix(prefix) {
                if let Some(content) = rest.strip_suffix(')') {
                    let keys = content.trim().trim_matches(|c| c == '"' || c == '\'');
                    let keys = unescape_ahk_string(keys);
                    return Ok(AhkAction::Send(keys));
                }
            }
        }

        for prefix in ["SendInput ", "SendEvent ", "Send "] {
            if let Some(rest) = s.strip_prefix(prefix) {
                let keys = rest.trim_matches(|c| c == '"' || c == '\'');
                let keys = unescape_ahk_string(keys);
                return Ok(AhkAction::Send(keys));
            }
        }

        if let Some(rest) = s.strip_prefix("Sleep ") {
            if let Ok(ms) = rest.trim().parse::<u64>() {
                return Ok(AhkAction::Sleep(ms));
            }
        }

        if s.starts_with("Media_") || s.starts_with("Volume_") {
            if let Some(key) = string_to_key(s) {
                return Ok(AhkAction::Remap(vec![key]));
            }
        }

        if let Some(key) = string_to_key(s) {
            return Ok(AhkAction::Remap(vec![key]));
        }

        Err(format!("Unknown action: {s}"))
    }
}

pub fn string_to_key(s: &str) -> Option<KeyCode> {
    match s.to_lowercase().as_str() {
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
        "0" => Some(KeyCode::KEY_0),
        "1" => Some(KeyCode::KEY_1),
        "2" => Some(KeyCode::KEY_2),
        "3" => Some(KeyCode::KEY_3),
        "4" => Some(KeyCode::KEY_4),
        "5" => Some(KeyCode::KEY_5),
        "6" => Some(KeyCode::KEY_6),
        "7" => Some(KeyCode::KEY_7),
        "8" => Some(KeyCode::KEY_8),
        "9" => Some(KeyCode::KEY_9),
        "space" => Some(KeyCode::KEY_SPACE),
        "enter" | "return" => Some(KeyCode::KEY_ENTER),
        "tab" => Some(KeyCode::KEY_TAB),
        "backspace" => Some(KeyCode::KEY_BACKSPACE),
        "delete" | "del" => Some(KeyCode::KEY_DELETE),
        "escape" | "esc" => Some(KeyCode::KEY_ESC),
        "capslock" => Some(KeyCode::KEY_CAPSLOCK),
        "up" => Some(KeyCode::KEY_UP),
        "down" => Some(KeyCode::KEY_DOWN),
        "left" => Some(KeyCode::KEY_LEFT),
        "right" => Some(KeyCode::KEY_RIGHT),
        "home" => Some(KeyCode::KEY_HOME),
        "end" => Some(KeyCode::KEY_END),
        "pageup" | "pgup" => Some(KeyCode::KEY_PAGEUP),
        "pagedown" | "pgdn" => Some(KeyCode::KEY_PAGEDOWN),
        "insert" => Some(KeyCode::KEY_INSERT),
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
        "f13" => Some(KeyCode::KEY_F13),
        "f14" => Some(KeyCode::KEY_F14),
        "f15" => Some(KeyCode::KEY_F15),
        "f16" => Some(KeyCode::KEY_F16),
        "f17" => Some(KeyCode::KEY_F17),
        "f18" => Some(KeyCode::KEY_F18),
        "f19" => Some(KeyCode::KEY_F19),
        "f20" => Some(KeyCode::KEY_F20),
        "f21" => Some(KeyCode::KEY_F21),
        "f22" => Some(KeyCode::KEY_F22),
        "f23" => Some(KeyCode::KEY_F23),
        "f24" => Some(KeyCode::KEY_F24),
        "media_play_pause" => Some(KeyCode::KEY_PLAYPAUSE),
        "media_next" => Some(KeyCode::KEY_NEXTSONG),
        "media_prev" => Some(KeyCode::KEY_PREVIOUSSONG),
        "media_stop" => Some(KeyCode::KEY_STOPCD),
        "volume_up" => Some(KeyCode::KEY_VOLUMEUP),
        "volume_down" => Some(KeyCode::KEY_VOLUMEDOWN),
        "volume_mute" => Some(KeyCode::KEY_MUTE),
        _ => None,
    }
}

pub fn parse_ahk_file(path: &Path) -> Result<AhkConfig, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

    let mut parser = AhkParser::new();
    parser.parse_file(&content)
}