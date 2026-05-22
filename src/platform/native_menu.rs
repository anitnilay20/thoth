/// Cross-platform native menu bar using muda.
///
/// - macOS: sets NSApplication.mainMenu (system menu bar at top of screen)
/// - Windows: attaches a Win32 menu bar to the window
/// - Linux: no-op (handled by egui in-window menu bar in toolbar.rs)
use muda::{
    Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, CMD_OR_CTRL},
};

/// Actions that can be triggered from the native menu bar.
#[derive(Debug, Clone)]
pub enum MenuAction {
    OpenFile,
    NewWindow,
    CloseTab,
    OpenSettings,
}

/// Holds the live `Menu` to prevent it from being dropped, which would
/// remove the menu bar from the application on some platforms.
pub struct NativeMenu {
    _menu: Menu,
}

/// Build and register the native menu bar for the current platform.
/// Returns `None` on Linux (egui fallback handles it instead).
pub fn setup(
    #[allow(unused_variables)] window_handle: raw_window_handle::RawWindowHandle,
    _shortcuts: &crate::shortcuts::KeyboardShortcuts,
) -> Option<NativeMenu> {
    let menu = Menu::new();

    // ── Thoth menu ─────────────────────────────────────────────────────────
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
    #[cfg(not(target_os = "macos"))]
    {
        let _ = thoth_menu.append_items(&[&settings_item]);
    }

    // ── File menu ──────────────────────────────────────────────────────────
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

    // ── Platform init ──────────────────────────────────────────────────────
    #[cfg(target_os = "macos")]
    {
        menu.init_for_nsapp();
        return Some(NativeMenu { _menu: menu });
    }

    #[cfg(target_os = "windows")]
    {
        if let raw_window_handle::RawWindowHandle::Win32(w) = window_handle {
            let hwnd = w.hwnd.get() as isize;
            let _ = unsafe { menu.init_for_hwnd(hwnd) };
            return Some(NativeMenu { _menu: menu });
        }
    }

    // Linux: no native init — egui menu bar used instead
    #[allow(unreachable_code)]
    None
}

/// Drain the muda event channel and return all pending menu actions.
/// Should be called every frame from `ThothApp::update()`.
pub fn poll_events() -> Vec<MenuAction> {
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
    actions
}
