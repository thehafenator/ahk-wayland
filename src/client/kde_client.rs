use log::{warn, info};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use dbus::blocking::Connection;
use dbus::message::MatchRule;

use crate::client::Client;

pub struct KdeClient {
    active_window: Arc<Mutex<ActiveWindow>>,
}

#[derive(Clone, Debug)]
struct ActiveWindow {
    res_class: String,
    title: String,
}

impl KdeClient {
    pub fn new() -> KdeClient {
        let active_window = Arc::new(Mutex::new(ActiveWindow {
            title: String::new(),
            res_class: String::new(),
        }));

        let window_clone = Arc::clone(&active_window);
        
        thread::spawn(move || {
            listen_for_window_changes(window_clone);
        });

        KdeClient {
            active_window,
        }
    }
}

fn listen_for_window_changes(window_state: Arc<Mutex<ActiveWindow>>) {
    info!("KDE Client: Starting D-Bus listener");
    
    loop {
        match Connection::new_session() {
            Ok(conn) => {
                info!("KDE Client: Connected to D-Bus session");
                
                // Clone Arc for the closure
                let ws = Arc::clone(&window_state);
                
                // Create match rule for our signals
                // Match all signals from the org.ahkwayland.ActiveWindow interface
                let rule = MatchRule::new()
                    .with_type(dbus::message::MessageType::Signal)
                    .with_interface("org.ahkwayland.ActiveWindow")
                    .static_clone();
                
                let match_token = match conn.add_match(rule, move |_: (), _conn, msg| {
                    // Check if this is our signal
                    if let (Some(interface), Some(member)) = (msg.interface(), msg.member()) {
                        let interface_str = interface.to_string();
                        let member_str = member.to_string();
                        
                        if interface_str == "org.ahkwayland.ActiveWindow" 
                            && (member_str == "Changed" || member_str == "Initial") {
                            // Try to read the two string arguments
                            if let Ok((class, title)) = msg.read2::<String, String>() {
                                info!("Window: class='{}', title='{}'", class, title);
                                
                                if let Ok(mut window) = ws.lock() {
                                    window.res_class = class;
                                    window.title = title.clone();
                                    info!("Updated active window: caption: '{}', class: '{}'", 
                                          window.title, window.res_class);
                                }
                            } else {
                                warn!("Failed to parse D-Bus message arguments");
                            }
                        }
                    }
                    true
                }) {
                    Ok(token) => token,
                    Err(e) => {
                        warn!("Failed to add D-Bus match rule: {:?}", e);
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                };
                
                info!("KDE Client: Listening for window signals");
                
                // Process incoming messages
                loop {
                    if let Err(e) = conn.process(Duration::from_millis(1000)) {
                        warn!("D-Bus connection error: {:?}", e);
                        break;
                    }
                }
                
                // Clean up match on disconnect
                let _ = conn.remove_match(match_token);
            }
            Err(e) => {
                warn!("Failed to connect to D-Bus: {:?}", e);
                thread::sleep(Duration::from_secs(5));
            }
        }
    }
}

impl Client for KdeClient {
    fn supported(&mut self) -> bool {
        true
    }

    fn current_window(&mut self) -> Option<String> {
        let aw = self.active_window.lock().ok()?;
        let title = aw.title.clone();
        if !title.is_empty() {
            Some(title)
        } else {
            None
        }
    }

    fn current_application(&mut self) -> Option<String> {
        let aw = self.active_window.lock().ok()?;
        let class = aw.res_class.clone();
        if !class.is_empty() {
            Some(class)
        } else {
            None
        }
    }
}