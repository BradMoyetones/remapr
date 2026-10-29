//! `cargo xtask <task>`: repository chores that are not part of the app.
//!
//! - `icons`: renders `assets/logo-light.svg` into the app's icons. Run it after changing the logo
//!   and commit the results; the app build only embeds them.
//! - `dist`: builds the release executable and the installer, `target/dist/Remapr-<version>-x64.msi`,
//!   with its SHA-256 beside it. Fetches WiX on first use; needs the .NET runtime (6 or newer).

use std::path::{Path, PathBuf};
use std::process::Command;

use resvg::{tiny_skia, usvg};
use sha2::{Digest, Sha256};

/// Every size Windows asks for, from the 100% tray icon to the 256 px Explorer tile.
const ICO_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

/// WiX 5 (as photocraft uses), pinned: NuGet id, version and the package's SHA-256.
const WIX_PACKAGES: [(&str, &str, &str); 1] =
    [("wix", "5.0.2", "f30ef0c74e2a986126539c5780be93ac24e8136eaf723b1937b26272703ae173")];

fn main() -> Result<(), String> {
    match std::env::args().nth(1).as_deref() {
        Some("icons") => icons(),
        Some("dist") => dist(),
        _ => Err("usage: cargo xtask <icons|dist>".into()),
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the workspace").to_path_buf()
}

fn dist() -> Result<(), String> {
    let root = root();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    run(Command::new(cargo).args(["build", "--release", "--locked", "-p", "remapr"]).current_dir(&root))?;

    let wix = root.join("target").join("wix");
    fetch_wix(&wix)?;
    let version = env!("CARGO_PKG_VERSION");
    // MSI versions are numeric: a pre-release tag is dropped there.
    let msi_version = version.split('-').next().unwrap_or(version);
    let dist = root.join("target").join("dist");
    std::fs::create_dir_all(&dist).map_err(|e| e.to_string())?;
    let msi = dist.join(format!("Remapr-{version}-x64.msi"));
    run(Command::new("dotnet")
        .arg(wix.join("wix.5.0.2/tools/net6.0/any/wix.dll"))
        .arg("build")
        .arg(root.join("packaging/windows/remapr.wxs"))
        .args(["-arch", "x64", "-pdbtype", "none"])
        .args(["-d", &format!("Version={msi_version}")])
        .arg("-d")
        .arg(format!("ExePath={}", root.join("target/release/remapr.exe").display()))
        .arg("-d")
        .arg(format!("IconPath={}", root.join("assets/icon.ico").display()))
        .arg("-o")
        .arg(&msi)
        // WiX targets .NET 6; any newer runtime will do.
        .env("DOTNET_ROLL_FORWARD", "Major"))?;

    let bytes = std::fs::read(&msi).map_err(|e| e.to_string())?;
    let name = msi.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    // The format `sha256sum -c` reads: what the updater checks a download against.
    std::fs::write(dist.join(format!("{name}.sha256")), format!("{}  {name}\n", sha256(&bytes)))
        .map_err(|e| e.to_string())?;
    println!("wrote {}", msi.display());
    Ok(())
}

/// Downloads and unpacks the pinned WiX packages into `dir`, once.
fn fetch_wix(dir: &Path) -> Result<(), String> {
    let system32 =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into())).join("System32");
    for (id, version, expected) in WIX_PACKAGES {
        let unpacked = dir.join(format!("{id}.{version}"));
        if unpacked.exists() {
            continue;
        }
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let nupkg = dir.join(format!("{id}.{version}.nupkg"));
        let url = format!("https://api.nuget.org/v3-flatcontainer/{id}/{version}/{id}.{version}.nupkg");
        println!("fetching {url}");
        // curl.exe and tar.exe (which reads zips) ship with Windows 10 and later.
        run(Command::new(system32.join("curl.exe")).args(["-sSfL", "-o"]).arg(&nupkg).arg(&url))?;
        let actual = sha256(&std::fs::read(&nupkg).map_err(|e| e.to_string())?);
        if actual != expected {
            let _ = std::fs::remove_file(&nupkg);
            return Err(format!("{id} {version}: SHA-256 is {actual}, expected {expected}"));
        }
        // Unpack beside, then rename: an interrupted run never leaves a half-unpacked folder behind.
        let partial = dir.join(format!("{id}.{version}.partial"));
        let _ = std::fs::remove_dir_all(&partial);
        std::fs::create_dir_all(&partial).map_err(|e| e.to_string())?;
        run(Command::new(system32.join("tar.exe")).arg("-xf").arg(&nupkg).arg("-C").arg(&partial))?;
        std::fs::rename(&partial, &unpacked).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn run(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|e| format!("{:?}: {e}", command.get_program()))?;
    if status.success() { Ok(()) } else { Err(format!("{:?} failed: {status}", command.get_program())) }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn icons() -> Result<(), String> {
    let dir = root().join("assets");
    let svg = std::fs::read(dir.join("logo-light.svg")).map_err(|e| e.to_string())?;
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).map_err(|e| e.to_string())?;

    write_ico(&tree, dir.join("icon.ico"), false)?;
    write_ico(&tree, dir.join("icon-paused.ico"), true)?;
    let png = render(&tree, 256).encode_png().map_err(|e| e.to_string())?;
    std::fs::write(dir.join("icon-256.png"), png).map_err(|e| e.to_string())?;
    println!("wrote icon.ico, icon-paused.ico and icon-256.png to {}", dir.display());
    Ok(())
}

/// Each size is rendered from the vector, not scaled down from a bitmap, so 16 px stays sharp.
fn write_ico(tree: &usvg::Tree, path: PathBuf, paused: bool) -> Result<(), String> {
    let mut icon = ico::IconDir::new(ico::ResourceType::Icon);
    for size in ICO_SIZES {
        let image = ico::IconImage::from_rgba_data(size, size, rgba(&render(tree, size), paused));
        // Windows' convention: 32-bit BMP below 256 px (every reader understands it), PNG at 256.
        let entry =
            if size < 256 { ico::IconDirEntry::encode_as_bmp(&image) } else { ico::IconDirEntry::encode(&image) };
        icon.add_entry(entry.map_err(|e| e.to_string())?);
    }
    let file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    icon.write(file).map_err(|e| e.to_string())
}

fn render(tree: &usvg::Tree, size: u32) -> tiny_skia::Pixmap {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("icon sizes are non-zero");
    let scale = size as f32 / tree.size().width();
    resvg::render(tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap
}

/// Straight (not premultiplied) RGBA, as ICO wants it. `paused` greys the icon out and fades it,
/// the way Windows draws a disabled item.
fn rgba(pixmap: &tiny_skia::Pixmap, paused: bool) -> Vec<u8> {
    pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            let (r, g, b, a) = (c.red(), c.green(), c.blue(), c.alpha());
            if paused {
                let grey = (0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)) as u8;
                [grey, grey, grey, a / 2]
            } else {
                [r, g, b, a]
            }
        })
        .collect()
}
