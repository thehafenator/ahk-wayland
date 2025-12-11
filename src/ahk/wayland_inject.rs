use std::process::{Command, Stdio};
use std::io::Write;
use std::thread;
use std::time::Duration;
use anyhow::Result;

pub struct WaylandTextInjector;

impl WaylandTextInjector {
    pub fn inject_text(text: &str) -> Result<()> {
        Self::copy_to_clipboard(text)?;
        thread::sleep(Duration::from_millis(10));
        Self::simulate_paste()?;
        Ok(())
    }

    fn copy_to_clipboard(text: &str) -> Result<()> {
        let mut child = Command::new("wl-copy")
            .arg("--type")
            .arg("text/plain")
            .stdin(Stdio::piped())
            .spawn()?;
        
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(text.as_bytes())?;
        }
        
        child.wait()?;
        Ok(())
    }

    fn simulate_paste() -> Result<()> {
        Command::new("wtype")
            .arg("-M")
            .arg("ctrl")
            .arg("-P")
            .arg("v")
            .arg("-m")
            .arg("ctrl")
            .arg("-p")
            .arg("v")
            .spawn()?
            .wait()?;
        
        Ok(())
    }
}
