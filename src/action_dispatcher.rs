use evdev::{uinput::VirtualDevice, EventType, InputEvent, KeyCode as Key};
use fork::{fork, setsid, Fork};
use log::debug;
use log::error;
use nix::sys::signal;
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet};
use std::process::{exit, Command, Stdio};
use crate::action::Action;
use crate::event::{KeyEvent, KeyValue, RelativeEvent};
use crate::ahk::interpreter::AhkInterpreter;  

pub struct ActionDispatcher<'a> {
    device: VirtualDevice,
    sigaction_set: bool,
    _interpreter: &'a mut AhkInterpreter<'a>,
}

impl<'a> ActionDispatcher<'a> {
    pub fn new(device: VirtualDevice, interpreter: &'a mut AhkInterpreter<'a>) -> Self {
        ActionDispatcher {
            device,
            sigaction_set: false,
            _interpreter: interpreter,
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

                // Copy replacement to clipboard
                crate::ahk::WaylandTextInjector::copy_to_clipboard(&final_text)?;

                // Paste using Shift+Insert instead of Ctrl+V
                self.on_key_event(KeyEvent::new(Key::KEY_LEFTSHIFT, KeyValue::Press))?;
                self.on_key_event(KeyEvent::new(Key::KEY_INSERT, KeyValue::Press))?;
                self.on_key_event(KeyEvent::new(Key::KEY_INSERT, KeyValue::Release))?;
                self.on_key_event(KeyEvent::new(Key::KEY_LEFTSHIFT, KeyValue::Release))?;
            }
        }
        Ok(())
    }

    fn on_key_event(&mut self, event: KeyEvent) -> std::io::Result<()> {
        let value = event.value();
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