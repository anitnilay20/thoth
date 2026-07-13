use crate::theme::icon_rich_text;
use eframe::egui;
use std::path::Path;

use crate::components::traits::ContextComponent;
use crate::consent::{
    manager::ConsentManager,
    modal::{ConsentModal, ConsentModalProps},
};
use crate::file::loaders::FileKind;
use crate::notification::notification_dropdown::{NotificationDropdown, NotificationDropdownProps};
use crate::settings::Settings;
use thoth_plugin_sdk::components::Breadcrumbs;

/// Status bar component displaying file info and application status
#[derive(Default)]
pub struct StatusBar {
    notification_dropdown: NotificationDropdown,
    consent_modal: ConsentModal,
}

/// Props for the status bar component (immutable, one-way binding)
pub struct StatusBarProps<'a> {
    /// Current file path (if any)
    pub file_path: Option<&'a Path>,

    /// File type
    pub file_type: &'a FileKind,

    /// Total item count
    pub item_count: usize,

    /// Filtered item count (if search is active)
    pub filtered_count: Option<usize>,

    /// Current status
    pub status: StatusBarStatus,

    /// Currently selected path in the JSON (for breadcrumbs)
    pub selected_path: Option<&'a str>,

    /// Set when the active tab is a plugin pane (its plugin id). The status bar
    /// then shows plugin-scoped info instead of file/item counts, and derives
    /// the status indicator from that plugin's live signals.
    pub active_plugin_id: Option<&'a str>,
}

/// Status indicator for the status bar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarStatus {
    Ready,
    Loading,
    Error,
    Searching,
    Filtered,
}

impl StatusBarStatus {
    /// Get the icon and text for this status
    pub fn icon_and_text(&self) -> (&'static str, &'static str) {
        match self {
            StatusBarStatus::Ready => ("⚡", "Ready"),
            StatusBarStatus::Loading => ("⏳", "Loading..."),
            StatusBarStatus::Error => ("⚠", "Error"),
            StatusBarStatus::Searching => ("🔍", "Searching..."),
            StatusBarStatus::Filtered => ("🔍", "Filtered"),
        }
    }

    /// Get the color for this status from theme
    pub fn color(&self, ctx: &egui::Context) -> egui::Color32 {
        ctx.memory(|mem| {
            let theme_colors = mem
                .data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| {
                    // Fallback: create default theme based on dark mode from visuals
                    let dark_mode = ctx.global_style().visuals.dark_mode;
                    crate::theme::Theme::for_dark_mode(dark_mode).colors()
                });

            match self {
                StatusBarStatus::Ready => theme_colors.success,
                StatusBarStatus::Loading => theme_colors.warning,
                StatusBarStatus::Error => theme_colors.error,
                StatusBarStatus::Searching | StatusBarStatus::Filtered => theme_colors.info,
            }
        })
    }
}

/// Events emitted by the status bar
#[derive(Debug, Clone)]
pub enum StatusBarEvent {
    /// User clicked on a breadcrumb to navigate
    NavigateToPath(String),
}

/// Output from status bar component
pub struct StatusBarOutput {
    pub events: Vec<StatusBarEvent>,
}

/// Render the live plugin signals from the host [`SignalRegistry`] as compact,
/// source-attributed chips: a status-colored dot, the plugin's short name, and
/// each `key value`. Draws nothing when no plugin has emitted a signal.
///
/// [`SignalRegistry`]: crate::plugin::signals
fn render_plugin_signals(ui: &mut egui::Ui) {
    use crate::plugin::signals::SignalStatus;

    let groups = crate::plugin::signals::snapshot();
    if groups.is_empty() {
        return;
    }

    let colors = ui.ctx().memory(|mem| {
        mem.data
            .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
            .unwrap_or_else(|| {
                let dark_mode = ui.ctx().global_style().visuals.dark_mode;
                crate::theme::Theme::for_dark_mode(dark_mode).colors()
            })
    });

    for group in groups {
        // "com.thoth.seshat" → "seshat"
        let short = group
            .plugin_id
            .rsplit('.')
            .next()
            .unwrap_or(group.plugin_id.as_str());
        for sig in &group.signals {
            ui.separator();
            let dot_color = match sig.status {
                SignalStatus::Ready => colors.success,
                SignalStatus::Loading => colors.warning,
                SignalStatus::Error => colors.error,
            };
            ui.colored_label(dot_color, "●");
            let text = if sig.value.is_empty() {
                format!("{} {}", short, sig.key)
            } else {
                format!("{} {} {}", short, sig.key, sig.value)
            };
            ui.label(text).on_hover_text(&group.plugin_id);
        }
    }
}

/// Render the active plugin pane's identity and its live signals as the primary
/// status-bar content: a plug glyph, the plugin's short name, then `key value`
/// chips (no per-chip source prefix, since the name is shown once). Used when a
/// plugin tab is focused, in place of the file/item info.
fn render_active_plugin_signals(ui: &mut egui::Ui, plugin_id: &str) {
    use crate::plugin::signals::SignalStatus;

    // "com.thoth.seshat" → "seshat"
    let short = plugin_id.rsplit('.').next().unwrap_or(plugin_id);
    ui.label(icon_rich_text(egui_phosphor::regular::PLUG, 12.0));
    ui.label(short);

    let groups = crate::plugin::signals::snapshot();
    let Some(group) = groups.iter().find(|g| g.plugin_id == plugin_id) else {
        return;
    };

    let colors = ui.ctx().memory(|mem| {
        mem.data
            .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
            .unwrap_or_else(|| {
                let dark_mode = ui.ctx().global_style().visuals.dark_mode;
                crate::theme::Theme::for_dark_mode(dark_mode).colors()
            })
    });

    for sig in &group.signals {
        ui.separator();
        let dot_color = match sig.status {
            SignalStatus::Ready => colors.success,
            SignalStatus::Loading => colors.warning,
            SignalStatus::Error => colors.error,
        };
        ui.colored_label(dot_color, "●");
        let text = if sig.value.is_empty() {
            sig.key.clone()
        } else {
            format!("{} {}", sig.key, sig.value)
        };
        ui.label(text);
    }
}

/// Aggregate a plugin's live signals into one status-bar indicator: `Error` if
/// any signal is errored, else `Loading` if any is loading, else `Ready`.
/// Returns `None` when the plugin has emitted no live signals (caller falls
/// back to the default status).
fn active_plugin_signal_status(plugin_id: &str) -> Option<StatusBarStatus> {
    use crate::plugin::signals::SignalStatus;

    let groups = crate::plugin::signals::snapshot();
    let group = groups.iter().find(|g| g.plugin_id == plugin_id)?;
    if group.signals.is_empty() {
        return None;
    }
    let status = if group
        .signals
        .iter()
        .any(|s| s.status == SignalStatus::Error)
    {
        StatusBarStatus::Error
    } else if group
        .signals
        .iter()
        .any(|s| s.status == SignalStatus::Loading)
    {
        StatusBarStatus::Loading
    } else {
        StatusBarStatus::Ready
    };
    Some(status)
}

impl ContextComponent for StatusBar {
    type Props<'a> = StatusBarProps<'a>;
    type Output = StatusBarOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();

        // Render consent modal (uses egui::Modal, so it floats above everything)
        let consent = ConsentManager::take_first();
        let (consent_request, allow_fn, deny_fn) = match consent {
            Some((r, a, d)) => (Some(r), Some(a), Some(d)),
            None => (None, None, None),
        };
        let id_accept = consent_request.as_ref().map(|r| r.id.clone());
        let id_cancel = id_accept.clone();
        let remember_domain = consent_request.as_ref().and_then(|r| r.domain.clone());
        let remember_plugin_id = consent_request.as_ref().and_then(|r| r.plugin_id.clone());

        let ctx = ui.ctx().clone();
        let on_accept = |remember: bool| {
            // Pass `remember` to the callback so the in-memory NetworkPolicy is
            // updated immediately (via runtime_allowed_handle).
            if let Some(ref f) = allow_fn {
                f(remember);
            }
            if let Some(ref id) = id_accept {
                ConsentManager::resolve(id);
            }
            if remember {
                // Also persist to Settings so the domain survives a restart.
                if let (Some(domain), Some(plugin_id)) =
                    (remember_domain.as_deref(), remember_plugin_id.as_deref())
                {
                    let domain = domain.to_string();
                    let plugin_id = plugin_id.to_string();
                    Settings::update(&ctx, |s| {
                        let domains = &mut s
                            .plugins
                            .network_policies
                            .entry(plugin_id)
                            .or_default()
                            .allowed_domains;
                        if !domains.contains(&domain) {
                            domains.push(domain);
                        }
                    });
                }
            }
        };
        let on_cancel = || {
            if let Some(ref f) = deny_fn {
                f(false);
            }
            if let Some(ref id) = id_cancel {
                ConsentManager::resolve(id);
            }
        };
        self.consent_modal.render(
            ui,
            ConsentModalProps {
                request: consent_request,
                on_accept: &on_accept,
                on_cancel: &on_cancel,
            },
        );

        // Use theme colors from context
        let bg_color = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| {
                    // Fallback: create default theme based on dark mode from visuals
                    let dark_mode = ui.ctx().global_style().visuals.dark_mode;
                    crate::theme::Theme::for_dark_mode(dark_mode).colors()
                })
                .bg_sunken
        });

        egui::Panel::bottom("status_bar")
            .exact_size(24.0)
            .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 4,
                bottom: 4,
            }))
            .show_inside(ui, |ui| {
                // Use theme text color from context
                let text_color = ui.ctx().memory(|mem| {
                    mem.data
                        .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                        .unwrap_or_else(|| {
                            // Fallback: create default theme based on dark mode from visuals
                            let dark_mode = ui.ctx().global_style().visuals.dark_mode;
                            crate::theme::Theme::for_dark_mode(dark_mode).colors()
                        })
                        .fg
                });
                ui.style_mut().visuals.override_text_color = Some(text_color);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                    if let Some(plugin_id) = props.active_plugin_id {
                        // Plugin pane tab: file/item counts are meaningless here,
                        // so show the plugin and its live signals instead.
                        render_active_plugin_signals(ui, plugin_id);
                    } else {
                        // File tab: filename, item count, file type, then any
                        // background plugin signals, then breadcrumbs.

                        // Filename with icon
                        if let Some(path) = props.file_path {
                            let filename = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Untitled");
                            ui.label(icon_rich_text(egui_phosphor::regular::FILE_TEXT, 12.0));
                            ui.label(filename);
                            ui.separator();
                        }

                        // Item count with icon
                        if let Some(filtered) = props.filtered_count {
                            ui.label(icon_rich_text(egui_phosphor::regular::FUNNEL, 12.0));
                            ui.label(format!("{} of {} items", filtered, props.item_count));
                        } else if props.item_count > 0 {
                            ui.label(icon_rich_text(egui_phosphor::regular::LIST_BULLETS, 12.0));
                            ui.label(format!("{} items", props.item_count));
                        } else {
                            ui.label(icon_rich_text(egui_phosphor::regular::LIST_BULLETS, 12.0));
                            ui.label("No items");
                        }

                        ui.separator();

                        // File type with icon
                        let file_type_icon = match props.file_type {
                            FileKind::Json => egui_phosphor::regular::BRACKETS_CURLY,
                            FileKind::Ndjson => egui_phosphor::regular::LIST_DASHES,
                            FileKind::Plugin => egui_phosphor::regular::PLUG,
                            FileKind::PluginTable => egui_phosphor::regular::TABLE,
                        };
                        ui.label(icon_rich_text(file_type_icon, 12.0));
                        ui.label(format!("{:?}", props.file_type));

                        // Live plugin signals (push channel), grouped by source.
                        // Renders nothing when no plugin has emitted.
                        render_plugin_signals(ui);

                        // Breadcrumbs navigation
                        if props.selected_path.is_some() {
                            ui.separator();
                            let clicked = Breadcrumbs::builder()
                                .maybe_path(props.selected_path.map(|s| s.to_string()))
                                .build()
                                .show(ui)
                                .inner;

                            // Convert breadcrumb click to status bar event
                            if let Some(path) = clicked {
                                events.push(StatusBarEvent::NavigateToPath(path));
                            }
                        }
                    }

                    // Push status to the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status indicator
                        ui.add_space(0.0);
                        self.notification_dropdown
                            .render(ui, NotificationDropdownProps {});
                        // On a plugin tab, reflect that plugin's aggregated signal
                        // state (loading / error / ready); otherwise the file status.
                        let status = props
                            .active_plugin_id
                            .and_then(active_plugin_signal_status)
                            .unwrap_or(props.status);
                        let (icon, text) = status.icon_and_text();
                        let status_color = status.color(ui.ctx());
                        ui.colored_label(status_color, format!("{} {}", icon, text));
                    });
                });
            });

        StatusBarOutput { events }
    }
}
