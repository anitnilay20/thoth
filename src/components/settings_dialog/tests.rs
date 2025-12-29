use super::*;
use crate::components::traits::StatelessComponent;
use crate::settings::*;
use crate::theme::{Theme, ThemeColors};

// Helper to create test theme colors
fn create_test_theme_colors() -> ThemeColors {
    Theme::default().colors()
}

// Helper to run UI tests
fn run_ui_test<F>(mut f: F)
where
    F: FnMut(&mut egui::Ui),
{
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, &mut f);
    });
}

// ========================================================================
// General Tab Render Tests
// ========================================================================

#[test]
fn test_general_tab_renders() {
    run_ui_test(|ui| {
        let window_settings = WindowSettings::default();
        let ui_settings = UiSettings::default();

        let output = GeneralTab::render(
            ui,
            general::GeneralTabProps {
                window_settings: &window_settings,
                ui_settings: &ui_settings,
            },
        );

        // Should not generate events on initial render
        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_general_tab_window_width_event() {
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
// Appearance Tab Render Tests
// ========================================================================

#[test]
fn test_appearance_tab_renders() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();

        AppearanceTab::render(ui, &mut settings, &theme_colors);
    });
}

// ========================================================================
// Performance Tab Render Tests
// ========================================================================

#[test]
fn test_performance_tab_renders() {
    run_ui_test(|ui| {
        let performance_settings = PerformanceSettings::default();
        let theme_colors = create_test_theme_colors();

        let output = PerformanceTab::render(
            ui,
            performance::PerformanceTabProps {
                performance_settings: &performance_settings,
                theme_colors: &theme_colors,
            },
        );

        // Should not generate events on initial render
        assert_eq!(output.events.len(), 0);
    });
}

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
// Viewer Tab Render Tests
// ========================================================================

#[test]
fn test_viewer_tab_renders() {
    run_ui_test(|ui| {
        let viewer_settings = ViewerSettings::default();
        let theme_colors = create_test_theme_colors();

        let output = ViewerTab::render(
            ui,
            viewer::ViewerTabProps {
                viewer_settings: &viewer_settings,
                theme_colors: &theme_colors,
            },
        );

        // Should not generate events on initial render
        assert_eq!(output.events.len(), 0);
    });
}

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
    assert!(settings.syntax_highlighting);
}

// ========================================================================
// Shortcuts Tab Render Tests
// ========================================================================

#[test]
fn test_shortcuts_tab_renders() {
    run_ui_test(|ui| {
        let shortcuts = crate::shortcuts::KeyboardShortcuts::default();
        let theme_colors = create_test_theme_colors();

        let output = ShortcutsTab::render(
            ui,
            shortcuts::ShortcutsTabProps {
                shortcuts: &shortcuts,
                theme_colors: &theme_colors,
            },
        );

        // Should not generate events (shortcuts are read-only)
        assert_eq!(output.events.len(), 0);
    });
}

// ========================================================================
// Advanced Tab Render Tests
// ========================================================================

#[test]
fn test_advanced_tab_renders() {
    run_ui_test(|ui| {
        let dev_settings = DeveloperSettings::default();
        let theme_colors = create_test_theme_colors();

        let output = AdvancedTab::render(
            ui,
            advanced::AdvancedTabProps {
                dev_settings: &dev_settings,
                theme_colors: &theme_colors,
            },
        );

        // Should not generate events on initial render
        assert_eq!(output.events.len(), 0);
    });
}

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
    assert!(!settings.show_profiler);
}

// ========================================================================
// Updates Tab Render Tests
// ========================================================================

#[test]
fn test_updates_tab_renders_no_update() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: None,
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        // Should not generate events on initial render
        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_renders_with_checking_state() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();
        let update_state = crate::update::UpdateState::Checking;

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&update_state),
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_renders_with_idle_state() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();
        let update_state = crate::update::UpdateState::Idle;

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&update_state),
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_renders_with_update_available_state() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();

        let release = crate::update::ReleaseInfo {
            tag_name: "v0.3.0".to_string(),
            name: "Version 0.3.0".to_string(),
            body: "New features and bug fixes".to_string(),
            published_at: "2025-01-01T00:00:00Z".to_string(),
            html_url: "https://github.com/user/repo/releases/tag/v0.3.0".to_string(),
            prerelease: false,
            assets: vec![],
        };

        let update_state = crate::update::UpdateState::UpdateAvailable {
            latest_version: "0.3.0".to_string(),
            current_version: "0.2.16".to_string(),
            releases: vec![release],
        };

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&update_state),
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_renders_with_downloading_state() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();

        let update_state = crate::update::UpdateState::Downloading {
            progress: 0.5,
            version: "0.3.0".to_string(),
        };

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&update_state),
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_renders_with_ready_to_install_state() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();

        let update_state = crate::update::UpdateState::ReadyToInstall {
            version: "0.3.0".to_string(),
            path: std::path::PathBuf::from("/tmp/update.tar.gz"),
        };

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&update_state),
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_renders_with_installing_state() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();

        let update_state = crate::update::UpdateState::Installing;

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&update_state),
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_renders_with_error_state() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();

        let update_state =
            crate::update::UpdateState::Error(crate::error::ThothError::UpdateCheckError {
                reason: "Network error".to_string(),
            });

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&update_state),
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

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
    assert!(settings.auto_check);
    assert_eq!(settings.check_interval_hours, 24);
}

// ========================================================================
// Settings Dialog Tests
// ========================================================================

#[test]
fn test_settings_dialog_default() {
    let dialog = SettingsDialog::default();
    assert!(!dialog.open);
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
fn test_settings_dialog_open_with_tab() {
    let mut dialog = SettingsDialog::default();
    let settings = Settings::default();

    dialog.open_with_tab(&settings, Some(SettingsTab::Performance));
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
// render_tab_content Tests
// ========================================================================

#[test]
fn test_render_tab_content_general() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        SettingsDialog::render_tab_content(
            ui,
            SettingsTab::General,
            &mut settings,
            &theme_colors,
            None,
            "0.2.16",
            &mut dialog_events,
        );

        assert_eq!(dialog_events.len(), 0);
    });
}

#[test]
fn test_render_tab_content_appearance() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        SettingsDialog::render_tab_content(
            ui,
            SettingsTab::Appearance,
            &mut settings,
            &theme_colors,
            None,
            "0.2.16",
            &mut dialog_events,
        );

        assert_eq!(dialog_events.len(), 0);
    });
}

#[test]
fn test_render_tab_content_performance() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        SettingsDialog::render_tab_content(
            ui,
            SettingsTab::Performance,
            &mut settings,
            &theme_colors,
            None,
            "0.2.16",
            &mut dialog_events,
        );

        assert_eq!(dialog_events.len(), 0);
    });
}

#[test]
fn test_render_tab_content_viewer() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        SettingsDialog::render_tab_content(
            ui,
            SettingsTab::Viewer,
            &mut settings,
            &theme_colors,
            None,
            "0.2.16",
            &mut dialog_events,
        );

        assert_eq!(dialog_events.len(), 0);
    });
}

#[test]
fn test_render_tab_content_shortcuts() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        SettingsDialog::render_tab_content(
            ui,
            SettingsTab::Shortcuts,
            &mut settings,
            &theme_colors,
            None,
            "0.2.16",
            &mut dialog_events,
        );

        assert_eq!(dialog_events.len(), 0);
    });
}

#[test]
fn test_render_tab_content_updates() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        SettingsDialog::render_tab_content(
            ui,
            SettingsTab::Updates,
            &mut settings,
            &theme_colors,
            None,
            "0.2.16",
            &mut dialog_events,
        );

        assert_eq!(dialog_events.len(), 0);
    });
}

#[test]
fn test_render_tab_content_advanced() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        SettingsDialog::render_tab_content(
            ui,
            SettingsTab::Advanced,
            &mut settings,
            &theme_colors,
            None,
            "0.2.16",
            &mut dialog_events,
        );

        assert_eq!(dialog_events.len(), 0);
    });
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

    assert!(settings.sidebar_width >= 200.0);
    assert!(settings.sidebar_width <= 600.0);
}

// ========================================================================
// Integration Tests
// ========================================================================

#[test]
fn test_settings_round_trip() {
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

#[test]
fn test_all_tabs_render_without_panic() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        // Test all tabs render successfully
        for tab in SettingsTab::all() {
            SettingsDialog::render_tab_content(
                ui,
                *tab,
                &mut settings,
                &theme_colors,
                None,
                "0.2.16",
                &mut dialog_events,
            );
        }
    });
}

// ========================================================================
// Event Handling Tests
// ========================================================================

#[test]
fn test_event_handling_clones_work() {
    // Test that all event types can be cloned
    let general_events = vec![
        general::GeneralTabEvent::WindowWidthChanged(1920.0),
        general::GeneralTabEvent::WindowHeightChanged(1080.0),
        general::GeneralTabEvent::RememberSidebarStateChanged(true),
        general::GeneralTabEvent::ShowToolbarChanged(false),
        general::GeneralTabEvent::ShowStatusBarChanged(false),
        general::GeneralTabEvent::EnableAnimationsChanged(true),
        general::GeneralTabEvent::SidebarWidthChanged(400.0),
    ];

    for event in &general_events {
        let _cloned = event.clone();
    }

    let perf_events = vec![
        performance::PerformanceTabEvent::CacheSizeChanged(200),
        performance::PerformanceTabEvent::MaxRecentFilesChanged(20),
    ];

    for event in &perf_events {
        let _cloned = event.clone();
    }
}

#[test]
fn test_settings_dialog_not_open_returns_empty() {
    let ctx = egui::Context::default();
    let mut dialog = SettingsDialog::default();

    // Dialog is not open, should return empty output
    let output = dialog.render(
        &ctx,
        SettingsDialogProps {
            update_state: None,
            current_version: "0.2.16",
        },
    );

    assert!(output.new_settings.is_none());
    assert_eq!(output.events.len(), 0);
}

#[test]
fn test_settings_tab_copy_and_equality() {
    let tab1 = SettingsTab::General;
    let tab2 = SettingsTab::General;
    let tab3 = SettingsTab::Appearance;

    assert_eq!(tab1, tab2);
    assert_ne!(tab1, tab3);

    // Test Copy trait
    let tab_copy = tab1;
    assert_eq!(tab_copy, tab1);
}

#[test]
fn test_settings_dialog_event_debug() {
    let event1 = SettingsDialogEvent::CheckForUpdates;
    let event2 = SettingsDialogEvent::DownloadUpdate;
    let event3 = SettingsDialogEvent::InstallUpdate;

    // Should be able to debug print
    let debug1 = format!("{:?}", event1);
    let debug2 = format!("{:?}", event2);
    let debug3 = format!("{:?}", event3);

    assert!(debug1.contains("CheckForUpdates"));
    assert!(debug2.contains("DownloadUpdate"));
    assert!(debug3.contains("InstallUpdate"));
}

#[test]
fn test_updates_tab_with_release_without_body() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();

        let release = crate::update::ReleaseInfo {
            tag_name: "v0.3.0".to_string(),
            name: "Version 0.3.0".to_string(),
            body: String::new(), // Empty body
            published_at: "2025-01-01T00:00:00Z".to_string(),
            html_url: "https://github.com/user/repo/releases/tag/v0.3.0".to_string(),
            prerelease: false,
            assets: vec![],
        };

        let update_state = crate::update::UpdateState::UpdateAvailable {
            latest_version: "0.3.0".to_string(),
            current_version: "0.2.16".to_string(),
            releases: vec![release],
        };

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&update_state),
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_update_available_with_no_releases() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();

        let update_state = crate::update::UpdateState::UpdateAvailable {
            latest_version: "0.3.0".to_string(),
            current_version: "0.2.16".to_string(),
            releases: vec![], // No releases
        };

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&update_state),
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_all_update_states_are_covered() {
    // Ensure we have tests for all UpdateState variants
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();

        // Test each state
        let states = vec![
            crate::update::UpdateState::Idle,
            crate::update::UpdateState::Checking,
            crate::update::UpdateState::UpdateAvailable {
                latest_version: "0.3.0".to_string(),
                current_version: "0.2.16".to_string(),
                releases: vec![],
            },
            crate::update::UpdateState::Downloading {
                progress: 0.75,
                version: "0.3.0".to_string(),
            },
            crate::update::UpdateState::ReadyToInstall {
                version: "0.3.0".to_string(),
                path: std::path::PathBuf::from("/tmp/update.tar.gz"),
            },
            crate::update::UpdateState::Installing,
            crate::update::UpdateState::Error(crate::error::ThothError::UpdateCheckError {
                reason: "Test error".to_string(),
            }),
        ];

        for state in states {
            let _output = UpdatesTab::render(
                ui,
                updates::UpdatesTabProps {
                    update_settings: &update_settings,
                    update_state: Some(&state),
                    current_version: "0.2.16",
                    theme_colors: &theme_colors,
                },
            );
        }
    });
}
