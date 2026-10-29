//! Embeds the icons and version information into remapr.exe.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    println!("cargo:rerun-if-changed=../../assets/icon-paused.ico");
    let mut res = winresource::WindowsResource::new();
    // Ids 1 and 2 are read back as `tray::ICON_ACTIVE` and `tray::ICON_PAUSED`. Explorer shows id 1.
    res.set_icon_with_id("../../assets/icon.ico", "1")
        .set_icon_with_id("../../assets/icon-paused.ico", "2")
        // Task Manager shows FileDescription as the process name.
        .set("FileDescription", "Remapr")
        .set("ProductName", "Remapr")
        .set("LegalCopyright", "Copyright (c) 2026 Brad Moyetones");
    res.compile().expect("embedding Windows resources (needs rc.exe from the Windows SDK)");
}
