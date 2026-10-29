//! Opening web pages.

use std::path::PathBuf;
use std::process::Command;

/// Opens `url` in the default browser, as the signed-in user even when Remapr runs elevated:
/// explorer.exe hands the request to the user's own shell, so the browser never gets admin rights.
pub fn open_in_browser(url: &str) {
    let explorer =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into())).join("explorer.exe");
    let _ = Command::new(explorer).arg(url).spawn();
}
