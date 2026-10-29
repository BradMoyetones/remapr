//! The scheduled task that starts Remapr at sign-in with administrator rights.
//!
//! The installer registers it once, elevated. It runs for every member of Users with the highest
//! rights each one has, so an administrator gets an elevated Remapr with no UAC prompt. It also
//! runs on demand, which is how a plain launch hands over to an elevated copy. Turning "start at
//! sign-in" off keeps the task and drops its trigger, so that hand-over keeps working.
//!
//! Driven through schtasks.exe, which ships with Windows.

use std::io;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const TASK: &str = "Remapr";
/// The task passes this to Remapr, which then starts hidden in the tray.
pub const BACKGROUND_ARG: &str = "--background";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Creates or replaces the task. Needs administrator rights.
pub fn register(exe: &Path, at_sign_in: bool) -> io::Result<()> {
    let path = std::env::temp_dir().join(format!("remapr-task-{}.xml", std::process::id()));
    // UTF-16 with a byte order mark, as the XML declaration says.
    let bytes: Vec<u8> =
        std::iter::once(0xFEFF).chain(task_xml(exe, at_sign_in).encode_utf16()).flat_map(u16::to_le_bytes).collect();
    std::fs::write(&path, bytes)?;
    let result = schtasks(&["/Create", "/TN", TASK, "/F", "/XML"], Some(&path));
    let _ = std::fs::remove_file(&path);
    result.map(drop)
}

/// Removes the task. Needs administrator rights.
pub fn unregister() -> io::Result<()> {
    schtasks(&["/Delete", "/TN", TASK, "/F"], None).map(drop)
}

/// Starts the task now, in the signed-in user's session.
pub fn run() -> io::Result<()> {
    schtasks(&["/Run", "/TN", TASK], None).map(drop)
}

/// Whether Remapr starts at sign-in, or `None` when there is no task (Remapr isn't installed).
pub fn starts_at_sign_in() -> Option<bool> {
    let xml = schtasks(&["/Query", "/TN", TASK, "/XML", "ONE"], None).ok()?;
    // schtasks prints in the console code page whatever the declaration says; tag names are ASCII.
    Some(String::from_utf8_lossy(&xml).contains("<LogonTrigger"))
}

fn schtasks(args: &[&str], file: Option<&Path>) -> io::Result<Vec<u8>> {
    // The full path: an elevated process must not pick up a schtasks.exe from its working folder.
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let out = Command::new(PathBuf::from(system_root).join("System32").join("schtasks.exe"))
        .args(args)
        .args(file)
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(io::Error::other(String::from_utf8_lossy(&out.stderr).trim().to_owned()))
    }
}

fn task_xml(exe: &Path, at_sign_in: bool) -> String {
    let exe = xml_escape(&exe.display().to_string());
    let triggers =
        if at_sign_in { "<Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>" } else { "" };
    // Users (S-1-5-32-545) at their highest rights. Authenticated users may read and run it.
    // No time limit (the default stops a task after 3 days), runs on battery, normal priority
    // (the default, 7, is below normal), and one copy per signed-in user.
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts Remapr at sign-in with the highest rights you have.</Description>
    <SecurityDescriptor>D:(A;;FA;;;BA)(A;;FA;;;SY)(A;;FRFX;;;AU)</SecurityDescriptor>
  </RegistrationInfo>
  {triggers}
  <Principals>
    <Principal id="Users">
      <GroupId>S-1-5-32-545</GroupId>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>Parallel</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Users">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{BACKGROUND_ARG}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_starts_the_exe_hidden_and_never_times_out() {
        let xml = task_xml(Path::new(r"C:\Program Files\R&D\remapr.exe"), true);
        assert!(xml.contains(r"<Command>C:\Program Files\R&amp;D\remapr.exe</Command>"));
        assert!(xml.contains("<Arguments>--background</Arguments>"));
        assert!(xml.contains("<LogonTrigger>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
    }

    #[test]
    fn turning_sign_in_off_keeps_the_task_without_its_trigger() {
        let xml = task_xml(Path::new(r"C:\remapr.exe"), false);
        assert!(!xml.contains("Trigger"));
        assert!(xml.contains("<AllowStartOnDemand>true</AllowStartOnDemand>"));
    }
}
