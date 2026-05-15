use eframe::egui::{self, Align2, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke};
use std::sync::Arc;
use std::time::SystemTime;

use crate::{
    components::{
        button::{Button, ButtonColor, ButtonProps, ButtonSize, ButtonType},
        button_group::{ButtonGroup, ButtonGroupItem, ButtonGroupProps},
        icon_button::{IconButton, IconButtonProps},
        list::{List, ListItem, ListItemPostfix, ListItemPrefix, ListProps},
        traits::{ContextComponent, StatelessComponent},
        typography::{Typography, TypographyProps, TypographyVariant},
    },
    notification::{NotificationKind, NotificationManager, NotificationStatus},
    theme::{ThemeColors, phosphor_font_id},
};

type NotifAction = Arc<dyn Fn() + Send + Sync + 'static>;

// id, title, message, status, kind, unread, actions
type NotifRow = (
    String,
    String,
    String,
    NotificationStatus,
    NotificationKind,
    bool,
    Vec<(String, NotifAction)>,
);

// ── Filter ────────────────────────────────────────────────────────────────────

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Filter {
    #[default]
    All,
    Unread,
    Plugins,
    Errors,
}

impl Filter {
    fn as_str(self) -> &'static str {
        match self {
            Filter::All => "all",
            Filter::Unread => "unread",
            Filter::Plugins => "plugins",
            Filter::Errors => "errors",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "unread" => Filter::Unread,
            "plugins" => Filter::Plugins,
            "errors" => Filter::Errors,
            _ => Filter::All,
        }
    }
}

// ── Component ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct NotificationDropdown {
    state: State,
}

#[derive(Default)]
struct State {
    is_open: bool,
    filter: Filter,
}

pub struct NotificationDropdownProps;

impl ContextComponent for NotificationDropdown {
    type Props<'a> = NotificationDropdownProps;
    type Output = ();

    fn render(&mut self, ui: &mut egui::Ui, _props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let unread_count = crate::NOTIFICATION_MANAGER
            .get()
            .and_then(|m| m.lock().ok())
            .map(|nm| nm.unread_count())
            .unwrap_or(0);

        let bell_icon = if unread_count > 0 {
            egui_phosphor::regular::BELL_RINGING
        } else {
            egui_phosphor::regular::BELL
        };

        let btn = IconButton::render(
            ui,
            IconButtonProps {
                icon: bell_icon,
                tooltip: Some("Notifications"),
                frame: false,
                badge_color: if unread_count > 0 { Some(colors.error) } else { None },
                size: None,
                disabled: false,
                icon_size: None,
                selected: self.state.is_open,
            },
        );

        if btn.clicked {
            self.state.is_open = !self.state.is_open;
        }

        if self.state.is_open {
            self.render_panel(ui, &colors, unread_count);
        }
    }
}

impl NotificationDropdown {
    fn render_panel(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, unread_count: usize) {
        let mut close_panel = false;
        let mut to_dismiss: Option<String> = None;
        let mut new_filter = self.state.filter;

        egui::Window::new("##notification_panel")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .movable(false)
            .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-4.0, -28.0))
            .fixed_size(egui::vec2(380.0, 520.0))
            .frame(
                Frame::new()
                    .fill(colors.bg_panel)
                    .stroke(Stroke::new(1.0, colors.surface))
                    .corner_radius(CornerRadius::same(8)),
            )
            .show(ui.ctx(), |ui| {
                ui.set_min_width(380.0);
                ui.set_max_width(380.0);

                // ── Header ────────────────────────────────────────────────────
                Frame::new()
                    .inner_margin(Margin { left: 16, right: 12, top: 12, bottom: 10 })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            Typography::title(ui, "Notifications");
                            if unread_count > 0 {
                                ui.add_space(6.0);
                                Typography::caption(ui, &format!("{unread_count} unread"));
                            }
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if IconButton::render(
                                    ui,
                                    IconButtonProps {
                                        icon: egui_phosphor::regular::X,
                                        tooltip: Some("Close"),
                                        frame: false,
                                        badge_color: None,
                                        size: Some(egui::vec2(20.0, 20.0)),
                                        icon_size: Some(13.0),
                                        disabled: false,
                                        selected: false,
                                    },
                                )
                                .clicked
                                {
                                    close_panel = true;
                                }
                                ui.add_space(4.0);
                                if IconButton::render(
                                    ui,
                                    IconButtonProps {
                                        icon: egui_phosphor::regular::CHECKS,
                                        tooltip: Some("Mark all as read"),
                                        frame: false,
                                        badge_color: None,
                                        size: Some(egui::vec2(20.0, 20.0)),
                                        icon_size: Some(13.0),
                                        disabled: unread_count == 0,
                                        selected: false,
                                    },
                                )
                                .clicked
                                {
                                    if let Some(m) = crate::NOTIFICATION_MANAGER.get() {
                                        if let Ok(mut nm) = m.lock() {
                                            nm.mark_all_read();
                                        }
                                    }
                                }
                            });
                        });
                    });

                ui.add(egui::Separator::default().spacing(0.0));

                // ── Filter tabs ───────────────────────────────────────────────
                Frame::new()
                    .inner_margin(Margin { left: 12, right: 12, top: 8, bottom: 8 })
                    .show(ui, |ui| {
                        let out = ButtonGroup::render(
                            ui,
                            ButtonGroupProps {
                                items: &[
                                    ButtonGroupItem { value: "all", label: "All" },
                                    ButtonGroupItem { value: "unread", label: "Unread" },
                                    ButtonGroupItem { value: "plugins", label: "Plugins" },
                                    ButtonGroupItem { value: "errors", label: "Errors" },
                                ],
                                active: new_filter.as_str(),
                            },
                        );
                        if let Some(v) = out.selected {
                            new_filter = Filter::from_str(&v);
                        }
                    });

                // ── Notification list ─────────────────────────────────────────
                let notifications: Vec<NotifRow> = crate::NOTIFICATION_MANAGER
                    .get()
                    .and_then(|m| m.lock().ok())
                    .map(|nm| {
                        let mut items: Vec<NotifRow> = nm
                            .notifications
                            .values()
                            .map(|n| {
                                (
                                    n.id.clone(),
                                    n.title.clone(),
                                    n.message.clone(),
                                    n.status,
                                    n.kind,
                                    n.unread,
                                    n.actions.clone(),
                                )
                            })
                            .collect();
                        items.sort_by(|a, b| b.0.cmp(&a.0));
                        items
                    })
                    .unwrap_or_default();

                let visible: Vec<&NotifRow> = notifications
                    .iter()
                    .filter(|(_, _, _, status, kind, unread, _)| match new_filter {
                        Filter::All => true,
                        Filter::Unread => *unread,
                        Filter::Plugins => *kind == NotificationKind::Plugin,
                        Filter::Errors => {
                            *status == NotificationStatus::Error
                                || *kind == NotificationKind::Error
                        }
                    })
                    .collect();

                egui::ScrollArea::vertical()
                    .max_height(380.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if visible.is_empty() {
                            render_empty_state(ui, new_filter);
                        } else {
                            for bucket_label in ["Today", "Yesterday", "Earlier"] {
                                // Rows for this date bucket
                                let bucket: Vec<&NotifRow> = visible
                                    .iter()
                                    .copied()
                                    .filter(|(id, ..)| date_bucket(id) == bucket_label)
                                    .collect();

                                if bucket.is_empty() {
                                    continue;
                                }

                                Frame::new()
                                    .inner_margin(Margin {
                                        left: 16,
                                        right: 16,
                                        top: 8,
                                        bottom: 4,
                                    })
                                    .show(ui, |ui| {
                                        Typography::group_label(
                                            ui,
                                            &bucket_label.to_uppercase(),
                                        );
                                    });

                                // Pre-build descriptions (formatted Strings that
                                // must outlive the ListItem borrows below).
                                let descs: Vec<String> = bucket
                                    .iter()
                                    .map(|(id, _, message, ..)| {
                                        let ts = relative_time(id);
                                        if message.is_empty() {
                                            ts
                                        } else {
                                            format!("{message}\n{ts}")
                                        }
                                    })
                                    .collect();

                                let list_items: Vec<ListItem<'_>> = bucket
                                    .iter()
                                    .zip(descs.iter())
                                    .map(|((_, title, _, _, kind, unread, _), desc)| {
                                        let (icon, icon_color) = kind_icon(*kind, colors);
                                        ListItem {
                                            title: title.as_str(),
                                            description: Some(desc.as_str()),
                                            prefix: Some(ListItemPrefix::Icon {
                                                glyph: icon,
                                                color: Some(icon_color),
                                            }),
                                            badge: None,
                                            postfix: Some(ListItemPostfix::IconButton(
                                                IconButtonProps {
                                                    icon: egui_phosphor::regular::X,
                                                    tooltip: Some("Dismiss"),
                                                    frame: false,
                                                    badge_color: None,
                                                    size: Some(egui::vec2(18.0, 18.0)),
                                                    icon_size: Some(11.0),
                                                    disabled: false,
                                                    selected: false,
                                                },
                                            )),
                                            // selected drives the left accent border
                                            // and the hover highlight — maps to unread.
                                            selected: *unread,
                                            tags: &[],
                                        }
                                    })
                                    .collect();

                                let list_out = List::render(
                                    ui,
                                    ListProps {
                                        items: &list_items,
                                        empty_label: None,
                                        // Must be true so the List's inner ScrollArea
                                        // shrinks to content and doesn't conflict with
                                        // our outer ScrollArea.
                                        shrink_to_fit: true,
                                        show_separators: true,
                                        compact: false,
                                    },
                                );

                                if let Some(idx) = list_out.postfix_clicked {
                                    if let Some(row) = bucket.get(idx) {
                                        to_dismiss = Some(row.0.clone());
                                    }
                                }

                                // Clicking a row fires its registered actions.
                                if let Some(idx) = list_out.row_clicked {
                                    if let Some(row) = bucket.get(idx) {
                                        for (_, cb) in &row.6 {
                                            cb();
                                        }
                                    }
                                }
                            }
                        }
                    });

                // ── Footer ────────────────────────────────────────────────────
                ui.add(egui::Separator::default().spacing(0.0));
                Frame::new()
                    .inner_margin(Margin { left: 14, right: 14, top: 8, bottom: 8 })
                    .fill(colors.bg_sunken)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if Button::render(
                                ui,
                                ButtonProps {
                                    label: "Clear all".to_string(),
                                    button_type: ButtonType::Text,
                                    color: ButtonColor::Default,
                                    button_size: ButtonSize::Small,
                                    ..Default::default()
                                },
                            )
                            .clicked
                            {
                                if let Some(m) = crate::NOTIFICATION_MANAGER.get() {
                                    if let Ok(mut nm) = m.lock() {
                                        nm.clear_notifications();
                                    }
                                }
                            }
                        });
                    });
            });

        self.state.filter = new_filter;
        if close_panel {
            self.state.is_open = false;
        }
        if let Some(id) = to_dismiss {
            NotificationManager::remove_notification(&id);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn render_empty_state(ui: &mut egui::Ui, filter: Filter) {
    let (icon, title, body) = match filter {
        Filter::All | Filter::Unread => (
            egui_phosphor::regular::BELL,
            "All caught up",
            "No notifications yet",
        ),
        Filter::Plugins => (
            egui_phosphor::regular::PUZZLE_PIECE,
            "No plugin events",
            "Plugin activity will appear here",
        ),
        Filter::Errors => (
            egui_phosphor::regular::WARNING_CIRCLE,
            "No errors",
            "Errors and warnings will appear here",
        ),
    };

    let colors = ui.ctx().memory(|mem| {
        mem.data
            .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
            .unwrap_or_else(|| crate::theme::Theme::default().colors())
    });

    Frame::new()
        .inner_margin(Margin { left: 0, right: 0, top: 40, bottom: 40 })
        .show(ui, |ui| {
            ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(icon)
                        .font(phosphor_font_id(32.0))
                        .color(colors.surface_active),
                );
                ui.add_space(8.0);
                Typography::render(
                    ui,
                    TypographyProps {
                        text: title,
                        variant: TypographyVariant::BodyLarge,
                        bold: true,
                        ..Default::default()
                    },
                );
                ui.add_space(4.0);
                Typography::body_muted(ui, body);
            });
        });
}

fn kind_icon(kind: NotificationKind, colors: &ThemeColors) -> (&'static str, Color32) {
    match kind {
        NotificationKind::Success => (egui_phosphor::regular::CHECK_CIRCLE, colors.success),
        NotificationKind::Error => (egui_phosphor::regular::WARNING_CIRCLE, colors.error),
        NotificationKind::Warn => (egui_phosphor::regular::WARNING, colors.warning),
        NotificationKind::Update => (egui_phosphor::regular::ARROW_CIRCLE_UP, colors.info),
        NotificationKind::Plugin => (egui_phosphor::regular::PUZZLE_PIECE, colors.accent),
        NotificationKind::Tip => (egui_phosphor::regular::LIGHTBULB, colors.info),
        NotificationKind::Info => (egui_phosphor::regular::INFO, colors.info),
    }
}

fn date_bucket(id: &str) -> &'static str {
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let created_ms: u128 = id.parse().unwrap_or(0);
    let age_ms = now_ms.saturating_sub(created_ms);
    let day_ms: u128 = 24 * 60 * 60 * 1000;
    if age_ms < day_ms {
        "Today"
    } else if age_ms < 2 * day_ms {
        "Yesterday"
    } else {
        "Earlier"
    }
}

fn relative_time(id: &str) -> String {
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let created_ms: u128 = id.parse().unwrap_or(0);
    let age_secs = now_ms.saturating_sub(created_ms) / 1000;
    if age_secs < 60 {
        "just now".to_string()
    } else if age_secs < 3600 {
        format!("{}m ago", age_secs / 60)
    } else if age_secs < 86400 {
        format!("{}h ago", age_secs / 3600)
    } else {
        format!("{}d ago", age_secs / 86400)
    }
}
