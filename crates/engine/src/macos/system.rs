//! What the tap asks macOS: the app in front, whether Secure Input is on, and the permission to
//! change keys.

use objc2_app_kit::NSWorkspace;
use objc2_core_graphics::{CGPreflightPostEventAccess, CGRequestListenEventAccess, CGRequestPostEventAccess};
use remapr_core::Permission;

/// The bundle identifier of the app in front (`com.apple.Safari`), if it has one.
pub fn frontmost_app() -> Option<String> {
    objc2::rc::autoreleasepool(|_| {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        Some(app.bundleIdentifier()?.to_string())
    })
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    /// HIToolbox, `CarbonEvents.h`.
    fn IsSecureEventInputEnabled() -> u8;
}

/// Whether an app has Secure Input on (a password field has the focus): then no tap sees keys.
pub fn secure_input() -> bool {
    // SAFETY: a query with no arguments.
    unsafe { IsSecureEventInputEnabled() != 0 }
}

/// Whether Remapr may change keys: Privacy & Security › Accessibility.
pub fn may_change_keys() -> bool {
    CGPreflightPostEventAccess()
}

/// Shows macOS's own prompt for `permission`, once; after that, only System Settings can grant it.
pub fn ask(permission: Permission) {
    match permission {
        Permission::Accessibility => {
            CGRequestPostEventAccess();
        }
        Permission::InputMonitoring => {
            CGRequestListenEventAccess();
        }
        Permission::InputDevices => {}
    }
}
