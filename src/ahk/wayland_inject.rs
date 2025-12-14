use anyhow::Result;
use std::io::Write;
use std::process::{Command, Stdio};
use wait_timeout::ChildExt;

pub struct WaylandTextInjector;

impl WaylandTextInjector {
    pub fn copy_to_clipboard(text: &str) -> Result<()> {
        let timeout = std::time::Duration::from_millis(500);
        let mut child = Command::new("wl-copy")
            .arg("--type")
            .arg("text/plain")
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(text.as_bytes())?;
        }

        match child.wait_timeout(timeout)? {
            Some(status) if status.success() => Ok(()),
            Some(_) => Err(anyhow::anyhow!("wl-copy failed")),
            None => {
                child.kill()?;
                Err(anyhow::anyhow!("wl-copy timed out"))
            }
        }
    }
}
