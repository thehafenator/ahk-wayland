use regex::Regex;
use evdev::KeyCode;
use std::fs;
use std::path::Path;
use crate::ahk::types::*;

pub fn parse_ahk_file(path: &Path) -> Result<AhkConfig, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    parse_ahk_content(&content)
}

pub fn parse_ahk_content(content: &str) -> Result<AhkConfig, String> {
    let mut hotkeys = Vec::new();
    let mut hotstrings = Vec::new();
    let mut current_context = None;

    for line in content.lines() {
        let trimmed = line.trim();
        
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }

        if trimmed.starts_with("#HotIf") {
            current_context = parse_hotif(trimmed)?;
            continue;
        }

        if let Some(hs) = parse_hotstring(trimmed)? {
            let mut hotstring = hs;
            hotstring.context = current_context.clone();
            hotstrings.push(hotstring);
            continue;
        }

        if let Some(hk) = parse_hotkey(trimmed)? {
            let mut hotkey = hk;
            hotkey.context = current_context.clone();
            hotkeys.push(hotkey);
            continue;
        }
    }

    Ok(AhkConfig { hotkeys, hotstrings })
}

fn parse_hotif(line: &str) -> Result<Option<String>, String> {
    if line == "#HotIf" {
        return Ok(None);
    }
    let re = Regex::new(r#"#HotIf\s+(?:!)?WinActive\("([^"]+)"\)"#).unwrap();
    if let Some(caps) = re.captures(line) {
        return Ok(Some(caps[1].to_string()));
    }
    Ok(None)
}

fn parse_hotstring(line: &str) -> Result<Option<AhkHotstring>, String> {
    let immediate = Regex::new(r"^:\*:([^:]+)::(.+)$").unwrap();
    let word = Regex::new(r"^::([^:]+)::(.+)$").unwrap();
    if let Some(caps) = immediate.captures(line) {
        return Ok(Some(AhkHotstring {
            trigger: caps[1].to_string(),
            replacement: caps[2].to_string(),
            immediate: true,
            context: None,
        }));
    }
    if let Some(caps) = word.captures(line) {
        return Ok(Some(AhkHotstring {
            trigger: caps[1].to_string(),
            replacement: caps[2].to_string(),
            immediate: false,
            context: None,
        }));
    }
    Ok(None)
}

fn parse_hotkey(line: &str) -> Result<Option<AhkHotkey>, String> {
    let re = Regex::new(r"^([^:]+)::(.+)$").unwrap();
    if let Some(caps) = re.captures(line) {
        let combo = &caps[1];
        let action_str = &caps[2];
        let (modifiers, key) = parse_key_combo(combo)?;
        let action = parse_action(action_str)?;
        return Ok(Some(AhkHotkey { modifiers, key, action, context: None }));
    }
    Ok(None)
}

fn parse_key_combo(combo: &str) -> Result<(Vec<KeyCode>, KeyCode), String> {
    let mut modifiers = Vec::new();
    let mut chars = combo.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            '^' => { modifiers.push(KeyCode::KEY_LEFTCTRL); chars.next(); }
            '!' => { modifiers.push(KeyCode::KEY_LEFTALT); chars.next(); }
            '+' => { modifiers.push(KeyCode::KEY_LEFTSHIFT); chars.next(); }
            '#' => { modifiers.push(KeyCode::KEY_LEFTMETA); chars.next(); }
            _ => break,
        }
    }
    let key_str: String = chars.collect();
    let key = string_to_key(&key_str)?;
    Ok((modifiers, key))
}

fn string_to_key(s: &str) -> Result<KeyCode, String> {
    match s.to_uppercase().as_str() {
        "A" => Ok(KeyCode::KEY_A), "B" => Ok(KeyCode::KEY_B), "C" => Ok(KeyCode::KEY_C),
        "D" => Ok(KeyCode::KEY_D), "E" => Ok(KeyCode::KEY_E), "F" => Ok(KeyCode::KEY_F),
        "T" => Ok(KeyCode::KEY_T), "W" => Ok(KeyCode::KEY_W), "V" => Ok(KeyCode::KEY_V),
        "SPACE" => Ok(KeyCode::KEY_SPACE), "ENTER" => Ok(KeyCode::KEY_ENTER),
        _ => Err(format!("Unknown key: {}", s))
    }
}

fn parse_action(action_str: &str) -> Result<AhkAction, String> {
    let run_re = Regex::new(r#"^Run\s+"([^"]+)"#).unwrap();
    if let Some(caps) = run_re.captures(action_str) {
        let parts: Vec<String> = caps[1].split_whitespace().map(|s| s.to_string()).collect();
        return Ok(AhkAction::Run(parts));
    }
    Err(format!("Unknown action: {}", action_str))
}
