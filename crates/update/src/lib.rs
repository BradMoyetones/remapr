//! Remapr's updater: finds a newer release on GitHub, downloads its installer, checks it against the
//! published SHA-256 and runs it. The installer itself ends the running Remapr, installs over it
//! (same folder, same settings) and starts it again.
//!
//! Only the version is read from GitHub's answer; download addresses are built from it, so an
//! unexpected response can't point the updater anywhere else.
//!
//! The network goes through curl.exe, which ships with Windows 10 and later (as schtasks.exe and
//! msiexec.exe do), so Remapr carries no TLS stack of its own.
//! shortcut: curl.exe ignores the system proxy settings (it honours `HTTPS_PROXY`); move to WinHTTP if
//! users behind a proxy report failed checks.
//!
//! Errors are an [`Error`]; the app puts them into words, in the user's language.

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const REPO: &str = "BradMoyetones/remapr";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// What went wrong, for the app to put into words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No connection, or curl.exe couldn't run; curl's own words, possibly empty.
    Network(String),
    /// GitHub answered something unexpected, or refused (its message, like a rate limit).
    GitHub(String),
    /// The published checksum file isn't one.
    UnreadableChecksum,
    /// The download doesn't match the published checksum; it was deleted.
    ChecksumMismatch,
    /// A file couldn't be written or read; the system's words.
    File(String),
    /// Windows Installer's exit code, or `None` when it couldn't start or was interrupted.
    Installer(Option<i32>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// The release page on GitHub, with its notes.
    pub notes_url: String,
    msi_url: String,
    sha256_url: String,
    /// Bytes, for the download's progress.
    msi_size: u64,
}

/// The page of `version`'s release, with its notes ("Novedades").
pub fn release_page(version: &str) -> String {
    format!("https://github.com/{REPO}/releases/tag/v{version}")
}

/// Where releases come from: GitHub in the app, local files in tests.
pub struct Source {
    latest: String,
    downloads: String,
}

impl Source {
    pub fn github() -> Self {
        Self {
            latest: format!("https://api.github.com/repos/{REPO}/releases/latest"),
            downloads: format!("https://github.com/{REPO}/releases/download"),
        }
    }

    /// A folder laid out like GitHub's: `latest.json` (the API's answer) and `v<version>/` with the
    /// files. For tests, and for trying an update in a debug build before publishing anything.
    pub fn local(dir: &Path) -> Self {
        let url = format!("file:///{}", dir.display().to_string().replace('\\', "/").replace(' ', "%20"));
        Self { latest: format!("{url}/latest.json"), downloads: url }
    }

    /// The latest release, if it is newer than `current` and its installer is already uploaded.
    pub fn check(&self, current: &str) -> Result<Option<Release>, Error> {
        // No `-f`: GitHub answers "no release yet" with a 404 whose JSON says so.
        let body = curl(&["-sSL", "--max-time", "20", "-H", "Accept: application/vnd.github+json", &self.latest])?;
        self.parse(&body, current)
    }

    fn parse(&self, body: &[u8], current: &str) -> Result<Option<Release>, Error> {
        #[derive(Deserialize)]
        struct Latest {
            tag_name: Option<String>,
            message: Option<String>,
            #[serde(default)]
            assets: Vec<Asset>,
        }
        #[derive(Deserialize)]
        struct Asset {
            name: String,
            size: u64,
        }

        let latest: Latest = serde_json::from_slice(body).map_err(|e| Error::GitHub(e.to_string()))?;
        let Some(tag) = latest.tag_name else {
            return match latest.message.as_deref() {
                None | Some("Not Found") => Ok(None),
                Some(message) => Err(Error::GitHub(message.to_owned())),
            };
        };
        let version = tag.trim_start_matches(['v', 'V']).to_owned();
        let (Some(new), Some(old)) = (parse_version(&version), parse_version(current)) else {
            return Err(Error::GitHub(format!("unexpected version {tag}")));
        };
        let msi = format!("Remapr-{version}-x64.msi");
        let sha256 = format!("{msi}.sha256");
        let msi_size = latest.assets.iter().find(|a| a.name == msi).map(|a| a.size);
        let has_sha256 = latest.assets.iter().any(|a| a.name == sha256);
        // A release whose files are still uploading is picked up on a later check.
        let (true, Some(msi_size), true) = (new > old, msi_size, has_sha256) else { return Ok(None) };
        Ok(Some(Release {
            notes_url: release_page(&version),
            msi_url: format!("{}/v{version}/{msi}", self.downloads),
            sha256_url: format!("{}/v{version}/{sha256}", self.downloads),
            msi_size,
            version,
        }))
    }
}

/// `major.minor.patch`, ignoring a pre-release or build suffix.
fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// Downloads `release`'s installer into `dir`, calling `progress` with 0..=1, and checks it against
/// the published SHA-256. A file that doesn't match is deleted, never run.
pub fn download(release: &Release, dir: &Path, progress: impl Fn(f32)) -> Result<PathBuf, Error> {
    std::fs::create_dir_all(dir).map_err(|e| Error::File(e.to_string()))?;
    let sums = curl(&["-sSfL", "--max-time", "30", &release.sha256_url])?;
    let expected = String::from_utf8_lossy(&sums)
        .split_whitespace()
        .next()
        .map(str::to_ascii_lowercase)
        .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(Error::UnreadableChecksum)?;

    let msi = dir.join(format!("Remapr-{}-x64.msi", release.version));
    let _ = std::fs::remove_file(&msi);
    let mut child = command(&["-sSfL", "--max-time", "900", "-o"])
        .arg(&msi)
        .arg(&release.msi_url)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Network(e.to_string()))?;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| Error::Network(e.to_string()))? {
            break status;
        }
        let got = std::fs::metadata(&msi).map_or(0, |m| m.len());
        progress((got as f32 / release.msi_size.max(1) as f32).min(1.0));
        std::thread::sleep(Duration::from_millis(100));
    };
    if !status.success() {
        let mut stderr = String::new();
        let _ = child.stderr.take().map(|mut e| e.read_to_string(&mut stderr));
        let _ = std::fs::remove_file(&msi);
        return Err(Error::Network(stderr.trim().to_owned()));
    }
    progress(1.0);

    let bytes = std::fs::read(&msi).map_err(|e| Error::File(e.to_string()))?;
    if sha256(&bytes) != expected {
        let _ = std::fs::remove_file(&msi);
        return Err(Error::ChecksumMismatch);
    }
    Ok(msi)
}

/// Starts Windows Installer on `msi` with a progress bar and no questions, logging beside the file.
pub fn install(msi: &Path) -> Result<Child, Error> {
    Command::new(system32("msiexec.exe"))
        .arg("/i")
        .arg(msi)
        .args(["/passive", "/norestart", "/l*v"])
        .arg(msi.with_extension("log"))
        .spawn()
        .map_err(|_| Error::Installer(None))
}

/// Windows Installer's exit code as a result: success (also "restart needed") or [`Error::Installer`].
pub fn install_result(code: Option<i32>) -> Result<(), Error> {
    match code {
        Some(0 | 3010 | 1641) => Ok(()),
        code => Err(Error::Installer(code)),
    }
}

fn curl(args: &[&str]) -> Result<Vec<u8>, Error> {
    let out = command(args).output().map_err(|e| Error::Network(e.to_string()))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(Error::Network(String::from_utf8_lossy(&out.stderr).trim().to_owned()))
    }
}

fn command(args: &[&str]) -> Command {
    let mut command = Command::new(system32("curl.exe"));
    // The header goes first: callers append options whose values follow them (`-o <file>`).
    command.arg("-H").arg(concat!("User-Agent: Remapr/", env!("CARGO_PKG_VERSION"))).args(args);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// The full path: an elevated process must not pick up a same-named program from its working folder.
fn system32(program: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into())).join("System32").join(program)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn latest(tag: &str, assets: &[(&str, u64)]) -> String {
        let assets: Vec<String> = assets.iter().map(|(n, s)| format!(r#"{{"name":"{n}","size":{s}}}"#)).collect();
        format!(r#"{{"tag_name":"{tag}","assets":[{}]}}"#, assets.join(","))
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(parse_version("0.10.0") > parse_version("0.9.9"));
        assert_eq!(parse_version("1.2.3-rc.1"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), None);
    }

    #[test]
    fn only_a_newer_complete_release_is_offered() {
        let source = Source::github();
        let full = [("Remapr-0.3.0-x64.msi", 10), ("Remapr-0.3.0-x64.msi.sha256", 90)];
        let found = source.parse(latest("v0.3.0", &full).as_bytes(), "0.2.9").unwrap().unwrap();
        assert_eq!(found.version, "0.3.0");
        assert_eq!(
            found.msi_url,
            "https://github.com/BradMoyetones/remapr/releases/download/v0.3.0/Remapr-0.3.0-x64.msi"
        );
        assert_eq!(found.notes_url, "https://github.com/BradMoyetones/remapr/releases/tag/v0.3.0");
        assert_eq!(source.parse(latest("v0.3.0", &full).as_bytes(), "0.3.0").unwrap(), None);
        // Still uploading: no checksum yet.
        assert_eq!(source.parse(latest("v0.3.0", &full[..1]).as_bytes(), "0.2.0").unwrap(), None);
        // No release at all, or GitHub refusing.
        assert_eq!(source.parse(br#"{"message":"Not Found"}"#, "0.2.0").unwrap(), None);
        assert!(source.parse(br#"{"message":"API rate limit exceeded"}"#, "0.2.0").is_err());
    }

    #[test]
    fn installer_codes_read_as_messages() {
        assert!(install_result(Some(0)).is_ok());
        assert!(install_result(Some(3010)).is_ok());
        assert_eq!(install_result(Some(1602)), Err(Error::Installer(Some(1602))));
    }

    /// The whole check and download through curl.exe, against files standing in for GitHub.
    #[test]
    fn downloads_and_verifies_the_installer() {
        let dir = std::env::temp_dir().join(format!("remapr-update-test-{}", std::process::id()));
        let release_dir = dir.join("v0.9.0");
        std::fs::create_dir_all(&release_dir).unwrap();
        let msi = b"not really an msi".repeat(1000);
        std::fs::write(release_dir.join("Remapr-0.9.0-x64.msi"), &msi).unwrap();
        let sums = format!("{}  Remapr-0.9.0-x64.msi\n", sha256(&msi));
        std::fs::write(release_dir.join("Remapr-0.9.0-x64.msi.sha256"), sums).unwrap();
        let assets = [("Remapr-0.9.0-x64.msi", msi.len() as u64), ("Remapr-0.9.0-x64.msi.sha256", 90)];
        std::fs::write(dir.join("latest.json"), latest("v0.9.0", &assets)).unwrap();

        let release = Source::local(&dir).check("0.1.0").unwrap().expect("a newer release");
        let last = std::cell::Cell::new(0.0);
        let path = download(&release, &dir.join("out"), |p| last.set(p)).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), msi);
        assert_eq!(last.get(), 1.0);

        // A file that doesn't match its checksum is refused and removed.
        std::fs::write(release_dir.join("Remapr-0.9.0-x64.msi"), b"tampered").unwrap();
        assert!(download(&release, &dir.join("out"), |_| {}).is_err());
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
