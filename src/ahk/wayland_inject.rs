// use anyhow::Result;
// use std::io::Write;
// use std::process::{Command, Stdio};
// use wait_timeout::ChildExt;

// pub struct WaylandTextInjector;

//  original pre 1.8.2026 no edits
// impl WaylandTextInjector {
//     pub fn copy_to_clipboard(text: &str) -> Result<()> {
//         let timeout = std::time::Duration::from_millis(500);
//         let mut child = Command::new("wl-copy")
//             .arg("--type")
//             .arg("text/plain")
//             .stdin(Stdio::piped())
//             .spawn()?;

//         if let Some(stdin) = child.stdin.as_mut() {
//             stdin.write_all(text.as_bytes())?;
//         }

//         match child.wait_timeout(timeout)? {
//             Some(status) if status.success() => Ok(()),
//             Some(_) => Err(anyhow::anyhow!("wl-copy failed")),
//             None => {
//                 child.kill()?;
//                 Err(anyhow::anyhow!("wl-copy timed out"))
//             }
//         }
//     }
// }

use anyhow::Result;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::io::Write;
use wait_timeout::ChildExt;

pub struct WaylandTextInjector;

impl WaylandTextInjector {
    /// Copy to the primary selection (highlight/middle-click buffer)
    /// Uses --paste-once so it clears automatically after first paste
    pub fn copy_to_primary(text: &str) -> Result<()> {
        Self::wl_copy(text, true)
    }

    /// Optional: copy to regular clipboard (not recommended for expansions)
    pub fn copy_to_clipboard(text: &str) -> Result<()> {
        Self::wl_copy(text, false)
    }

    fn wl_copy(text: &str, primary: bool) -> Result<()> {
        let timeout = Duration::from_millis(500);
        let mut cmd = Command::new("wl-copy");
        cmd.arg("--type").arg("text/plain");

        if primary {
            cmd.arg("--primary");
            cmd.arg("--paste-once");  // Auto-clear after first paste
        }

        let mut child = cmd.stdin(Stdio::piped()).spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
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

    /// Optional: read current primary (only needed if you want to restore manually)
    pub fn get_primary() -> Result<String> {
        let output = Command::new("wl-paste")
            .arg("--primary")
            .arg("--no-newline")
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Ok(String::new())
        }
    }
}