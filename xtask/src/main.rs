//! `cargo xtask <task>`: repository chores that are not part of the app.
//!
//! - `icons`: renders `assets/logo-light.svg` into the app's icons. Run it after changing the logo
//!   and commit the results; the app build only embeds them.

use std::path::{Path, PathBuf};

use resvg::{tiny_skia, usvg};

/// Every size Windows asks for, from the 100% tray icon to the 256 px Explorer tile.
const ICO_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

fn main() -> Result<(), String> {
    match std::env::args().nth(1).as_deref() {
        Some("icons") => icons(),
        _ => Err("usage: cargo xtask icons".into()),
    }
}

fn icons() -> Result<(), String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
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
