//! Tray-click popup behaviour of the main window (Hyprland placement); see
//! `crate::hyprland` for the compositor side.

use std::rc::Rc;

use adw::prelude::*;

use super::{MainWindow, TITLE};

impl MainWindow {
    /// Tray click: hide the window if it is showing, otherwise open it under the
    /// tray icon (Hyprland, when the preference is on) or just present it. It
    /// stays up until closed or the tray icon is clicked again.
    pub fn toggle_from_tray(self: &Rc<Self>, runtime: &tokio::runtime::Handle) {
        if self.window.is_visible() {
            self.window.set_visible(false);
            return;
        }
        let as_popup = self.prefs.borrow().popup_from_tray && crate::hyprland::available();
        if !as_popup {
            self.present();
            return;
        }
        let size = {
            let p = self.prefs.borrow();
            (p.window_width as i32, p.window_height as i32)
        };
        let main = self.clone();
        let runtime = runtime.clone();
        glib::spawn_future_local(async move {
            // Pointer position (and the window rule) first, while it is still on the tray icon.
            let prepared = runtime.spawn(crate::hyprland::prepare_popup(crate::APP_ID, TITLE, size)).await.unwrap_or_else(|e| Err(e.to_string()));
            main.window.present();
            match prepared {
                Ok(placement) => {
                    runtime.spawn(async move {
                        if let Err(err) = crate::hyprland::finish_popup(crate::APP_ID, TITLE, size, &placement).await {
                            eprintln!("tuple-ui: could not place the window under the tray icon: {err}");
                        }
                    });
                }
                Err(err) => eprintln!("tuple-ui: could not place the window under the tray icon: {err}"),
            }
        });
    }
}
