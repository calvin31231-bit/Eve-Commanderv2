//! System tray and the single OS-notification dispatch point.
//!
//! The tray gives the app a persistent home in the OS tray with a Show/Quit
//! menu. [`dispatch`] is the one funnel every notification producer goes
//! through: it collects the event in the shared [`NotificationCenter`] (so it
//! shows up in the Alerts rail) and raises an OS toast only when the center says
//! the event is allowed to interrupt — the "collect, don't interrupt" policy.

use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use eve_core::notify::{Notification, NotificationCenter};

/// Shared, mutable notification center.
pub type SharedCenter = Arc<Mutex<NotificationCenter>>;

/// Build the system tray with a Show/Quit menu.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show EVE Commander", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(
            app.default_window_icon()
                .expect("bundled default window icon")
                .clone(),
        )
        .tooltip("EVE Commander")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "show" => show_main_window(app),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Bring the main window to the foreground.
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Collect a notification and, if it asks to interrupt, raise an OS toast. This
/// is the integration point all producers (poller, intel, fuel timers, …) call.
pub fn dispatch(app: &AppHandle, center: &SharedCenter, notification: Notification) {
    let result = match center.lock() {
        Ok(mut c) => c.push(notification.clone()),
        Err(_) => return,
    };

    if result.interrupt {
        let _ = app
            .notification()
            .builder()
            .title(&notification.title)
            .body(&notification.body)
            .show();
    }
}
