use eframe::egui::{self, Align2, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke};
use std::sync::Arc;
use std::time::SystemTime;

use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonGroupItem, ButtonGroups, ButtonSize, ButtonType, IconButton, List,
    ListEvent, ListItem, ListItemPostfix, ListItemPrefix, Typography, TypographyVariant,
};
use thoth_plugin_sdk::theme::color_to_hex;

use crate::{
    components::traits::ContextComponent,
    notification::{NotificationKind, NotificationManager, NotificationStatus},
    theme::{ThemeColors, phosphor_font_id},
};

type NotifAction = Arc<dyn Fn() + Send + Sync + 'static>;

// id, title, message, status, kind, unread, actions, created_at, pinned
type NotifRow = (
    String,
    String,
    String,
    NotificationStatus,
    NotificationKind,
    bool,
    Vec<(String, NotifAction)>,
    i64,
    bool,
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

        let btn = ui.add(
            IconButton::builder()
                .icon(bell_icon)
                .tooltip("Notifications")
                .frame(false)
                .maybe_badge_color(if unread_count > 0 {
                    Some(color_to_hex(colors.error))
                } else {
                    None
                })
                .selected(self.state.is_open)
                .build(),
        );

        if btn.clicked() {
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
                    .inner_margin(Margin {
                        left: 16,
                        right: 12,
                        top: 12,
                        bottom: 10,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            Typography::title(ui, "Notifications");
                            if unread_count > 0 {
                                ui.add_space(6.0);
                                Typography::caption(ui, &format!("{unread_count} unread"));
                            }
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .add(
                                        IconButton::builder()
                                            .icon(egui_phosphor::regular::X)
                                            .tooltip("Close")
                                            .frame(false)
                                            .size(20.0)
                                            .icon_size(13.0)
                                            .build(),
                                    )
                                    .clicked()
                                {
                                    close_panel = true;
                                }
                                ui.add_space(4.0);
                                if ui
                                    .add(
                                        IconButton::builder()
                                            .icon(egui_phosphor::regular::CHECKS)
                                            .tooltip("Mark all as read")
                                            .frame(false)
                                            .size(20.0)
                                            .icon_size(13.0)
                                            .disabled(unread_count == 0)
                                            .build(),
                                    )
                                    .clicked()
                                    && let Some(m) = crate::NOTIFICATION_MANAGER.get()
                                    && let Ok(mut nm) = m.lock()
                                {
                                    nm.mark_all_read();
                                }
                            });
                        });
                    });

                ui.add(egui::Separator::default().spacing(0.0));

                // ── Filter tabs ───────────────────────────────────────────────
                Frame::new()
                    .inner_margin(Margin {
                        left: 12,
                        right: 12,
                        top: 8,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        let selected = ButtonGroups::builder()
                            .items(vec![
                                ButtonGroupItem::builder().value("all").label("All").build(),
                                ButtonGroupItem::builder()
                                    .value("unread")
                                    .label("Unread")
                                    .build(),
                                ButtonGroupItem::builder()
                                    .value("plugins")
                                    .label("Plugins")
                                    .build(),
                                ButtonGroupItem::builder()
                                    .value("errors")
                                    .label("Errors")
                                    .build(),
                            ])
                            .active(new_filter.as_str())
                            .build()
                            .show(ui)
                            .inner;
                        if let Some(v) = selected {
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
                                    n.created_at,
                                    n.pinned,
                                )
                            })
                            .collect();
                        items.sort_by_key(|b| std::cmp::Reverse(b.7));
                        items
                    })
                    .unwrap_or_default();

                let visible: Vec<&NotifRow> = notifications
                    .iter()
                    .filter(
                        |(_, _, _, status, kind, unread, _, _, _)| match new_filter {
                            Filter::All => true,
                            Filter::Unread => *unread,
                            Filter::Plugins => *kind == NotificationKind::Plugin,
                            Filter::Errors => {
                                *status == NotificationStatus::Error
                                    || *kind == NotificationKind::Error
                                    || *kind == NotificationKind::Warn
                            }
                        },
                    )
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
                                    .filter(|row| date_bucket(row.7) == bucket_label)
                                    .collect();

                                if bucket.is_empty() {
                                    continue;
                                }

                                Frame::new()
                                    .inner_margin(Margin {
                                        left: 16,
                                        right: 16,
                                        top: 8,
                                        bottom: 8,
                                    })
                                    .show(ui, |ui| {
                                        Typography::group_label(ui, &bucket_label.to_uppercase());
                                    });

                                // Pre-build descriptions (formatted Strings that
                                // must outlive the ListItem borrows below).
                                let descs: Vec<String> = bucket
                                    .iter()
                                    .map(|row| {
                                        let ts = relative_time(row.7);
                                        if row.2.is_empty() {
                                            ts
                                        } else {
                                            format!("{}\n{ts}", row.2)
                                        }
                                    })
                                    .collect();

                                // Pre-build action labels for pinned rows (must own the Strings).
                                let action_labels: Vec<Option<String>> = bucket
                                    .iter()
                                    .map(|row| {
                                        if row.8 {
                                            row.6.first().map(|(label, _)| label.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();

                                let list_items: Vec<ListItem> = bucket
                                    .iter()
                                    .zip(descs.iter())
                                    .zip(action_labels.iter())
                                    .map(
                                        |(
                                            ((_, title, _, _, kind, unread, _, _, pinned), desc),
                                            action_label,
                                        )| {
                                            let (icon, icon_color) = kind_icon(*kind, colors);
                                            let postfix = if *pinned {
                                                action_label.as_deref().map(|label| {
                                                    ListItemPostfix::Button(
                                                        Button::builder()
                                                            .label(label)
                                                            .button_type(ButtonType::Elevated)
                                                            .color(ButtonColor::Primary)
                                                            .button_size(ButtonSize::Small)
                                                            .build(),
                                                    )
                                                })
                                            } else {
                                                Some(ListItemPostfix::IconButton(
                                                    IconButton::builder()
                                                        .icon(egui_phosphor::regular::X)
                                                        .tooltip("Dismiss")
                                                        .frame(false)
                                                        .size(18.0)
                                                        .icon_size(11.0)
                                                        .build(),
                                                ))
                                            };
                                            ListItem::builder()
                                                .title(title.clone())
                                                .description(desc.clone())
                                                .prefix(ListItemPrefix::Icon {
                                                    glyph: icon.to_string(),
                                                    color: Some(color_to_hex(icon_color)),
                                                })
                                                .maybe_accent(
                                                    (*unread).then(|| color_to_hex(icon_color)),
                                                )
                                                .maybe_postfix(postfix)
                                                .build()
                                        },
                                    )
                                    .collect();

                                let list_event = List::builder()
                                    .items(list_items)
                                    .shrink_to_fit(true)
                                    .build()
                                    .show(ui);

                                if let Some(ListEvent::PostfixClicked(idx)) = list_event
                                    && let Some(row) = bucket.get(idx)
                                {
                                    if row.8 {
                                        // Pinned row: postfix is an action button — fire it.
                                        if let Some((_, cb)) = row.6.first() {
                                            cb();
                                        }
                                    } else {
                                        to_dismiss = Some(row.0.clone());
                                    }
                                }

                                // Clicking a row fires the primary (first) action only.
                                if let Some(ListEvent::ItemClicked(idx)) = list_event
                                    && let Some(row) = bucket.get(idx)
                                    && let Some((_, cb)) = row.6.first()
                                {
                                    cb();
                                }
                            }
                        }
                    });

                // ── Footer ────────────────────────────────────────────────────
                ui.add(egui::Separator::default().spacing(0.0));
                Frame::new()
                    .inner_margin(Margin {
                        left: 14,
                        right: 14,
                        top: 8,
                        bottom: 8,
                    })
                    .fill(colors.bg_sunken)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    Button::builder()
                                        .label("Clear all")
                                        .button_type(ButtonType::Text)
                                        .color(ButtonColor::Default)
                                        .button_size(ButtonSize::Small)
                                        .build(),
                                )
                                .clicked()
                                && let Some(m) = crate::NOTIFICATION_MANAGER.get()
                                && let Ok(mut nm) = m.lock()
                            {
                                nm.clear_notifications();
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
        .inner_margin(Margin {
            left: 0,
            right: 0,
            top: 40,
            bottom: 40,
        })
        .show(ui, |ui| {
            ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(icon)
                        .font(phosphor_font_id(32.0))
                        .color(colors.surface_active),
                );
                ui.add_space(8.0);
                ui.add(
                    Typography::builder()
                        .text(title)
                        .variant(TypographyVariant::BodyLarge)
                        .bold(true)
                        .build(),
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

fn date_bucket(created_ms: i64) -> &'static str {
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let age_ms = (now_ms - created_ms).max(0);
    let day_ms: i64 = 24 * 60 * 60 * 1000;
    if age_ms < day_ms {
        "Today"
    } else if age_ms < 2 * day_ms {
        "Yesterday"
    } else {
        "Earlier"
    }
}

fn relative_time(created_ms: i64) -> String {
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let age_secs = ((now_ms - created_ms).max(0)) / 1000;
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
