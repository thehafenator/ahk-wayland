use crate::action::Action;
use crate::client::WMClient;
use crate::config::application::OnlyOrNot;
use crate::config::key_press::{KeyPress, Modifier};
use crate::config::keymap::{build_override_table, OverrideEntry};
use crate::config::keymap_action::KeymapAction;
use crate::config::modmap_action::{Keys, ModmapAction, MultiPurposeKey, PressReleaseKey};
use crate::config::remap::Remap;
use crate::device::InputDeviceInfo;
use crate::event::{Event, KeyEvent, RelativeEvent};
use crate::hotstring;
use crate::Config;
use evdev::KeyCode as Key;
use lazy_static::lazy_static;
use log::debug;
use nix::sys::time::TimeSpec;
use nix::sys::timerfd::{Expiration, TimerFd, TimerSetTimeFlags};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::{Duration, Instant};

// This const is a value used to offset RELATIVE events' scancodes
// so that they correspond to the custom aliases created in config::key::parse_key.
// This offset also prevents resulting scancodes from corresponding to non-Xremap scancodes,
// to prevent conflating disguised relative events with other events.
pub const DISGUISED_EVENT_OFFSETTER: u16 = 59974;

// This const is defined a keycode for a configuration key used to match any key.
// It's the offset of XHIRES_LEFTSCROLL + 1
pub const KEY_MATCH_ANY: Key = Key(DISGUISED_EVENT_OFFSETTER + 26);

pub struct EventHandler {
    // Currently pressed modifier keys
    modifiers: HashSet<Key>,
    // Modifiers that are currently pressed but not in the source KeyPress
    extra_modifiers: HashSet<Key>,
    // Make sure the original event is released even if remapping changes while holding the key
    pressed_keys: HashMap<Key, Key>,
    // Client that interacts with the window manager.
    application_client: WMClient,
    application_cache: Option<String>,
    title_cache: Option<String>,
    // State machine for multi-purpose keys
    multi_purpose_keys: HashMap<Key, MultiPurposeKeyState>,
    // Current nested remaps
    override_remaps: Vec<HashMap<Key, Vec<OverrideEntry>>>,
    // Key triggered on a timeout of nested remaps
    override_timeout_key: Option<Vec<Key>>,
    // Trigger a timeout of nested remaps through select(2)
    override_timer: TimerFd,
    // { set_mode: String }
    mode: String,
    // { set_mark: true }
    mark_set: bool,
    // { escape_next_key: true }
    escape_next_key: bool,
    // keypress_delay_ms
    keypress_delay: Duration,
    // Buffered actions to be dispatched. TODO: Just return actions from each function instead of using this.
    actions: Vec<Action>,
    // Hotstring matching state
    hotstring_state: Option<hotstring::HotstringMatcherState>,
    hotstring_buffer: String,
}

struct TaggedAction {
    action: KeymapAction,
    exact_match: bool,
}

impl EventHandler {
    pub fn new(timer: TimerFd, mode: &str, keypress_delay: Duration, application_client: WMClient) -> EventHandler {
        EventHandler {
            modifiers: HashSet::new(),
            extra_modifiers: HashSet::new(),
            pressed_keys: HashMap::new(),
            application_client,
            application_cache: None,
            title_cache: None,
            multi_purpose_keys: HashMap::new(),
            override_remaps: vec![],
            override_timeout_key: None,
            override_timer: timer,
            mode: mode.to_string(),
            mark_set: false,
            escape_next_key: false,
            keypress_delay,
            actions: vec![],
            hotstring_state: None,
            hotstring_buffer: String::new(),
        }
    }

    // Handle an Event and return Actions. This should be the only public method of EventHandler.
    pub fn on_events(&mut self, events: &Vec<Event>, config: &Config) -> Result<Vec<Action>, Box<dyn Error>> {
        // a vector to collect mouse movement events to be able to send them all at once as one MouseMovementEventCollection.
        let mut mouse_movement_collection: Vec<RelativeEvent> = Vec::new();
        for event in events {
            match event {
                Event::KeyEvent(device, key_event) => {
                    self.on_key_event(key_event, config, device)?;
                }
                Event::RelativeEvent(device, relative_event) => {
                    self.on_relative_event(relative_event, &mut mouse_movement_collection, config, device)?
                }

                Event::OtherEvents(event) => self.send_action(Action::InputEvent(*event)),
                Event::OverrideTimeout => self.timeout_override()?,
            };
        }
        // if there is at least one mouse movement event, sending all of them as one MouseMovementEventCollection
        if !mouse_movement_collection.is_empty() {
            self.send_action(Action::MouseMovementEventCollection(mouse_movement_collection));
        }
        Ok(self.actions.drain(..).collect())
    }

    // Handle EventType::KEY

    // Convert key code to character for hotstring matching
    // Returns: Some(char) for valid characters, None for keys that should reset matcher
    fn key_to_char(&mut self, key: &Key) -> Option<String> {
        match *key {
            // Letters
            Key::KEY_A => Some("a".to_string()),
            Key::KEY_B => Some("b".to_string()),
            Key::KEY_C => Some("c".to_string()),
            Key::KEY_D => Some("d".to_string()),
            Key::KEY_E => Some("e".to_string()),
            Key::KEY_F => Some("f".to_string()),
            Key::KEY_G => Some("g".to_string()),
            Key::KEY_H => Some("h".to_string()),
            Key::KEY_I => Some("i".to_string()),
            Key::KEY_J => Some("j".to_string()),
            Key::KEY_K => Some("k".to_string()),
            Key::KEY_L => Some("l".to_string()),
            Key::KEY_M => Some("m".to_string()),
            Key::KEY_N => Some("n".to_string()),
            Key::KEY_O => Some("o".to_string()),
            Key::KEY_P => Some("p".to_string()),
            Key::KEY_Q => Some("q".to_string()),
            Key::KEY_R => Some("r".to_string()),
            Key::KEY_S => Some("s".to_string()),
            Key::KEY_T => Some("t".to_string()),
            Key::KEY_U => Some("u".to_string()),
            Key::KEY_V => Some("v".to_string()),
            Key::KEY_W => Some("w".to_string()),
            Key::KEY_X => Some("x".to_string()),
            Key::KEY_Y => Some("y".to_string()),
            Key::KEY_Z => Some("z".to_string()),

            // Numbers
            Key::KEY_0 => Some("0".to_string()),
            Key::KEY_1 => Some("1".to_string()),
            Key::KEY_2 => Some("2".to_string()),
            Key::KEY_3 => Some("3".to_string()),
            Key::KEY_4 => Some("4".to_string()),
            Key::KEY_5 => Some("5".to_string()),
            Key::KEY_6 => Some("6".to_string()),
            Key::KEY_7 => Some("7".to_string()),
            Key::KEY_8 => Some("8".to_string()),
            Key::KEY_9 => Some("9".to_string()),

            // Punctuation
            Key::KEY_DOT => Some(".".to_string()),
            Key::KEY_COMMA => Some(",".to_string()),
            Key::KEY_SEMICOLON => Some(";".to_string()),
            Key::KEY_SLASH => Some("/".to_string()),
            Key::KEY_APOSTROPHE => Some("'".to_string()),
            Key::KEY_MINUS => Some("-".to_string()),
            Key::KEY_EQUAL => Some("=".to_string()),
            Key::KEY_LEFTBRACE => Some("[".to_string()),
            Key::KEY_RIGHTBRACE => Some("]".to_string()),
            Key::KEY_BACKSLASH => Some("\\".to_string()),
            Key::KEY_GRAVE => Some("`".to_string()),

            // Whitespace
            Key::KEY_SPACE => Some(" ".to_string()),
            Key::KEY_TAB => Some("\t".to_string()),
            Key::KEY_ENTER => Some("\n".to_string()),

            // Backspace - remove from buffer
            Key::KEY_BACKSPACE => {
                self.hotstring_state = None;
                self.hotstring_buffer.clear();
                if !self.hotstring_buffer.is_empty() {
                    self.hotstring_buffer.pop();
                }
                None
            }

            // These keys should reset the matcher (return None)
            // Modifiers
            Key::KEY_LEFTSHIFT
            | Key::KEY_RIGHTSHIFT
            | Key::KEY_LEFTCTRL
            | Key::KEY_RIGHTCTRL
            | Key::KEY_LEFTALT
            | Key::KEY_RIGHTALT
            | Key::KEY_LEFTMETA
            | Key::KEY_RIGHTMETA => None,

            // Navigation keys - these should reset
            Key::KEY_UP
            | Key::KEY_DOWN
            | Key::KEY_LEFT
            | Key::KEY_RIGHT
            | Key::KEY_HOME
            | Key::KEY_END
            | Key::KEY_PAGEUP
            | Key::KEY_PAGEDOWN => None,

            // Function keys - reset
            Key::KEY_F1
            | Key::KEY_F2
            | Key::KEY_F3
            | Key::KEY_F4
            | Key::KEY_F5
            | Key::KEY_F6
            | Key::KEY_F7
            | Key::KEY_F8
            | Key::KEY_F9
            | Key::KEY_F10
            | Key::KEY_F11
            | Key::KEY_F12 => None,

            // Other control keys - reset
            Key::KEY_ESC | Key::KEY_DELETE | Key::KEY_INSERT | Key::KEY_CAPSLOCK => None,

            _ => None,
        }
    }

    fn on_key_event(
        &mut self,
        event: &KeyEvent,
        config: &Config,
        device: &InputDeviceInfo,
    ) -> Result<bool, Box<dyn Error>> {
        self.application_cache = None; // expire cache
        self.title_cache = None; // expire cache
        let key = Key::new(event.code());
        println!("Looking for key: {:?}, modifiers: {:?}", key, self.modifiers);

        if key.code() < DISGUISED_EVENT_OFFSETTER {
            debug!("=> {}: {:?}", event.value(), &key);
        }

        // Apply modmap
        let mut key_values = if let Some(key_action) = self.find_modmap(config, &key, device) {
            self.dispatch_keys(key_action, key, event.value())?
        } else {
            vec![(key, event.value())]
        };
        self.maintain_pressed_keys(key, event.value(), &mut key_values);
        if !self.multi_purpose_keys.is_empty() {
            key_values = self.flush_timeout_keys(key_values);
        }

        let mut send_original_relative_event = false;

        // Apply keymap
        for (key, value) in key_values.into_iter() {
            // Handle virtual modifiers
            if config.virtual_modifiers.contains(&key) {
                self.update_modifier(key, value);
                continue;
            }
            
            // Handle real modifier keys - update state AND send the key
            if MODIFIER_KEYS.contains(&key) {
                self.update_modifier(key, value);
                self.send_key(&key, value);
                continue;
            }
            
            // Handle non-modifier keys
            if is_pressed(value) {
                if self.escape_next_key {
                    self.escape_next_key = false;
                }

                // === 1. FIRST: Check regular hotkeys (including ^t, Ctrl+anything, etc.) ===
                else if let Some(actions) = self.find_keymap(config, &key, device)? {
                    self.dispatch_actions(&actions, &key)?;
                    continue;
                }
                if let Some(actions) = self.find_keymap(config, &KEY_MATCH_ANY, device)? {
                    self.dispatch_actions(&actions, &KEY_MATCH_ANY)?;
                    continue;
                }

                // === 2. SECOND: Only if no hotkey matched – process hotstrings ===
                if let Some(matcher) = &config.hotstring_matcher {
                    match self.key_to_char(&key) {
                        Some(ch) => {
                            // Valid character - process through matcher
                            self.hotstring_buffer.push_str(&ch);
                            let (new_state, matched) = matcher.process(self.hotstring_state.as_ref(), &ch);
                            self.hotstring_state = Some(new_state);

                            if let Some(hotstring_match) = matched {
                                // Delete just the trigger
                                let chars_to_delete = hotstring_match.trigger.len();

                                if hotstring_match.execute {
                                    // X option: execute as command
                                    for _ in 0..chars_to_delete {
                                        self.send_key(&Key::KEY_BACKSPACE, PRESS);
                                        self.send_key(&Key::KEY_BACKSPACE, RELEASE);
                                    }

                                    if let Some(rest) = hotstring_match.replacement.strip_prefix("Run(") {
                                        if let Some(cmd) = rest.strip_suffix(')') {
                                            let cmd = cmd.trim().trim_matches(|c| c == '"' || c == '\'');
                                            let command: Vec<String> = if cmd.starts_with("http://") || cmd.starts_with("https://") {
                                                vec!["xdg-open".to_string(), cmd.to_string()]
                                            } else {
                                                cmd.split_whitespace().map(String::from).collect()
                                            };
                                            self.send_action(Action::Command(command));
                                        }
                                    }
                                } else {
                                    // Regular text expansion
                                    let final_replacement = hotstring_match.replacement.clone();
                                    self.send_action(Action::TextExpansion {
                                        trigger_len: chars_to_delete,
                                        replacement: final_replacement,
                                        add_space: !hotstring_match.omit_char && !hotstring_match.immediate,
                                    });
                                }

                                self.hotstring_buffer.clear();
                                self.hotstring_state = None;
                                continue; // hotstring matched – suppress original key
                            }
                        }
                        None => {
                            // Reset matcher on non-character keys
                            self.hotstring_state = None;
                            self.hotstring_buffer.clear();
                        }
                    }
                }

                // If nothing matched above, pass through the original key
                self.send_key(&key, value);
            } else {
                // Release or repeat – just send
                self.send_key(&key, value);
            }

            // Check for disguised relative events
            if key.code() >= DISGUISED_EVENT_OFFSETTER && (key.code(), value) == (event.code(), event.value()) {
                send_original_relative_event = true;
                continue;
            }
        }

        Ok(send_original_relative_event)
    }

    // Handle EventType::RELATIVE
    fn on_relative_event(
        &mut self,
        event: &RelativeEvent,
        mouse_movement_collection: &mut Vec<RelativeEvent>,
        config: &Config,
        device: &InputDeviceInfo,
    ) -> Result<(), Box<dyn Error>> {
        const RELEASE: i32 = 0;
        const PRESS: i32 = 1;

        let key = match event.value {
            1..=i32::MAX => (event.code * 2) + DISGUISED_EVENT_OFFSETTER,
            i32::MIN..=-1 => (event.code * 2) + 1 + DISGUISED_EVENT_OFFSETTER,
            0 => {
                println!("This event has a value of zero : {event:?}");
                (event.code * 2) + DISGUISED_EVENT_OFFSETTER
            }
        };

        match self.on_key_event(&KeyEvent::new_with(key, PRESS), config, device)? {
            true => {
                let action = RelativeEvent::new_with(event.code, event.value);
                if event.code <= 2 {
                    mouse_movement_collection.push(action);
                } else {
                    self.send_action(Action::RelativeEvent(action));
                }
            }
            false => {}
        }

        self.on_key_event(&KeyEvent::new_with(key, RELEASE), config, device)?;

        Ok(())
    }

    fn timeout_override(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(keys) = &self.override_timeout_key.take() {
            for key in keys {
                self.send_key(key, PRESS);
                self.send_key(key, RELEASE);
            }
        }
        self.remove_override()
    }

    fn remove_override(&mut self) -> Result<(), Box<dyn Error>> {
        self.override_timer.unset()?;
        self.override_remaps.clear();
        self.override_timeout_key = None;
        Ok(())
    }

    fn send_keys(&mut self, keys: &Vec<Key>, value: i32) {
        for key in keys {
            self.send_key(key, value);
        }
    }

    fn send_key(&mut self, key: &Key, value: i32) {
        let event = KeyEvent::new_with(key.code(), value);
        self.send_action(Action::KeyEvent(event));
    }

    fn send_action(&mut self, action: Action) {
        self.actions.push(action);
    }

    // Repeat/Release what's originally pressed even if remapping changes while holding it
    fn maintain_pressed_keys(&mut self, key: Key, value: i32, events: &mut [(Key, i32)]) {
        // Not handling multi-purpose keys for now; too complicated
        if events.len() != 1 || value != events[0].1 {
            return;
        }

        let event = events[0];
        if value == PRESS {
            self.pressed_keys.insert(key, event.0);
        } else {
            if let Some(original_key) = self.pressed_keys.get(&key) {
                events[0].0 = *original_key;
            }
            if value == RELEASE {
                self.pressed_keys.remove(&key);
            }
        }
    }

    fn dispatch_keys(
        &mut self,
        key_action: ModmapAction,
        key: Key,
        value: i32,
    ) -> Result<Vec<(Key, i32)>, Box<dyn Error>> {
        let keys = match key_action {
            ModmapAction::Keys(modmap_keys) => modmap_keys
                .into_vec()
                .into_iter()
                .map(|modmap_key| (modmap_key, value))
                .collect(),
            ModmapAction::MultiPurposeKey(MultiPurposeKey {
                held,
                alone,
                alone_timeout,
                free_hold,
            }) => {
                match value {
                    PRESS => {
                        self.multi_purpose_keys.insert(
                            key,
                            MultiPurposeKeyState {
                                held,
                                alone,
                                alone_timeout_at: if free_hold {
                                    None
                                } else {
                                    Some(Instant::now() + alone_timeout)
                                },
                                held_down: false,
                            },
                        );
                        return Ok(vec![]); // delay the press
                    }
                    REPEAT => {
                        if let Some(state) = self.multi_purpose_keys.get_mut(&key) {
                            return Ok(state.repeat());
                        }
                    }
                    RELEASE => {
                        if let Some(state) = self.multi_purpose_keys.remove(&key) {
                            return Ok(state.release());
                        }
                    }
                    _ => panic!("unexpected key event value: {value}"),
                }
                // fallthrough on state discrepancy
                vec![(key, value)]
            }
            ModmapAction::PressReleaseKey(PressReleaseKey {
                skip_key_event,
                press,
                repeat,
                release,
            }) => {
                let actions_to_dispatch = match value {
                    PRESS => press,
                    RELEASE => release,
                    _ => repeat,
                };
                self.dispatch_actions(
                    &actions_to_dispatch
                        .into_iter()
                        .map(|action| TaggedAction {
                            action,
                            exact_match: false,
                        })
                        .collect(),
                    &key,
                )?;

                match skip_key_event {
                    true => vec![],
                    false => vec![(key, value)],
                }
            }
        };
        Ok(keys)
    }

    fn flush_timeout_keys(&mut self, key_values: Vec<(Key, i32)>) -> Vec<(Key, i32)> {
        let mut flush = false;
        for (_, value) in key_values.iter() {
            if *value == PRESS {
                flush = true;
                break;
            }
        }

        if flush {
            let mut flushed: Vec<(Key, i32)> = vec![];
            for (_, state) in self.multi_purpose_keys.iter_mut() {
                flushed.extend(state.force_held());
            }

            let flushed_presses: HashSet<Key> = flushed
                .iter()
                .filter_map(|(k, v)| (*v == PRESS).then_some(*k))
                .collect();
            let key_values: Vec<(Key, i32)> = key_values
                .into_iter()
                .filter(|(key, value)| !(*value == PRESS && flushed_presses.contains(key)))
                .collect();

            flushed.extend(key_values);
            flushed
        } else {
            key_values
        }
    }

    fn find_modmap(&mut self, config: &Config, key: &Key, device: &InputDeviceInfo) -> Option<ModmapAction> {
        for modmap in &config.modmap {
            if let Some(key_action) = modmap.remap.get(key) {
                if let Some(window_matcher) = &modmap.window {
                    if !self.match_window(window_matcher) {
                        continue;
                    }
                }
                if let Some(application_matcher) = &modmap.application {
                    if !self.match_application(application_matcher) {
                        continue;
                    }
                }
                if let Some(device_matcher) = &modmap.device {
                    if !self.match_device(device_matcher, device) {
                        continue;
                    }
                }
                if let Some(modes) = &modmap.mode {
                    if !modes.contains(&self.mode) {
                        continue;
                    }
                }
                return Some(key_action.clone());
            }
        }
        None
    }

    fn find_keymap(
        &mut self,
        config: &Config,
        key: &Key,
        device: &InputDeviceInfo,
    ) -> Result<Option<Vec<TaggedAction>>, Box<dyn Error>> {
        if !self.override_remaps.is_empty() {
            let entries: Vec<OverrideEntry> = self
                .override_remaps
                .iter()
                .flat_map(|map| map.get(key).cloned().unwrap_or_default())
                .collect();

            if !entries.is_empty() {
                self.remove_override()?;

                for exact_match in [true, false] {
                    let mut remaps = vec![];
                    for entry in &entries {
                        if entry.exact_match && !exact_match {
                            continue;
                        }
                        let (extra_modifiers, missing_modifiers) = self.diff_modifiers(&entry.modifiers);
                        if (exact_match && !extra_modifiers.is_empty()) || !missing_modifiers.is_empty() {
                            continue;
                        }

                        let actions = with_extra_modifiers(&entry.actions, &extra_modifiers, entry.exact_match);
                        let is_remap = is_remap(&entry.actions);

                        if remaps.is_empty() && !is_remap {
                            return Ok(Some(actions));
                        } else if is_remap {
                            remaps.extend(actions);
                        }
                    }
                    if !remaps.is_empty() {
                        return Ok(Some(remaps));
                    }
                }
            }
            self.timeout_override()?;
        }

        if let Some(entries) = config.keymap_table.get(key) {
            for exact_match in [true, false] {
                let mut remaps = vec![];
                for entry in entries {
                    if entry.exact_match && !exact_match {
                        continue;
                    }
                    let (extra_modifiers, missing_modifiers) = self.diff_modifiers(&entry.modifiers);
                    if (exact_match && !extra_modifiers.is_empty()) || !missing_modifiers.is_empty() {
                        continue;
                    }
                    if let Some(window_matcher) = &entry.title {
                        if !self.match_window(window_matcher) {
                            continue;
                        }
                    }

                    if let Some(application_matcher) = &entry.application {
                        if !self.match_application(application_matcher) {
                            continue;
                        }
                    }
                    if let Some(device_matcher) = &entry.device {
                        if !self.match_device(device_matcher, device) {
                            continue;
                        }
                    }
                    if let Some(modes) = &entry.mode {
                        if !modes.contains(&self.mode) {
                            continue;
                        }
                    }

                    let actions = with_extra_modifiers(&entry.actions, &extra_modifiers, entry.exact_match);
                    let is_remap = is_remap(&entry.actions);

                    if remaps.is_empty() && !is_remap {
                        return Ok(Some(actions));
                    } else if is_remap {
                        remaps.extend(actions)
                    }
                }
                if !remaps.is_empty() {
                    return Ok(Some(remaps));
                }
            }
        }
        Ok(None)
    }

    fn dispatch_actions(&mut self, actions: &Vec<TaggedAction>, key: &Key) -> Result<(), Box<dyn Error>> {
        for action in actions {
            self.dispatch_action(action, key)?;
        }
        Ok(())
    }

    fn dispatch_action(&mut self, action: &TaggedAction, key: &Key) -> Result<(), Box<dyn Error>> {
        match &action.action {
            KeymapAction::KeyPressAndRelease(key_press) => self.send_key_press_and_release(key_press),
            KeymapAction::KeyPress(key) => self.send_key(key, PRESS),
            KeymapAction::KeyRepeat(key) => self.send_key(key, REPEAT),
            KeymapAction::KeyRelease(key) => self.send_key(key, RELEASE),
            KeymapAction::Remap(Remap {
                remap,
                timeout,
                timeout_key,
            }) => {
                let set_timeout = self.override_remaps.is_empty();
                self.override_remaps
                    .push(build_override_table(remap, action.exact_match));

                if set_timeout {
                    if let Some(timeout) = timeout {
                        let expiration = Expiration::OneShot(TimeSpec::from_duration(*timeout));
                        self.override_timer.unset()?;
                        self.override_timer.set(expiration, TimerSetTimeFlags::empty())?;
                        self.override_timeout_key = timeout_key.clone().or_else(|| Some(vec![*key]))
                    }
                }
            }
            KeymapAction::Launch(command) => self.run_command(command.clone()),
            KeymapAction::SetMode(mode) => {
                self.mode = mode.clone();
                println!("mode: {mode}");
            }
            KeymapAction::SetMark(set) => self.mark_set = *set,
            KeymapAction::WithMark(key_press) => self.send_key_press_and_release(&self.with_mark(key_press)),
            KeymapAction::EscapeNextKey(escape_next_key) => self.escape_next_key = *escape_next_key,
            KeymapAction::Sleep(millis) => self.send_action(Action::Delay(Duration::from_millis(*millis))),
            KeymapAction::SetExtraModifiers(keys) => {
                self.extra_modifiers.clear();
                for key in keys {
                    self.extra_modifiers.insert(*key);
                }
            }
            KeymapAction::AhkInterpreted(ahk_action) => {
    // Save currently held modifiers
    let held_modifiers: Vec<Key> = self.modifiers.iter().copied().collect();
    
    // Virtually release them (send release events)
    for modifier in &held_modifiers {
        self.send_key(modifier, RELEASE);
    }
    
    // Execute interpreter
    let mut interpreter = crate::ahk::interpreter::AhkInterpreter::new(&mut self.application_client);
    match interpreter.execute(ahk_action) {
        Ok(interp_actions) => {
            for action in interp_actions {
                self.send_action(action);
            }
        }
        Err(e) => eprintln!("ERROR: AHK interpreter failed: {}", e),
    }
    
    // Restore modifiers if they're still physically held
    // (This happens automatically when user releases them physically)
    // Just update internal state to match
    for modifier in &held_modifiers {
        if self.is_physically_held(modifier) {
            self.send_key(modifier, PRESS);
        }
    }
}
        }
        Ok(())
    }

    fn send_key_press_and_release(&mut self, key_press: &KeyPress) {
        let (mut extra_modifiers, mut missing_modifiers) = self.diff_modifiers(&key_press.modifiers);
        extra_modifiers.retain(|key| MODIFIER_KEYS.contains(key) && !self.extra_modifiers.contains(key));
        missing_modifiers.retain(|key| MODIFIER_KEYS.contains(key));

        self.send_keys(&missing_modifiers, PRESS);
        self.send_keys(&extra_modifiers, RELEASE);

        self.send_key(&key_press.key, PRESS);
        self.send_key(&key_press.key, RELEASE);

        self.send_action(Action::Delay(self.keypress_delay));

        self.send_keys(&extra_modifiers, PRESS);
        self.send_action(Action::Delay(self.keypress_delay));
        self.send_keys(&missing_modifiers, RELEASE);
    }

    fn with_mark(&self, key_press: &KeyPress) -> KeyPress {
        if self.mark_set && !self.match_modifier(&Modifier::Shift) {
            let mut modifiers = key_press.modifiers.clone();
            modifiers.push(Modifier::Shift);
            KeyPress {
                key: key_press.key,
                modifiers,
            }
        } else {
            key_press.clone()
        }
    }

    fn run_command(&mut self, command: Vec<String>) {
        self.send_action(Action::Command(command));
    }

    fn diff_modifiers(&self, modifiers: &[Modifier]) -> (Vec<Key>, Vec<Key>) {
        let extra_modifiers: Vec<Key> = self
            .modifiers
            .iter()
            .filter(|modifier| !contains_modifier(modifiers, modifier))
            .copied()
            .collect();
        let missing_modifiers: Vec<Key> = modifiers
            .iter()
            .filter_map(|modifier| {
                if self.match_modifier(modifier) {
                    None
                } else {
                    match modifier {
                        Modifier::Shift => Some(Key::KEY_LEFTSHIFT),
                        Modifier::Control => Some(Key::KEY_LEFTCTRL),
                        Modifier::Alt => Some(Key::KEY_LEFTALT),
                        Modifier::Windows => Some(Key::KEY_LEFTMETA),
                        Modifier::Key(key) => Some(*key),
                    }
                }
            })
            .collect();
        (extra_modifiers, missing_modifiers)
    }

    fn match_modifier(&self, modifier: &Modifier) -> bool {
        match modifier {
            Modifier::Shift => {
                self.modifiers.contains(&Key::KEY_LEFTSHIFT) || self.modifiers.contains(&Key::KEY_RIGHTSHIFT)
            }
            Modifier::Control => {
                self.modifiers.contains(&Key::KEY_LEFTCTRL) || self.modifiers.contains(&Key::KEY_RIGHTCTRL)
            }
            Modifier::Alt => self.modifiers.contains(&Key::KEY_LEFTALT) || self.modifiers.contains(&Key::KEY_RIGHTALT),
            Modifier::Windows => {
                self.modifiers.contains(&Key::KEY_LEFTMETA) || self.modifiers.contains(&Key::KEY_RIGHTMETA)
            }
            Modifier::Key(key) => self.modifiers.contains(key),
        }
    }

    fn match_window(&mut self, window_matcher: &OnlyOrNot) -> bool {
        if self.title_cache.is_none() {
            match self.application_client.current_window() {
                Some(title) if !title.is_empty() => self.title_cache = Some(title),
                _ => {
                    if let Ok(output) = std::process::Command::new("kdotool")
                        .arg("getactivewindow")
                        .arg("getwindowname")
                        .output()
                    {
                        if output.status.success() {
                            let title = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            self.title_cache = Some(title);
                        } else {
                            self.title_cache = Some(String::new());
                        }
                    } else {
                        self.title_cache = Some(String::new());
                    }
                }
            }
        }

        if let Some(title) = &self.title_cache {
            if let Some(title_only) = &window_matcher.only {
                return title_only.iter().any(|m| m.matches(title));
            }
            if let Some(title_not) = &window_matcher.not {
                return title_not.iter().all(|m| !m.matches(title));
            }
        }
        false
    }

    fn match_application(&mut self, application_matcher: &OnlyOrNot) -> bool {
        if self.application_cache.is_none() {
            match self.application_client.current_application() {
                Some(application) if !application.is_empty() => self.application_cache = Some(application),
                _ => {
                    if let Ok(output) = std::process::Command::new("kdotool")
                        .arg("getactivewindow")
                        .arg("getwindowclassname")
                        .output()
                    {
                        if output.status.success() {
                            let application = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            self.application_cache = Some(application);
                        } else {
                            self.application_cache = Some(String::new());
                        }
                    } else {
                        self.application_cache = Some(String::new());
                    }
                }
            }
        }

        if let Some(application) = &self.application_cache {
            if let Some(application_only) = &application_matcher.only {
                return application_only.iter().any(|m| m.matches(application));
            }
            if let Some(application_not) = &application_matcher.not {
                return application_not.iter().all(|m| !m.matches(application));
            }
        }
        false
    }

    fn match_device(&self, device_matcher: &crate::config::device::Device, device: &InputDeviceInfo) -> bool {
        if let Some(device_only) = &device_matcher.only {
            return device_only.iter().any(|m| device.matches(m));
        }
        if let Some(device_not) = &device_matcher.not {
            return device_not.iter().all(|m| !device.matches(m));
        }
        false
    }

    fn update_modifier(&mut self, key: Key, value: i32) {
        if value == PRESS {
            self.modifiers.insert(key);
        } else if value == RELEASE {
            self.modifiers.remove(&key);
        }
    }

    fn is_physically_held(&self, key: &Key) -> bool {
    // Check if the key is currently in our modifiers set
    // This represents the physical state
    self.modifiers.contains(key)
}


}

fn is_remap(actions: &[KeymapAction]) -> bool {
    if actions.is_empty() {
        return false;
    }

    actions.iter().all(|x| matches!(x, KeymapAction::Remap(..)))
}

fn with_extra_modifiers(actions: &[KeymapAction], extra_modifiers: &[Key], exact_match: bool) -> Vec<TaggedAction> {
    let mut result: Vec<TaggedAction> = vec![];
    if !extra_modifiers.is_empty() {
        result.push(TaggedAction {
            action: KeymapAction::SetExtraModifiers(extra_modifiers.to_vec()),
            exact_match,
        });
    }
    result.extend(actions.iter().map(|action| TaggedAction {
        action: action.clone(),
        exact_match,
    }));
    if !extra_modifiers.is_empty() {
        result.push(TaggedAction {
            action: KeymapAction::SetExtraModifiers(vec![]),
            exact_match,
        });
    }
    result
}

fn contains_modifier(modifiers: &[Modifier], key: &Key) -> bool {
    for modifier in modifiers {
        if match modifier {
            Modifier::Shift => key == &Key::KEY_LEFTSHIFT || key == &Key::KEY_RIGHTSHIFT,
            Modifier::Control => key == &Key::KEY_LEFTCTRL || key == &Key::KEY_RIGHTCTRL,
            Modifier::Alt => key == &Key::KEY_LEFTALT || key == &Key::KEY_RIGHTALT,
            Modifier::Windows => key == &Key::KEY_LEFTMETA || key == &Key::KEY_RIGHTMETA,
            Modifier::Key(modifier_key) => key == modifier_key,
        } {
            return true;
        }
    }
    false
}

lazy_static! {
    static ref MODIFIER_KEYS: [Key; 8] = [
        Key::KEY_LEFTSHIFT,
        Key::KEY_RIGHTSHIFT,
        Key::KEY_LEFTCTRL,
        Key::KEY_RIGHTCTRL,
        Key::KEY_LEFTALT,
        Key::KEY_RIGHTALT,
        Key::KEY_LEFTMETA,
        Key::KEY_RIGHTMETA,
    ];
}

fn is_pressed(value: i32) -> bool {
    value == PRESS || value == REPEAT
}

const RELEASE: i32 = 0;
const PRESS: i32 = 1;
const REPEAT: i32 = 2;

#[derive(Debug)]
struct MultiPurposeKeyState {
    held: Keys,
    alone: Keys,
    alone_timeout_at: Option<Instant>,
    held_down: bool,
}

impl MultiPurposeKeyState {
    fn repeat(&mut self) -> Vec<(Key, i32)> {
        match self.alone_timeout_at {
            Some(alone_timeout_at) if Instant::now() < alone_timeout_at => {
                vec![]
            }
            Some(_) => {
                self.alone_timeout_at = None;
                self.held_down = true;
                let mut keys = self.held.clone().into_vec();
                keys.sort_by(modifiers_first);
                keys.into_iter().map(|key| (key, PRESS)).collect()
            }
            None => {
                let mut keys = self.held.clone().into_vec();
                keys.sort_by(modifiers_first);
                keys.into_iter().map(|key| (key, REPEAT)).collect()
            }
        }
    }

    fn release(&self) -> Vec<(Key, i32)> {
        match self.alone_timeout_at {
            Some(alone_timeout_at) if Instant::now() < alone_timeout_at => self.press_and_release(&self.alone),
            Some(_) => self.press_and_release(&self.held),
            None => match self.held_down {
                true => {
                    let mut release_keys = self.held.clone().into_vec();
                    release_keys.sort_by(modifiers_last);
                    release_keys.into_iter().map(|key| (key, RELEASE)).collect()
                }
                false => self.press_and_release(&self.alone),
            },
        }
    }

    fn force_held(&mut self) -> Vec<(Key, i32)> {
        let press = match self.alone_timeout_at {
            Some(_) => {
                self.alone_timeout_at = None;
                self.held_down = true;
                true
            }
            None => {
                if !self.held_down {
                    self.held_down = true;
                    true
                } else {
                    false
                }
            }
        };

        if press {
            let mut keys = self.held.clone().into_vec();
            keys.sort_by(modifiers_first);
            keys.into_iter().map(|key| (key, PRESS)).collect()
        } else {
            vec![]
        }
    }

    fn press_and_release(&self, keys_to_use: &Keys) -> Vec<(Key, i32)> {
        let mut release_keys = keys_to_use.clone().into_vec();
        release_keys.sort_by(modifiers_last);
        let release_events: Vec<(Key, i32)> = release_keys.into_iter().map(|key| (key, RELEASE)).collect();

        let mut press_keys = keys_to_use.clone().into_vec();
        press_keys.sort_by(modifiers_first);
        let mut events: Vec<(Key, i32)> = press_keys.into_iter().map(|key| (key, PRESS)).collect();
        events.extend(release_events);
        events
    }
}

fn modifiers_first(a: &Key, b: &Key) -> Ordering {
    if MODIFIER_KEYS.contains(a) {
        if MODIFIER_KEYS.contains(b) {
            Ordering::Equal
        } else {
            Ordering::Less
        }
    } else if MODIFIER_KEYS.contains(b) {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

fn modifiers_last(a: &Key, b: &Key) -> Ordering {
    modifiers_first(a, b).reverse()
}