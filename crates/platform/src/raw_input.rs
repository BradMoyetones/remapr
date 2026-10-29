//! Raw keyboard input, which the keyboard hook cannot coexist with.

use std::ptr::null_mut;

use windows_sys::Win32::UI::Input::{RAWINPUTDEVICE, RIDEV_REMOVE, RegisterRawInputDevices};

const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
const HID_USAGE_GENERIC_KEYBOARD: u16 = 0x06;

/// Unregisters this process from raw keyboard input.
///
/// winit registers for it (for its `DeviceEvent`s, which egui doesn't use), and while a window of
/// a process with that registration is in front, Windows skips the process's own low-level keyboard
/// hook: Remapr couldn't capture or remap a single key while its window had the focus. Call once
/// the event loop exists.
pub fn stop_raw_keyboard_input() -> bool {
    let device = RAWINPUTDEVICE {
        usUsagePage: HID_USAGE_PAGE_GENERIC,
        usUsage: HID_USAGE_GENERIC_KEYBOARD,
        dwFlags: RIDEV_REMOVE,
        hwndTarget: null_mut(),
    };
    // SAFETY: one correctly sized RAWINPUTDEVICE; RIDEV_REMOVE requires a null target.
    unsafe { RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32) != 0 }
}
