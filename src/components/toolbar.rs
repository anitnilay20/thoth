use std::path::{Path, PathBuf};

use eframe::egui;

use crate::{
    components::{
        icon_button::{IconButton, IconButtonProps},
        traits::{ContextComponent, StatelessComponent},
    },
    file::lazy_loader::FileKind,
    shortcuts::KeyboardShortcuts,
};

// pick_file is only used by the Linux in-window menu bar.
#[cfg(target_os = "linux")]
use crate::app::pick_file;

#[derive(Default)]
pub struct Toolbar {
    pub previous_file_type: FileKind,
}

/// Props passed down to the Toolbar (immutable, one-way binding)
pub struct ToolbarProps<'a> {
    pub file_type: &'a FileKind,
    pub dark_mode: bool,
    pub shortcuts: &'a KeyboardShortcuts,
    pub file_path: Option<&'a Path>,
    pub is_fullscreen: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub plugins_enabled: bool,
}

/// Events emitted by the toolbar (bottom-to-top communication)
pub enum ToolbarEvent {
    FileOpen { path: PathBuf, file_type: FileKind },
    CloseTab,
    NewWindow,
    ToggleTheme,
    OpenSettings,
    NavigateBack,
    NavigateForward,
}

pub struct ToolbarOutput {
    pub events: Vec<ToolbarEvent>,
}

impl ContextComponent for Toolbar {
    type Props<'a> = ToolbarProps<'a>;
    type Output = ToolbarOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        self.render_ui(ui, props, &mut events);

        ToolbarOutput { events }
    }
}

impl Toolbar {
    fn render_ui(
        &mut self,
        ui: &mut egui::Ui,
        props: ToolbarProps<'_>,
        events: &mut Vec<ToolbarEvent>,
    ) {
        // Use theme colors from context
        let bg_color = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                .map(|c| c.bg_sunken)
                .unwrap_or(ui.ctx().global_style().visuals.extreme_bg_color)
        });

        // Row 1: Title bar (32px height - integrated with window controls, with title)
        // Hide completely in fullscreen mode
        if !props.is_fullscreen {
            egui::Panel::top("title_bar_row")
                .exact_size(32.0)
                .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin {
                    left: 8,
                    right: 8,
                    top: 0,
                    bottom: 0,
                }))
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);

                        #[cfg(target_os = "macos")]
                        let traffic_light_space = 70.0_f32;
                        #[cfg(not(target_os = "macos"))]
                        let traffic_light_space = 0.0_f32;

                        // Reserve space for macOS traffic lights
                        if traffic_light_space > 0.0 {
                            ui.add_space(traffic_light_space);
                        }

                        let button_size = egui::vec2(26.0, 26.0);

                        // Measure title text width so we can center the whole group
                        let title = if let Some(path) = props.file_path {
                            let filename = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Untitled");
                            format!("Thoth - {}", filename)
                        } else {
                            "Thoth".to_string()
                        };
                        let title_width = ui.fonts_mut(|f| {
                            f.layout_no_wrap(
                                title.clone(),
                                egui::FontId::proportional(13.0),
                                egui::Color32::WHITE,
                            )
                            .rect
                            .width()
                        });

                        // Group = back(26) + gap(2) + fwd(26) + gap(8) + title
                        let group_width = button_size.x + 2.0 + button_size.x + 8.0 + title_width;
                        let total_width = ui.max_rect().width();
                        // Center the group within the full panel, offset by traffic lights
                        let lead =
                            ((total_width - group_width) / 2.0 - traffic_light_space).max(0.0);
                        ui.add_space(lead);

                        // Navigation buttons
                        let back_btn = IconButton::render(
                            ui,
                            IconButtonProps {
                                icon: egui_phosphor::regular::CARET_LEFT,
                                frame: false,
                                tooltip: Some(&format!(
                                    "Go back ({})",
                                    props.shortcuts.nav_back.format()
                                )),
                                badge_color: None,
                                size: Some(button_size),
                                disabled: !props.can_go_back,
                                selected: false,
                                icon_size: None,
                            },
                        );
                        if back_btn.clicked {
                            events.push(ToolbarEvent::NavigateBack);
                        }

                        let fwd_btn = IconButton::render(
                            ui,
                            IconButtonProps {
                                icon: egui_phosphor::regular::CARET_RIGHT,
                                frame: false,
                                tooltip: Some(&format!(
                                    "Go forward ({})",
                                    props.shortcuts.nav_forward.format()
                                )),
                                badge_color: None,
                                size: Some(button_size),
                                disabled: !props.can_go_forward,
                                selected: false,
                                icon_size: None,
                            },
                        );
                        if fwd_btn.clicked {
                            events.push(ToolbarEvent::NavigateForward);
                        }

                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(title).size(13.0));
                    });
                });
        }

        // Row 2: In-window egui menu bar — only on Linux.
        // macOS and Windows use the native menu bar set up via muda in ThothApp::new().
        #[cfg(target_os = "linux")]
        egui::Panel::top("menu_bar_row")
            .exact_size(28.0)
            .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin {
                left: 4,
                right: 4,
                top: 0,
                bottom: 0,
            }))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);

                    let mut pending: Option<ToolbarEvent> = None;

                    // ── Thoth menu ─────────────────────────────────────────────
                    ui.menu_button("Thoth", |ui| {
                        if ui
                            .button(format!("Settings  {}", props.shortcuts.settings.format()))
                            .clicked()
                        {
                            pending = Some(ToolbarEvent::OpenSettings);
                            ui.close();
                        }
                    });
                    if let Some(e) = pending.take() {
                        events.push(e);
                    }

                    // ── File menu ──────────────────────────────────────────────
                    let plugins_enabled = props.plugins_enabled;
                    let open_shortcut = props.shortcuts.open_file.format();
                    let close_shortcut = props.shortcuts.close_tab.format();
                    let new_win_shortcut = props.shortcuts.new_window.format();

                    ui.menu_button("File", |ui| {
                        if ui.button(format!("Open File…  {open_shortcut}")).clicked() {
                            ui.close();
                            if let Some(path) = pick_file(plugins_enabled)
                                && let Some(file_type) = infer_file_type(&path)
                            {
                                pending = Some(ToolbarEvent::FileOpen { path, file_type });
                            }
                        }
                        if ui
                            .button(format!("New Window  {new_win_shortcut}"))
                            .clicked()
                        {
                            pending = Some(ToolbarEvent::NewWindow);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(format!("Close Tab  {close_shortcut}")).clicked() {
                            pending = Some(ToolbarEvent::CloseTab);
                            ui.close();
                        }
                    });
                    if let Some(e) = pending.take() {
                        events.push(e);
                    }
                });
            });
    }
}

pub fn infer_file_type_pub(path: &Path) -> Option<FileKind> {
    infer_file_type(path)
}

fn infer_file_type(path: &Path) -> Option<FileKind> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "ndjson" => Some(FileKind::Ndjson),
        "json" => Some(FileKind::Json),
        _ => {
            // Ask the plugin registry whether any plugin handles this extension
            // so we don't fall back to a stale file-type from the previous file.
            if let Some(Some(pm)) = crate::PLUGIN_MANAGER.get()
                && pm.find_loader_for_extension(&ext).is_some()
            {
                return Some(
                    if pm.plugin_has_capability(&ext, &crate::plugin::Capability::FileViewer) {
                        FileKind::PluginTable
                    } else {
                        FileKind::Plugin
                    },
                );
            }
            None
        }
    }
}
