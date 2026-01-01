use evdev::{uinput::VirtualDevice, EventType, InputEvent, KeyCode as Key};
use fork::{fork, setsid, Fork};
use log::debug;
use log::error;
use nix::sys::signal;
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet};
use std::process::{exit, Command, Stdio};

use crate::action::Action;
use crate::event::{KeyEvent, KeyValue, RelativeEvent};
use crate::ahk::interpreter::AhkInterpreter;  // add import

pub struct ActionDispatcher<'a> {
    device: VirtualDevice,
    sigaction_set: bool,
    interpreter: &'a mut AhkInterpreter<'a>,
}

// impl ActionDispatcher {
    // pub fn new(device: VirtualDevice) -> ActionDispatcher {
    //     ActionDispatcher {
    //         device,
    //         sigaction_set: false,
    //     }
    // }
impl<'a> ActionDispatcher<'a> {
pub fn new(device: VirtualDevice, interpreter: &'a mut AhkInterpreter<'a>) -> Self {    ActionDispatcher {
        device,
        sigaction_set: false,
        interpreter,
    }
}

    pub fn on_action(&mut self, action: Action) -> anyhow::Result<()> {
        match action {
            Action::KeyEvent(key_event) => self.on_key_event(key_event)?,
            Action::RelativeEvent(relative_event) => self.on_relative_event(relative_event)?,
            Action::MouseMovementEventCollection(mouse_movement_events) => {
                self.send_mousemovement_event_batch(mouse_movement_events)?;
            }
            Action::InputEvent(event) => self.send_event(event)?,
            Action::Command(command) => self.run_command(command),
            Action::Delay(_) => {}   

            Action::TextExpansion { trigger_len, replacement, add_space } => {
            let final_text = if add_space {
                format!("{}\u{00A0}", replacement)
            } else {
                replacement.clone()
            };

            // Delete trigger
            for _ in 0..trigger_len {
                self.on_key_event(KeyEvent::new(Key::KEY_BACKSPACE, KeyValue::Press))?;
                self.on_key_event(KeyEvent::new(Key::KEY_BACKSPACE, KeyValue::Release))?;
            }

            // Copy to clipboard
            crate::ahk::WaylandTextInjector::copy_to_clipboard(&final_text)?;
            // thread::sleep(Duration::from_millis(0));

            self.on_key_event(KeyEvent::new(Key::KEY_LEFTCTRL, KeyValue::Press))?;
            self.on_key_event(KeyEvent::new(Key::KEY_V, KeyValue::Press))?;
            self.on_key_event(KeyEvent::new(Key::KEY_V, KeyValue::Release))?;
            self.on_key_event(KeyEvent::new(Key::KEY_LEFTCTRL, KeyValue::Release))?;
            // Use the same interpreter that hotkeys use
            // ; if not working, make sure to have wl-clipboard installed
        
            // let paste_actions = self.interpreter
            // .execute(&AhkAction::Send("^v".to_string()))
            // .map_err(|e| anyhow!("AHK interpreter error: {}", e))?;
            // for action in paste_actions {
            //     self.on_action(action)?;  // recurse safely – only KeyEvent comes out
            // }
        }



        }
        Ok(())
    }

fn on_key_event(&mut self, event: KeyEvent) -> std::io::Result<()> {
    let value = event.value();  // directly i32, no match needed
    let ev = InputEvent::new(EventType::KEY.0, event.code(), value);
    self.device.emit(&[ev])
}

fn on_relative_event(&mut self, event: RelativeEvent) -> std::io::Result<()> {
    let ev = InputEvent::new(EventType::RELATIVE.0, event.code, event.value);
    self.device.emit(&[ev])
}

    fn send_mousemovement_event_batch(&mut self, eventbatch: Vec<RelativeEvent>) -> std::io::Result<()> {
        let mut batch = Vec::new();
        for mouse in eventbatch {
            batch.push(InputEvent::new(EventType::RELATIVE.0, mouse.code, mouse.value));
        }
        self.device.emit(&batch)
    }

    fn send_event(&mut self, event: InputEvent) -> std::io::Result<()> {
        if event.event_type() == EventType::KEY {
            debug!("{}: {:?}", event.value(), Key::new(event.code()))
        }
        self.device.emit(&[event])
    }

    fn run_command(&mut self, command: Vec<String>) {
        if !self.sigaction_set {
            let sig_action = SigAction::new(SigHandler::SigDfl, SaFlags::SA_NOCLDWAIT, SigSet::empty());
            unsafe {
                sigaction(signal::SIGCHLD, &sig_action).expect("Failed to register SIGCHLD handler");
            }
            self.sigaction_set = true;
        }

        debug!("Running command: {command:?}");
        match fork() {
            Ok(Fork::Child) => {
                match fork() {
                    Ok(Fork::Child) => {
                        setsid().expect("Failed to setsid.");
                        match Command::new(&command[0])
                            .args(&command[1..])
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                        {
                            Ok(child) => {
                                debug!("Process started: {:?}, pid {}", command, child.id());
                                exit(0);
                            }
                            Err(e) => {
                                error!("Error running command: {e:?}");
                                exit(1);
                            }
                        }
                    }
                    Ok(Fork::Parent(_)) => exit(0),
                    Err(e) => {
                        error!("Error spawning process: {e:?}");
                        exit(1);
                    }
                }
            }
            Ok(Fork::Parent(_)) => (),
            Err(e) => error!("Error spawning process: {e:?}"),
        }
    }
}

// use evdev::{uinput::VirtualDevice, EventType, InputEvent, KeyCode as Key};
// use fork::{fork, setsid, Fork};
// use log::debug;
// use log::error;
// use nix::sys::signal;
// use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet};
// use std::process::{exit, Command, Stdio};

// use crate::event::RelativeEvent;
// use crate::{action::Action, event::KeyEvent};
// use crate::event::{KeyEvent, KeyValue, Key};
// use crate::ahk::interpreter::AhkInterpreter;  
// use evdev::Key;  // This brings KeyCode into scope as Key

// pub struct ActionDispatcher {
//     // Device to emit events
//     device: VirtualDevice,
//     // Whether we've called a sigaction for spawning commands or not
//     sigaction_set: bool,
//     interpreter: AhkInterpreter<'a>,
// }

// impl ActionDispatcher {
//     pub fn new(device: VirtualDevice) -> ActionDispatcher {
//         ActionDispatcher {
//             device,
//             sigaction_set: false,
//         }
//     }




//     // Execute Actions created by EventHandler. This should be the only public method of ActionDispatcher.
//     pub fn on_action(&mut self, action: Action) -> anyhow::Result<()> {
//         match action {
//             Action::KeyEvent(key_event) => self.on_key_event(key_event)?,
//             Action::RelativeEvent(relative_event) => self.on_relative_event(relative_event)?,
//             Action::MouseMovementEventCollection(mouse_movement_events) => {
//                 self.send_mousemovement_event_batch(mouse_movement_events)?;
//             }
//             Action::InputEvent(event) => self.send_event(event)?,
//             Action::Command(command) => self.run_command(command),
//             // Action::TextExpansion { trigger_len, replacement, add_space } => {
//             //     use crate::event::KeyValue;
                
//             //     let final_text = if add_space {
//             //         format!("{}\u{00A0}", replacement)
//             //     } else {
//             //         replacement.clone()
//             //     };
                
//             //     // Delete trigger
//             //     for _ in 0..trigger_len {
//             //         self.on_key_event(KeyEvent::new(Key::KEY_BACKSPACE, KeyValue::Press))?;
//             //         self.on_key_event(KeyEvent::new(Key::KEY_BACKSPACE, KeyValue::Release))?;
//             //     }
                
//             //     // Copy to clipboard
//             //     crate::ahk::WaylandTextInjector::copy_to_clipboard(&final_text)?;
//             //     thread::sleep(Duration::from_millis(50));
                
//             //     // Type lctrl, then v
//             //     for key_str in &["lctrl", "v"] {
//             //         if let Some((key, needs_shift)) = self.char_to_key_with_modifiers(key_str) {
//             //             if needs_shift {
//             //                 self.on_key_event(KeyEvent::new(Key::KEY_LEFTSHIFT, KeyValue::Press))?;
//             //             }
//             //             self.on_key_event(KeyEvent::new(key, KeyValue::Press))?;
//             //             self.on_key_event(KeyEvent::new(key, KeyValue::Release))?;
//             //             if needs_shift {
//             //                 self.on_key_event(KeyEvent::new(Key::KEY_LEFTSHIFT, KeyValue::Release))?;
//             //             }
//             //         }
//             //     }
//             // }

//           Action::TextExpansion { trigger_len, replacement, add_space } => {
//     use crate::event::{KeyEvent, KeyValue};
//     use evdev::Key;  // Import Key directly (fixes the private import error)

//     let final_text = if add_space {
//         format!("{}\u{00A0}", replacement)
//     } else {
//         replacement.clone()
//     };

//     // Delete the trigger
//     for _ in 0..trigger_len {
//         self.on_key_event(KeyEvent::new(Key::KEY_BACKSPACE, KeyValue::Press))?;
//         self.on_key_event(KeyEvent::new(Key::KEY_BACKSPACE, KeyValue::Release))?;
//     }

//     // Copy to clipboard
//     crate::ahk::WaylandTextInjector::copy_to_clipboard(&final_text)?;
//     thread::sleep(Duration::from_millis(100));  // Increased to 100ms for reliability

//     // Send Ctrl+V — same as your working hotkeys
//     self.on_key_event(KeyEvent::new(Key::KEY_LEFTCTRL, KeyValue::Press))?;
//     self.on_key_event(KeyEvent::new(Key::KEY_V, KeyValue::Press))?;
//     self.on_key_event(KeyEvent::new(Key::KEY_V, KeyValue::Release))?;
//     self.on_key_event(KeyEvent::new(Key::KEY_LEFTCTRL, KeyValue::Release))?;
// }




//         }
//         Ok(())
//     }

//     fn char_to_key_with_modifiers(&self, s: &str) -> Option<(Key, bool)> {
//         match s {
//             // Modifier keys
//             "lctrl" => Some((Key::KEY_LEFTCTRL, false)),
//             "rctrl" => Some((Key::KEY_RIGHTCTRL, false)),
//             "lalt" => Some((Key::KEY_LEFTALT, false)),
//             "ralt" => Some((Key::KEY_RIGHTALT, false)),
//             "lshift" => Some((Key::KEY_LEFTSHIFT, false)),
//             "rshift" => Some((Key::KEY_RIGHTSHIFT, false)),
//             "lmeta" => Some((Key::KEY_LEFTMETA, false)),
//             "rmeta" => Some((Key::KEY_RIGHTMETA, false)),
            
//             // Regular single characters
//             _ if s.len() == 1 => {
//                 let ch = s.chars().next().unwrap();
//                 match ch {
//                     'a'..='z' => {
//                         let key = match ch {
//                             'a' => Key::KEY_A, 'b' => Key::KEY_B, 'c' => Key::KEY_C,
//                             'd' => Key::KEY_D, 'e' => Key::KEY_E, 'f' => Key::KEY_F,
//                             'g' => Key::KEY_G, 'h' => Key::KEY_H, 'i' => Key::KEY_I,
//                             'j' => Key::KEY_J, 'k' => Key::KEY_K, 'l' => Key::KEY_L,
//                             'm' => Key::KEY_M, 'n' => Key::KEY_N, 'o' => Key::KEY_O,
//                             'p' => Key::KEY_P, 'q' => Key::KEY_Q, 'r' => Key::KEY_R,
//                             's' => Key::KEY_S, 't' => Key::KEY_T, 'u' => Key::KEY_U,
//                             'v' => Key::KEY_V, 'w' => Key::KEY_W, 'x' => Key::KEY_X,
//                             'y' => Key::KEY_Y, 'z' => Key::KEY_Z,
//                             _ => return None,
//                         };
//                         Some((key, false))
//                     }
//                     'A'..='Z' => {
//                         let key = match ch {
//                             'A' => Key::KEY_A, 'B' => Key::KEY_B, 'C' => Key::KEY_C,
//                             'D' => Key::KEY_D, 'E' => Key::KEY_E, 'F' => Key::KEY_F,
//                             'G' => Key::KEY_G, 'H' => Key::KEY_H, 'I' => Key::KEY_I,
//                             'J' => Key::KEY_J, 'K' => Key::KEY_K, 'L' => Key::KEY_L,
//                             'M' => Key::KEY_M, 'N' => Key::KEY_N, 'O' => Key::KEY_O,
//                             'P' => Key::KEY_P, 'Q' => Key::KEY_Q, 'R' => Key::KEY_R,
//                             'S' => Key::KEY_S, 'T' => Key::KEY_T, 'U' => Key::KEY_U,
//                             'V' => Key::KEY_V, 'W' => Key::KEY_W, 'X' => Key::KEY_X,
//                             'Y' => Key::KEY_Y, 'Z' => Key::KEY_Z,
//                             _ => return None,
//                         };
//                         Some((key, true))
//                     }
//                     ' ' => Some((Key::KEY_SPACE, false)),
//                     _ => None,
//                 }
//             }
//             _ => None,
//         }
//     }

//     fn on_key_event(&mut self, event: KeyEvent) -> std::io::Result<()> {
//         let event = InputEvent::new_now(EventType::KEY.0, event.code(), event.value());
//         self.send_event(event)
//     }

//     fn on_relative_event(&mut self, event: RelativeEvent) -> std::io::Result<()> {
//         let event = InputEvent::new_now(EventType::RELATIVE.0, event.code, event.value);
//         self.send_event(event)
//     }

//     fn send_mousemovement_event_batch(&mut self, eventbatch: Vec<RelativeEvent>) -> std::io::Result<()> {
//         let mut mousemovementbatch: Vec<InputEvent> = Vec::new();
//         for mouse_movement in eventbatch {
//             mousemovementbatch.push(InputEvent::new_now(
//                 EventType::RELATIVE.0,
//                 mouse_movement.code,
//                 mouse_movement.value,
//             ));
//         }
//         self.device.emit(&mousemovementbatch)
//     }

//     fn send_event(&mut self, event: InputEvent) -> std::io::Result<()> {
//         if event.event_type() == EventType::KEY {
//             debug!("{}: {:?}", event.value(), Key::new(event.code()))
//         }
//         self.device.emit(&[event])
//     }

//     fn run_command(&mut self, command: Vec<String>) {
//         if !self.sigaction_set {
//             let sig_action = SigAction::new(SigHandler::SigDfl, SaFlags::SA_NOCLDWAIT, SigSet::empty());
//             unsafe {
//                 sigaction(signal::SIGCHLD, &sig_action).expect("Failed to register SIGCHLD handler");
//             }
//             self.sigaction_set = true;
//         }

//         debug!("Running command: {command:?}");
//         match fork() {
//             Ok(Fork::Child) => {
//                 match fork() {
//                     Ok(Fork::Child) => {
//                         setsid().expect("Failed to setsid.");
//                         match Command::new(&command[0])
//                             .args(&command[1..])
//                             .stdin(Stdio::null())
//                             .stdout(Stdio::null())
//                             .stderr(Stdio::null())
//                             .spawn()
//                         {
//                             Ok(child) => {
//                                 debug!("Process started: {:?}, pid {}", command, child.id());
//                                 exit(0);
//                             }
//                             Err(e) => {
//                                 error!("Error running command: {e:?}");
//                                 exit(1);
//                             }
//                         }
//                     }
//                     Ok(Fork::Parent(_)) => exit(0),
//                     Err(e) => {
//                         error!("Error spawning process: {e:?}");
//                         exit(1);
//                     }
//                 }
//             }
//             Ok(Fork::Parent(_)) => (),
//             Err(e) => error!("Error spawning process: {e:?}"),
//         }
//     }
// }