/// Cross-platform native menu bar.
///
/// - macOS: NSApplication system menu bar (via muda)
/// - Windows: Win32 in-window menu bar (via muda)
/// - Linux: no-op — egui in-window menu bar in toolbar.rs is used instead,
///   so muda (which requires GTK dev headers) is not compiled on Linux.

/// Actions that can be triggered from the native menu bar.
#[derive(Debug, Clone)]
pub enum MenuAction {
    OpenFile,
    NewWindow,
    CloseTab,
    OpenSettings,
}

/// Holds the live muda `Menu` on macOS/Windows so it is not dropped.
/// On Linux this is an empty unit struct (zero cost).
pub struct NativeMenu {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    _menu: muda::Menu,
}

/// Build and register the native menu bar. Returns `None` on Linux.
pub fn setup(
    #[allow(unused_variables)] window_handle: raw_window_handle::RawWindowHandle,
    #[allow(unused_variables)] shortcuts: &crate::shortcuts::KeyboardShortcuts,
) -> Option<NativeMenu> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use muda::{
            Menu, MenuItem, PredefinedMenuItem, Submenu,
            accelerator::{Accelerator, Code, CMD_OR_CTRL},
        };

        let menu = Menu::new();

        // ── Thoth menu ─────────────────────────────────────────────────────
        let thoth_menu = Submenu::new("Thoth", true);
        let settings_item = MenuItem::with_id(
            "settings",
            "Settings",
            true,
            Some(Accelerator::new(Some(CMD_OR_CTRL), Code::Comma)),
        );

        #[cfg(target_os = "macos")]
        {
            let quit_item = PredefinedMenuItem::quit(Some("Quit Thoth"));
            let _ = thoth_menu.append_items(&[
                &settings_item,
                &PredefinedMenuItem::separator(),
                &quit_item,
            ]);
        }
        #[cfg(target_os = "windows")]
        {
            let _ = thoth_menu.append_items(&[&settings_item]);
        }

        // ── File menu ──────────────────────────────────────────────────────
        let file_menu = Submenu::new("File", true);
        let open_item = MenuItem::with_id(
            "open_file",
            "Open File…",
            true,
            Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyO)),
        );
        let new_window_item = MenuItem::with_id(
            "new_window",
            "New Window",
            true,
            Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyN)),
        );
        let close_tab_item = MenuItem::with_id(
            "close_tab",
            "Close Tab",
            true,
            Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyW)),
        );
        let _ = file_menu.append_items(&[
            &open_item,
            &new_window_item,
            &PredefinedMenuItem::separator(),
            &close_tab_item,
        ]);

        let _ = menu.append_items(&[&thoth_menu, &file_menu]);

        // ── Platform init ──────────────────────────────────────────────────
        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
        }
        #[cfg(target_os = "windows")]
        {
            if let raw_window_handle::RawWindowHandle::Win32(w) = window_handle {
                let hwnd = w.hwnd.get() as isize;
                let _ = unsafe { menu.init_for_hwnd(hwnd) };
            }
        }

        return Some(NativeMenu { _menu: menu });
    }

    // Linux: no native menu — egui fallback in toolbar.rs handles it.
    #[allow(unreachable_code)]
    None
}

/// Drain pending native menu events. Returns an empty Vec on Linux.
pub fn poll_events() -> Vec<MenuAction> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use muda::MenuEvent;
        let mut actions = Vec::new();
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let action = match event.id().0.as_str() {
                "open_file" => Some(MenuAction::OpenFile),
                "new_window" => Some(MenuAction::NewWindow),
                "close_tab" => Some(MenuAction::CloseTab),
                "settings" => Some(MenuAction::OpenSettings),
                _ => None,
            };
            if let Some(a) = action {
                actions.push(a);
            }
        }
        return actions;
    }

    #[allow(unreachable_code)]
    Vec::new()
}
