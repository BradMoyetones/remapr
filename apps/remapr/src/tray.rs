//! The notification-area icon: shows whether Remapr is active and keeps it reachable while the
//! window is hidden.

use std::sync::mpsc::Sender;

use eframe::egui;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::i18n::tr;

/// Icon resources embedded by build.rs.
pub const ICON_ACTIVE: u16 = 1;
const ICON_PAUSED: u16 = 2;

/// What the tray, or a second launch of Remapr, asks the app to do.
pub enum Command {
    Show,
    TogglePause,
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    pause: CheckMenuItem,
}

impl Tray {
    /// Must run on the UI thread: Windows delivers tray clicks through its message loop.
    pub fn new(enabled: bool, commands: Sender<Command>, ctx: egui::Context) -> Result<Self, String> {
        let open = MenuItem::new(tr!("Open Remapr"), true, None);
        let pause = CheckMenuItem::new(tr!("Pause remapping"), true, !enabled, None);
        let quit = MenuItem::new(tr!("Quit"), true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &pause, &PredefinedMenuItem::separator(), &quit]).map_err(|e| e.to_string())?;

        let (open_id, pause_id, quit_id) = (open.id().clone(), pause.id().clone(), quit.id().clone());
        let (menu_commands, menu_ctx) = (commands.clone(), ctx.clone());
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let command = match event.id {
                id if id == open_id => Command::Show,
                id if id == pause_id => Command::TogglePause,
                id if id == quit_id => Command::Quit,
                _ => return,
            };
            let _ = menu_commands.send(command);
            menu_ctx.request_repaint();
        }));
        TrayIconEvent::set_event_handler(Some(move |event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let _ = commands.send(Command::Show);
                ctx.request_repaint();
            }
        }));

        let (icon, tooltip) = look(enabled);
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_icon(icon)
            .with_tooltip(tooltip)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { icon, pause })
    }

    /// Shows the remapping state: icon, tooltip and the "Pausar" check mark.
    pub fn sync(&self, enabled: bool) {
        let (icon, tooltip) = look(enabled);
        let _ = self.icon.set_icon(Some(icon));
        let _ = self.icon.set_tooltip(Some(tooltip));
        self.pause.set_checked(!enabled);
    }
}

fn look(enabled: bool) -> (Icon, &'static str) {
    let (id, tooltip) = if enabled { (ICON_ACTIVE, tr!("Remapr: on")) } else { (ICON_PAUSED, tr!("Remapr: paused")) };
    // Load the exact small size so Windows doesn't shrink the 32 px one.
    let size = remapr_platform::small_icon_size();
    let icon = Icon::from_resource(id, Some((size, size))).expect("build.rs embeds both tray icons");
    (icon, tooltip)
}
