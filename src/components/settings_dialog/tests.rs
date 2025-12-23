#[cfg(test)]
use super::*;
use crate::settings::*;

// ========================================================================
// General Tab Tests
// ========================================================================

#[test]
fn test_general_tab_window_width_event() {
    // We can't actually render without egui context, but we can test event logic
    // by directly creating events
    let event = general::GeneralTabEvent::WindowWidthChanged(1920.0);

    match event {
        general::GeneralTabEvent::WindowWidthChanged(width) => {
            assert_eq!(width, 1920.0);
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_general_tab_window_height_event() {
    let event = general::GeneralTabEvent::WindowHeightChanged(1080.0);

    match event {
        general::GeneralTabEvent::WindowHeightChanged(height) => {
            assert_eq!(height, 1080.0);
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_general_tab_sidebar_events() {
    let remember_event = general::GeneralTabEvent::RememberSidebarStateChanged(true);
    match remember_event {
        general::GeneralTabEvent::RememberSidebarStateChanged(value) => {
            assert!(value);
        }
        _ => panic!("Wrong event type"),
    }

    let width_event = general::GeneralTabEvent::SidebarWidthChanged(400.0);
    match width_event {
        general::GeneralTabEvent::SidebarWidthChanged(width) => {
            assert_eq!(width, 400.0);
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_general_tab_ui_events() {
    let toolbar_event = general::GeneralTabEvent::ShowToolbarChanged(false);
    let statusbar_event = general::GeneralTabEvent::ShowStatusBarChanged(false);
    let animations_event = general::GeneralTabEvent::EnableAnimationsChanged(true);

    assert!(matches!(
        toolbar_event,
        general::GeneralTabEvent::ShowToolbarChanged(false)
    ));
    assert!(matches!(
        statusbar_event,
        general::GeneralTabEvent::ShowStatusBarChanged(false)
    ));
    assert!(matches!(
        animations_event,
        general::GeneralTabEvent::EnableAnimationsChanged(true)
    ));
}

// ========================================================================
// Performance Tab Tests
// ========================================================================

#[test]
fn test_performance_tab_cache_size_event() {
    let event = performance::PerformanceTabEvent::CacheSizeChanged(500);

    match event {
        performance::PerformanceTabEvent::CacheSizeChanged(size) => {
            assert_eq!(size, 500);
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_performance_tab_recent_files_event() {
    let event = performance::PerformanceTabEvent::MaxRecentFilesChanged(20);

    match event {
        performance::PerformanceTabEvent::MaxRecentFilesChanged(max) => {
            assert_eq!(max, 20);
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_performance_settings_defaults() {
    let settings = PerformanceSettings::default();
    assert_eq!(settings.cache_size, 100);
    assert_eq!(settings.max_recent_files, 10);
}

// ========================================================================
// Viewer Tab Tests
// ========================================================================

#[test]
fn test_viewer_tab_syntax_highlighting_event() {
    let enable_event = viewer::ViewerTabEvent::SyntaxHighlightingChanged(true);
    let disable_event = viewer::ViewerTabEvent::SyntaxHighlightingChanged(false);

    match enable_event {
        viewer::ViewerTabEvent::SyntaxHighlightingChanged(enabled) => {
            assert!(enabled);
        }
    }

    match disable_event {
        viewer::ViewerTabEvent::SyntaxHighlightingChanged(enabled) => {
            assert!(!enabled);
        }
    }
}

#[test]
fn test_viewer_settings_defaults() {
    let settings = ViewerSettings::default();
    assert!(settings.syntax_highlighting); // Should default to true
}

// ========================================================================
// Advanced Tab Tests
// ========================================================================

#[test]
#[cfg(feature = "profiling")]
fn test_advanced_tab_profiler_event() {
    let event = advanced::AdvancedTabEvent::ShowProfilerChanged(true);

    match event {
        advanced::AdvancedTabEvent::ShowProfilerChanged(enabled) => {
            assert!(enabled);
        }
    }
}

#[test]
fn test_developer_settings_defaults() {
    let settings = DeveloperSettings::default();
    assert!(!settings.show_profiler); // Should default to false
}

// ========================================================================
// Updates Tab Tests
// ========================================================================

#[test]
fn test_updates_tab_auto_check_event() {
    let event = updates::UpdatesTabEvent::AutoCheckChanged(true);

    match event {
        updates::UpdatesTabEvent::AutoCheckChanged(enabled) => {
            assert!(enabled);
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_updates_tab_check_interval_event() {
    let event = updates::UpdatesTabEvent::CheckIntervalChanged(24);

    match event {
        updates::UpdatesTabEvent::CheckIntervalChanged(hours) => {
            assert_eq!(hours, 24);
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_updates_tab_action_events() {
    let check_event = updates::UpdatesTabEvent::CheckForUpdates;
    let download_event = updates::UpdatesTabEvent::DownloadUpdate;
    let install_event = updates::UpdatesTabEvent::InstallUpdate;

    assert!(matches!(
        check_event,
        updates::UpdatesTabEvent::CheckForUpdates
    ));
    assert!(matches!(
        download_event,
        updates::UpdatesTabEvent::DownloadUpdate
    ));
    assert!(matches!(
        install_event,
        updates::UpdatesTabEvent::InstallUpdate
    ));
}

#[test]
fn test_update_settings_defaults() {
    let settings = UpdateSettings::default();
    assert!(settings.auto_check); // Should default to true
    assert_eq!(settings.check_interval_hours, 24); // Default to 24 hours
}

// ========================================================================
// Settings Dialog Tests
// ========================================================================

#[test]
fn test_settings_dialog_default() {
    let dialog = SettingsDialog::default();
    assert!(!dialog.open); // Should start closed
}

#[test]
fn test_settings_dialog_open() {
    let mut dialog = SettingsDialog::default();
    let settings = Settings::default();

    dialog.open(&settings);
    assert!(dialog.open);
}

#[test]
fn test_settings_dialog_open_updates_tab() {
    let mut dialog = SettingsDialog::default();
    let settings = Settings::default();

    dialog.open_updates(&settings);
    assert!(dialog.open);
}

#[test]
fn test_settings_dialog_events() {
    let check_event = SettingsDialogEvent::CheckForUpdates;
    let download_event = SettingsDialogEvent::DownloadUpdate;
    let install_event = SettingsDialogEvent::InstallUpdate;

    assert!(matches!(check_event, SettingsDialogEvent::CheckForUpdates));
    assert!(matches!(
        download_event,
        SettingsDialogEvent::DownloadUpdate
    ));
    assert!(matches!(install_event, SettingsDialogEvent::InstallUpdate));
}

#[test]
fn test_settings_tab_enum() {
    let tabs = SettingsTab::all();
    assert_eq!(tabs.len(), 7);
    assert_eq!(tabs[0], SettingsTab::General);
    assert_eq!(tabs[1], SettingsTab::Appearance);
    assert_eq!(tabs[2], SettingsTab::Performance);
    assert_eq!(tabs[3], SettingsTab::Viewer);
    assert_eq!(tabs[4], SettingsTab::Shortcuts);
    assert_eq!(tabs[5], SettingsTab::Updates);
    assert_eq!(tabs[6], SettingsTab::Advanced);
}

#[test]
fn test_settings_tab_labels() {
    assert_eq!(SettingsTab::General.label(), "General");
    assert_eq!(SettingsTab::Appearance.label(), "Appearance");
    assert_eq!(SettingsTab::Performance.label(), "Performance");
    assert_eq!(SettingsTab::Viewer.label(), "Viewer");
    assert_eq!(SettingsTab::Shortcuts.label(), "Shortcuts");
    assert_eq!(SettingsTab::Updates.label(), "Updates");
    assert_eq!(SettingsTab::Advanced.label(), "Advanced");
}

#[test]
fn test_settings_tab_icons() {
    use egui_phosphor::regular::*;

    assert_eq!(SettingsTab::General.icon(), GEAR);
    assert_eq!(SettingsTab::Appearance.icon(), PAINT_BRUSH);
    assert_eq!(SettingsTab::Performance.icon(), GAUGE);
    assert_eq!(SettingsTab::Viewer.icon(), EYE);
    assert_eq!(SettingsTab::Shortcuts.icon(), KEYBOARD);
    assert_eq!(SettingsTab::Updates.icon(), ARROWS_CLOCKWISE);
    assert_eq!(SettingsTab::Advanced.icon(), WRENCH);
}

// ========================================================================
// Window Settings Tests
// ========================================================================

#[test]
fn test_window_settings_defaults() {
    let settings = WindowSettings::default();
    assert_eq!(settings.default_width, 1200.0);
    assert_eq!(settings.default_height, 800.0);
}

#[test]
fn test_window_settings_valid_ranges() {
    let settings = WindowSettings {
        default_width: 1920.0,
        default_height: 1080.0,
    };

    // Test that values are within reasonable ranges
    assert!(settings.default_width >= 800.0);
    assert!(settings.default_width <= 2560.0);
    assert!(settings.default_height >= 600.0);
    assert!(settings.default_height <= 1440.0);
}

// ========================================================================
// UI Settings Tests
// ========================================================================

#[test]
fn test_ui_settings_defaults() {
    let settings = UiSettings::default();
    assert_eq!(settings.sidebar_width, 350.0);
    assert!(settings.remember_sidebar_state);
    assert!(settings.show_status_bar);
    assert!(settings.show_toolbar);
    assert!(settings.enable_animations);
}

#[test]
fn test_ui_settings_sidebar_width_range() {
    let settings = UiSettings {
        sidebar_width: 300.0,
        ..Default::default()
    };

    // Test that sidebar width is within reasonable range
    assert!(settings.sidebar_width >= 200.0);
    assert!(settings.sidebar_width <= 600.0);
}

// ========================================================================
// Integration Tests
// ========================================================================

#[test]
fn test_settings_round_trip() {
    // Test that we can create settings, modify them, and values persist
    let mut settings = Settings::default();

    // Apply some changes
    settings.window.default_width = 1920.0;
    settings.performance.cache_size = 500;
    settings.viewer.syntax_highlighting = false;
    settings.updates.auto_check = false;

    // Verify changes persisted
    assert_eq!(settings.window.default_width, 1920.0);
    assert_eq!(settings.performance.cache_size, 500);
    assert!(!settings.viewer.syntax_highlighting);
    assert!(!settings.updates.auto_check);
}

#[test]
fn test_all_event_types_are_clone_and_debug() {
    // Ensure all event types implement Clone and Debug
    let general_event = general::GeneralTabEvent::WindowWidthChanged(100.0);
    let cloned = general_event.clone();
    assert!(format!("{:?}", cloned).contains("WindowWidthChanged"));

    let perf_event = performance::PerformanceTabEvent::CacheSizeChanged(100);
    let cloned = perf_event.clone();
    assert!(format!("{:?}", cloned).contains("CacheSizeChanged"));

    let viewer_event = viewer::ViewerTabEvent::SyntaxHighlightingChanged(true);
    let cloned = viewer_event.clone();
    assert!(format!("{:?}", cloned).contains("SyntaxHighlightingChanged"));

    let updates_event = updates::UpdatesTabEvent::AutoCheckChanged(true);
    let cloned = updates_event.clone();
    assert!(format!("{:?}", cloned).contains("AutoCheckChanged"));
}
