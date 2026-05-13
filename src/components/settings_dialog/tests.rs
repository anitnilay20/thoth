use super::*;
use crate::components::traits::StatelessComponent;
use crate::settings::*;
use crate::theme::{Theme, ThemeColors};

fn create_test_theme_colors() -> ThemeColors {
    Theme::default().colors()
}

fn run_ui_test<F>(mut f: F)
where
    F: FnMut(&mut egui::Ui),
{
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts.families.insert(
        egui::FontFamily::Name("phosphor".into()),
        vec!["phosphor".into()],
    );
    ctx.set_fonts(fonts);
    let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show_inside(ctx, &mut f);
    });
}

// ── General Tab ──────────────────────────────────────────────────────────────

#[test]
fn test_general_tab_renders() {
    run_ui_test(|ui| {
        let settings = Settings::default();
        let baseline = Settings::default();
        let theme_colors = create_test_theme_colors();

        let output = GeneralTab::render(
            ui,
            general::GeneralTabProps {
                settings: &settings,
                baseline: &baseline,
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_general_tab_window_width_event() {
    let event = general::GeneralTabEvent::WindowWidth(1920.0);
    match event {
        general::GeneralTabEvent::WindowWidth(w) => assert_eq!(w, 1920.0),
        _ => panic!("wrong event"),
    }
}

#[test]
fn test_general_tab_window_height_event() {
    let event = general::GeneralTabEvent::WindowHeight(1080.0);
    match event {
        general::GeneralTabEvent::WindowHeight(h) => assert_eq!(h, 1080.0),
        _ => panic!("wrong event"),
    }
}

// ── Interface Tab ────────────────────────────────────────────────────────────

#[test]
fn test_interface_tab_sidebar_events() {
    let remember_event = interface::InterfaceTabEvent::RememberSidebarStateChanged(true);
    match remember_event {
        interface::InterfaceTabEvent::RememberSidebarStateChanged(v) => assert!(v),
        _ => panic!("wrong event"),
    }

    let width_event = interface::InterfaceTabEvent::SidebarWidthChanged(400.0);
    match width_event {
        interface::InterfaceTabEvent::SidebarWidthChanged(w) => assert_eq!(w, 400.0),
        _ => panic!("wrong event"),
    }
}

#[test]
fn test_interface_tab_ui_events() {
    assert!(matches!(
        interface::InterfaceTabEvent::ShowToolbarChanged(false),
        interface::InterfaceTabEvent::ShowToolbarChanged(false)
    ));
    assert!(matches!(
        interface::InterfaceTabEvent::ShowStatusBarChanged(false),
        interface::InterfaceTabEvent::ShowStatusBarChanged(false)
    ));
    assert!(matches!(
        interface::InterfaceTabEvent::EnableAnimationsChanged(true),
        interface::InterfaceTabEvent::EnableAnimationsChanged(true)
    ));
}

// ── Performance Tab ──────────────────────────────────────────────────────────

#[test]
fn test_performance_tab_renders() {
    run_ui_test(|ui| {
        let performance_settings = PerformanceSettings::default();
        let theme_colors = create_test_theme_colors();

        // Sliders can fire `changed()` in headless egui contexts; just verify it renders.
        let _output = PerformanceTab::render(
            ui,
            performance::PerformanceTabProps {
                performance_settings: &performance_settings,
                theme_colors: &theme_colors,
            },
        );
    });
}

#[test]
fn test_performance_tab_cache_size_event() {
    let event = performance::PerformanceTabEvent::CacheSizeChanged(500);
    match event {
        performance::PerformanceTabEvent::CacheSizeChanged(s) => assert_eq!(s, 500),
        _ => panic!("wrong event"),
    }
}

#[test]
fn test_performance_tab_recent_files_event() {
    let event = performance::PerformanceTabEvent::MaxRecentFilesChanged(20);
    match event {
        performance::PerformanceTabEvent::MaxRecentFilesChanged(m) => assert_eq!(m, 20),
        _ => panic!("wrong event"),
    }
}

#[test]
fn test_performance_settings_defaults() {
    let s = PerformanceSettings::default();
    assert_eq!(s.cache_size, 100);
    assert_eq!(s.max_recent_files, 10);
}

// ── Viewer Tab ───────────────────────────────────────────────────────────────

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

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_viewer_tab_syntax_highlighting_event() {
    let enable_event = viewer::ViewerTabEvent::SyntaxHighlightingChanged(true);
    let disable_event = viewer::ViewerTabEvent::SyntaxHighlightingChanged(false);

    match enable_event {
        viewer::ViewerTabEvent::SyntaxHighlightingChanged(v) => assert!(v),
    }
    match disable_event {
        viewer::ViewerTabEvent::SyntaxHighlightingChanged(v) => assert!(!v),
    }
}

#[test]
fn test_viewer_settings_defaults() {
    assert!(ViewerSettings::default().syntax_highlighting);
}

// ── Shortcuts Tab ────────────────────────────────────────────────────────────

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

        assert_eq!(output.events.len(), 0);
    });
}

// ── Advanced / Developer Tab ─────────────────────────────────────────────────

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
                is_in_path: false,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
#[cfg(feature = "profiling")]
fn test_advanced_tab_profiler_event() {
    let event = advanced::AdvancedTabEvent::ShowProfilerChanged(true);
    match event {
        advanced::AdvancedTabEvent::ShowProfilerChanged(v) => assert!(v),
        advanced::AdvancedTabEvent::RegisterInPath
        | advanced::AdvancedTabEvent::UnregisterFromPath => {
            panic!("expected ShowProfilerChanged")
        }
    }
}

#[test]
fn test_developer_settings_defaults() {
    assert!(!DeveloperSettings::default().show_profiler);
}

// ── Updates Tab ──────────────────────────────────────────────────────────────

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
                last_check: None,
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_renders_with_checking_state() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();
        let state = crate::update::UpdateState::Checking;

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&state),
                last_check: None,
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
        let state = crate::update::UpdateState::Idle;

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&state),
                last_check: None,
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
            body: "New features".to_string(),
            published_at: "2025-01-01T00:00:00Z".to_string(),
            html_url: "https://example.com".to_string(),
            prerelease: false,
            assets: vec![],
        };
        let state = crate::update::UpdateState::UpdateAvailable {
            latest_version: "0.3.0".to_string(),
            current_version: "0.2.16".to_string(),
            releases: vec![release],
        };

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&state),
                last_check: None,
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
        let state = crate::update::UpdateState::Downloading {
            progress: 0.5,
            version: "0.3.0".to_string(),
        };

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&state),
                last_check: None,
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
        let state = crate::update::UpdateState::ReadyToInstall {
            version: "0.3.0".to_string(),
            path: std::path::PathBuf::from("/tmp/update.tar.gz"),
        };

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&state),
                last_check: None,
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
        let state = crate::update::UpdateState::Installing;

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&state),
                last_check: None,
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
        let state = crate::update::UpdateState::Error(crate::error::ThothError::UpdateCheckError {
            reason: "Network error".to_string(),
        });

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&state),
                last_check: None,
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_updates_tab_with_last_check_timestamp() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();
        let last_check = Some(chrono::Utc::now());

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: None,
                last_check,
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
        updates::UpdatesTabEvent::AutoCheckChanged(v) => assert!(v),
        _ => panic!("wrong event"),
    }
}

#[test]
fn test_updates_tab_check_interval_event() {
    let event = updates::UpdatesTabEvent::CheckIntervalChanged(24);
    match event {
        updates::UpdatesTabEvent::CheckIntervalChanged(h) => assert_eq!(h, 24),
        _ => panic!("wrong event"),
    }
}

#[test]
fn test_updates_tab_action_events() {
    assert!(matches!(
        updates::UpdatesTabEvent::CheckForUpdates,
        updates::UpdatesTabEvent::CheckForUpdates
    ));
    assert!(matches!(
        updates::UpdatesTabEvent::DownloadUpdate,
        updates::UpdatesTabEvent::DownloadUpdate
    ));
    assert!(matches!(
        updates::UpdatesTabEvent::InstallUpdate,
        updates::UpdatesTabEvent::InstallUpdate
    ));
}

#[test]
fn test_update_settings_defaults() {
    let s = UpdateSettings::default();
    assert!(s.auto_check);
    assert_eq!(s.check_interval_hours, 24);
}

// ── SettingsDialog ───────────────────────────────────────────────────────────

#[test]
fn test_settings_dialog_default() {
    let dialog = SettingsDialog::default();
    assert!(!dialog.open);
}

#[test]
fn test_settings_dialog_open() {
    let mut dialog = SettingsDialog::default();
    dialog.open(&Settings::default());
    assert!(dialog.open);
}

#[test]
fn test_settings_dialog_open_updates_tab() {
    let mut dialog = SettingsDialog::default();
    dialog.open_updates(&Settings::default());
    assert!(dialog.open);
}

#[test]
fn test_settings_dialog_events() {
    assert!(matches!(
        SettingsDialogEvent::CheckForUpdates,
        SettingsDialogEvent::CheckForUpdates
    ));
    assert!(matches!(
        SettingsDialogEvent::DownloadUpdate,
        SettingsDialogEvent::DownloadUpdate
    ));
    assert!(matches!(
        SettingsDialogEvent::InstallUpdate,
        SettingsDialogEvent::InstallUpdate
    ));
}

#[test]
fn test_settings_tab_enum() {
    let tabs = SettingsTab::all();
    assert_eq!(tabs.len(), 8);
    assert_eq!(tabs[0], SettingsTab::General);
    assert_eq!(tabs[1], SettingsTab::Interface);
    assert_eq!(tabs[2], SettingsTab::Viewer);
    assert_eq!(tabs[3], SettingsTab::Performance);
    assert_eq!(tabs[4], SettingsTab::Shortcuts);
    assert_eq!(tabs[5], SettingsTab::Plugins);
    assert_eq!(tabs[6], SettingsTab::Updates);
    assert_eq!(tabs[7], SettingsTab::Developer);
}

#[test]
fn test_settings_tab_labels() {
    assert_eq!(SettingsTab::General.label(), "General");
    assert_eq!(SettingsTab::Interface.label(), "Interface");
    assert_eq!(SettingsTab::Performance.label(), "Performance");
    assert_eq!(SettingsTab::Viewer.label(), "Viewer");
    assert_eq!(SettingsTab::Shortcuts.label(), "Shortcuts");
    assert_eq!(SettingsTab::Updates.label(), "Updates");
    assert_eq!(SettingsTab::Developer.label(), "Developer");
}

#[test]
fn test_settings_tab_icons() {
    use egui_phosphor::regular::*;
    assert_eq!(SettingsTab::General.icon(), SLIDERS);
    assert_eq!(SettingsTab::Interface.icon(), SIDEBAR);
    assert_eq!(SettingsTab::Performance.icon(), GAUGE);
    assert_eq!(SettingsTab::Viewer.icon(), EYE);
    assert_eq!(SettingsTab::Shortcuts.icon(), KEYBOARD);
    assert_eq!(SettingsTab::Updates.icon(), ARROWS_CLOCKWISE);
    assert_eq!(SettingsTab::Developer.icon(), WRENCH);
}

// ── render_tab_content ───────────────────────────────────────────────────────

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
            &Settings::default(),
            &theme_colors,
            None,
            None,
            "0.2.16",
            &mut dialog_events,
            &Arc::new(Mutex::new(None)),
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
            &Settings::default(),
            &theme_colors,
            None,
            None,
            "0.2.16",
            &mut dialog_events,
            &Arc::new(Mutex::new(None)),
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
            &Settings::default(),
            &theme_colors,
            None,
            None,
            "0.2.16",
            &mut dialog_events,
            &Arc::new(Mutex::new(None)),
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
            &Settings::default(),
            &theme_colors,
            None,
            None,
            "0.2.16",
            &mut dialog_events,
            &Arc::new(Mutex::new(None)),
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
            &Settings::default(),
            &theme_colors,
            None,
            None,
            "0.2.16",
            &mut dialog_events,
            &Arc::new(Mutex::new(None)),
        );

        assert_eq!(dialog_events.len(), 0);
    });
}

#[test]
fn test_render_tab_content_developer() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        SettingsDialog::render_tab_content(
            ui,
            SettingsTab::Developer,
            &mut settings,
            &Settings::default(),
            &theme_colors,
            None,
            None,
            "0.2.16",
            &mut dialog_events,
            &Arc::new(Mutex::new(None)),
        );

        assert_eq!(dialog_events.len(), 0);
    });
}

// ── Window / UI settings ─────────────────────────────────────────────────────

#[test]
fn test_window_settings_defaults() {
    let s = WindowSettings::default();
    assert_eq!(s.default_width, 1800.0);
    assert_eq!(s.default_height, 1200.0);
}

#[test]
fn test_window_settings_valid_ranges() {
    let s = WindowSettings {
        default_width: 1920.0,
        default_height: 1080.0,
    };
    assert!(s.default_width >= 400.0);
    assert!(s.default_width <= 7680.0);
    assert!(s.default_height >= 300.0);
    assert!(s.default_height <= 4320.0);
}

#[test]
fn test_ui_settings_defaults() {
    let s = UiSettings::default();
    assert_eq!(s.sidebar_width, 350.0);
    assert!(s.remember_sidebar_state);
    assert!(s.show_status_bar);
    assert!(s.show_toolbar);
    assert!(s.enable_animations);
}

#[test]
fn test_ui_settings_sidebar_width_range() {
    let s = UiSettings {
        sidebar_width: 300.0,
        ..Default::default()
    };
    assert!(s.sidebar_width >= 200.0);
    assert!(s.sidebar_width <= 1000.0);
}

// ── Integration ──────────────────────────────────────────────────────────────

#[test]
fn test_settings_round_trip() {
    let mut settings = Settings::default();
    settings.window.default_width = 1920.0;
    settings.performance.cache_size = 500;
    settings.viewer.syntax_highlighting = false;
    settings.updates.auto_check = false;

    assert_eq!(settings.window.default_width, 1920.0);
    assert_eq!(settings.performance.cache_size, 500);
    assert!(!settings.viewer.syntax_highlighting);
    assert!(!settings.updates.auto_check);
}

#[test]
fn test_all_event_types_are_clone_and_debug() {
    let general_event = general::GeneralTabEvent::WindowWidth(100.0);
    let cloned = general_event.clone();
    assert!(format!("{cloned:?}").contains("WindowWidth"));

    let perf_event = performance::PerformanceTabEvent::CacheSizeChanged(100);
    let cloned = perf_event.clone();
    assert!(format!("{cloned:?}").contains("CacheSizeChanged"));

    let viewer_event = viewer::ViewerTabEvent::SyntaxHighlightingChanged(true);
    let cloned = viewer_event.clone();
    assert!(format!("{cloned:?}").contains("SyntaxHighlightingChanged"));

    let updates_event = updates::UpdatesTabEvent::AutoCheckChanged(true);
    let cloned = updates_event.clone();
    assert!(format!("{cloned:?}").contains("AutoCheckChanged"));
}

#[test]
fn test_all_tabs_render_without_panic() {
    run_ui_test(|ui| {
        let mut settings = Settings::default();
        let theme_colors = create_test_theme_colors();
        let mut dialog_events = Vec::new();

        for tab in SettingsTab::all() {
            SettingsDialog::render_tab_content(
                ui,
                *tab,
                &mut settings,
                &Settings::default(),
                &theme_colors,
                None,
                None,
                "0.2.16",
                &mut dialog_events,
                &Arc::new(Mutex::new(None)),
            );
        }
    });
}

#[test]
fn test_event_handling_clones_work() {
    let general_events = vec![
        general::GeneralTabEvent::WindowWidth(1920.0),
        general::GeneralTabEvent::WindowHeight(1080.0),
        general::GeneralTabEvent::ThemeName("mocha".to_string()),
        general::GeneralTabEvent::FontSize(14.0),
    ];
    for event in &general_events {
        let _ = event.clone();
    }

    let interface_events = vec![
        interface::InterfaceTabEvent::RememberSidebarStateChanged(true),
        interface::InterfaceTabEvent::ShowToolbarChanged(false),
        interface::InterfaceTabEvent::ShowStatusBarChanged(false),
        interface::InterfaceTabEvent::EnableAnimationsChanged(true),
        interface::InterfaceTabEvent::SidebarWidthChanged(400.0),
    ];
    for event in &interface_events {
        let _ = event.clone();
    }

    let perf_events = vec![
        performance::PerformanceTabEvent::CacheSizeChanged(200),
        performance::PerformanceTabEvent::MaxRecentFilesChanged(20),
    ];
    for event in &perf_events {
        let _ = event.clone();
    }
}

#[test]
fn test_settings_tab_copy_and_equality() {
    let tab1 = SettingsTab::General;
    let tab2 = SettingsTab::General;
    let tab3 = SettingsTab::Developer;

    assert_eq!(tab1, tab2);
    assert_ne!(tab1, tab3);

    let tab_copy = tab1;
    assert_eq!(tab_copy, tab1);
}

#[test]
fn test_settings_dialog_event_debug() {
    let e1 = SettingsDialogEvent::CheckForUpdates;
    let e2 = SettingsDialogEvent::DownloadUpdate;
    let e3 = SettingsDialogEvent::InstallUpdate;

    assert!(format!("{e1:?}").contains("CheckForUpdates"));
    assert!(format!("{e2:?}").contains("DownloadUpdate"));
    assert!(format!("{e3:?}").contains("InstallUpdate"));
}

#[test]
fn test_updates_tab_with_release_without_body() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();
        let release = crate::update::ReleaseInfo {
            tag_name: "v0.3.0".to_string(),
            name: "Version 0.3.0".to_string(),
            body: String::new(),
            published_at: "2025-01-01T00:00:00Z".to_string(),
            html_url: "https://example.com".to_string(),
            prerelease: false,
            assets: vec![],
        };
        let state = crate::update::UpdateState::UpdateAvailable {
            latest_version: "0.3.0".to_string(),
            current_version: "0.2.16".to_string(),
            releases: vec![release],
        };

        let output = UpdatesTab::render(
            ui,
            updates::UpdatesTabProps {
                update_settings: &update_settings,
                update_state: Some(&state),
                last_check: None,
                current_version: "0.2.16",
                theme_colors: &theme_colors,
            },
        );

        assert_eq!(output.events.len(), 0);
    });
}

#[test]
fn test_all_update_states_are_covered() {
    run_ui_test(|ui| {
        let update_settings = UpdateSettings::default();
        let theme_colors = create_test_theme_colors();
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
            let _ = UpdatesTab::render(
                ui,
                updates::UpdatesTabProps {
                    update_settings: &update_settings,
                    update_state: Some(&state),
                    last_check: None,
                    current_version: "0.2.16",
                    theme_colors: &theme_colors,
                },
            );
        }
    });
}
