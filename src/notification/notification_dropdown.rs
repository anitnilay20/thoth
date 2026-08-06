use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, RichText, Sense,
};
use std::sync::Arc;
use std::time::SystemTime;

use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonGroupItem, ButtonGroups, ButtonSize, ButtonType, IconButton,
    IconButtonSelectedStyle, Typography, TypographyVariant,
};
use thoth_plugin_sdk::theme::{FONT_CONTROL, RADIUS_POPOVER, color_to_hex, with_alpha};

use crate::{
    components::traits::ContextComponent,
    notification::{NotificationKind, NotificationManager, NotificationStatus},
    theme::{RADIUS_CHIP, ThemeColors, edge_stroke, phosphor_font_id, popover_shadow},
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

// ── Design metrics ────────────────────────────────────────────────────────────
//
// Radii come from the ladder (`RADIUS_*`), never from the CSS pixel values.

/// Bell trigger. `overlays.html` draws `.bell` at 28px, but that is the floating
/// `.bellbar` context; here the trigger lives in the 26px status bar, whose items
/// are 22px `.chip`s (app-mockup.html). At 28px it would overflow the bar's clip
/// rect and lose a pixel off each corner, so it is chip-sized instead.
const BELL_SIZE: f32 = 22.0;
/// …with a glyph scaled to match (the design's 16px on a 28px box).
const BELL_GLYPH: f32 = 14.0;
/// Unread dot — design `.bell .bd{width:8px;height:8px}`…
const BELL_DOT: f32 = 8.0;
/// …tucked 3px in from the top-right corner (the component's own dot overhangs
/// the corner instead).
const BELL_DOT_INSET: f32 = 3.0;

/// Panel width — design `.npanel{width:380px}`.
const PANEL_W: f32 = 380.0;

/// Header padding — design `.nhead{padding:12px 12px 10px 16px}`.
const HEAD_MARGIN: Margin = Margin {
    left: 16,
    right: 12,
    top: 12,
    bottom: 10,
};
/// Gap between the header's title and its unread count — design `.nhead{gap:8px}`.
const HEAD_GAP: f32 = 8.0;
/// Unread count — design `.nhead .u{font-size:11.5px}`.
const HEAD_COUNT_FONT: f32 = 11.5;
/// Header actions — design `.nhead .ib{width:24px;height:24px}`…
const HEAD_ACTION: f32 = 24.0;
/// …with a 14px glyph…
const HEAD_ACTION_GLYPH: f32 = 14.0;
/// …spaced tightly — design `.nhead .acts{gap:2px}`.
const HEAD_ACTION_GAP: f32 = 2.0;

/// Filter tabs sit inset from the panel edges — design `.tabs{margin:0 16px 6px}`.
const TABS_MARGIN: Margin = Margin {
    left: 16,
    right: 16,
    top: 0,
    bottom: 6,
};

/// The list scrolls past this height — design `.nlist{max-height:300px}`.
const LIST_MAX_H: f32 = 300.0;
/// …and is inset from the panel edges — design `.nlist{padding:0 8px}`.
const LIST_MARGIN: Margin = Margin {
    left: 8,
    right: 8,
    top: 0,
    bottom: 0,
};

/// Date separator padding — design `.ndate{padding:9px 8px 5px}`.
const DATE_MARGIN: Margin = Margin {
    left: 8,
    right: 8,
    top: 9,
    bottom: 5,
};
/// …and its type — design `.ndate{font-size:10px;font-weight:700}`.
const DATE_FONT: f32 = 10.0;

/// Row padding — design `.nrow{padding:9px 8px}`.
const ROW_MARGIN: Margin = Margin {
    left: 8,
    right: 8,
    top: 9,
    bottom: 9,
};
/// Gap between a row's glyph, body, and dismiss — design `.nrow{gap:10px}`.
const ROW_GAP: f32 = 10.0;
/// Gap *after* every row, so the chip-radius cards read as separate cards rather
/// than one packed column, and so the last row is not flush against `.nfoot`.
///
/// `.nrow` itself declares no margin, but it is the same construct as
/// app-mockup's `.card` — a wash-filled, 3px-striped card in a scrolling stack
/// (`.card::before{top:9px;bottom:9px}` against `.nrow.unread::before{top:10px;
/// bottom:10px}`) — so its stack rule carries over: `.card{margin-bottom:6px}`.
const ROW_STACK_GAP: f32 = 6.0;
/// Hover wash — design `.nrow:hover{background:text@5%}`.
const ROW_HOVER_ALPHA: u8 = 13; // 5% of 255
/// Unread wash — design `.nrow.unread{background:mauve@7%}`.
const ROW_UNREAD_ALPHA: u8 = 18; // 7% of 255
/// Unread stripe — design `.nrow.unread::before{width:3px}`…
const STRIPE_W: f32 = 3.0;
/// …inset this far from the row's top and bottom edges…
const STRIPE_INSET_Y: f32 = 10.0;
/// …with its own tiny radius, below the ladder's smallest rung.
const STRIPE_RADIUS: f32 = 3.0;
/// Leading kind glyph — design `.nrow>i{font-size:17px}`…
const KIND_GLYPH: f32 = 17.0;
/// …nudged down off the row's top edge — design `.nrow>i{margin-top:1px}`.
const KIND_GLYPH_DROP: f32 = 1.0;
/// Message line — design `.nrow .nm{font-size:11.5px}`…
const MESSAGE_FONT: f32 = 11.5;
/// …just under the title — design `.nm{margin-top:1px}`.
const MESSAGE_GAP: f32 = 1.0;
/// Timestamp — design `.nrow .tm{font-size:10.5px}`…
const TIME_FONT: f32 = 10.5;
/// …below the message — design `.tm{margin-top:2px}`.
const TIME_GAP: f32 = 2.0;
/// Dismiss button — design `.nrow .dismiss{width:22px;height:22px}`…
const DISMISS_SIZE: f32 = 22.0;
/// …with a 13px glyph.
const DISMISS_GLYPH: f32 = 13.0;

/// Footer padding — design `.nfoot{padding:8px 12px}`. The footer already carries
/// its own 8px top padding, so the gap above "Clear all" is bought on the list
/// side (see [`ROW_STACK_GAP`]), not here.
const FOOT_MARGIN: Margin = Margin {
    left: 12,
    right: 12,
    top: 8,
    bottom: 8,
};

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

/// What the user did to one `.nrow` this frame.
enum RowEvent {
    /// The row (or a pinned row's action button) was clicked — fire its primary
    /// action.
    Activate,
    /// The row's dismiss button was clicked.
    Dismiss,
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

        // Design `.bell`: a ghost square that takes a 14% accent wash while its
        // panel is open, keeps its `--text` glyph in every state, and carries an
        // 8px unread dot ringed in the bar it sits on.
        let btn = ui.add(
            IconButton::builder()
                .icon(bell_icon)
                .tooltip("Notifications")
                .frame(false)
                .size_px(BELL_SIZE)
                .icon_size(BELL_GLYPH)
                .selected(self.state.is_open)
                .selected_style(IconButtonSelectedStyle::Wash)
                .glyph_color("fg")
                .maybe_badge_color((unread_count > 0).then(|| color_to_hex(colors.error)))
                .badge_size(BELL_DOT)
                .badge_inset(BELL_DOT_INSET)
                .badge_ring_color(color_to_hex(colors.bg_panel))
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
            // Design `.npanel{width:380px}` — pinned width, height hugs content.
            .min_width(PANEL_W)
            .max_width(PANEL_W)
            .frame(
                // Design `.npanel`: a mantle popover lifted by the menu shadow over
                // a hairline edge.
                Frame::new()
                    .fill(colors.bg_panel)
                    .stroke(edge_stroke(colors))
                    .corner_radius(RADIUS_POPOVER)
                    .shadow(popover_shadow(ui.visuals().dark_mode)),
            )
            .show(ui.ctx(), |ui| {
                // A flex column: each band carries its own padding, so there is no
                // spacing between them.
                ui.spacing_mut().item_spacing.y = 0.0;

                // ── Header ────────────────────────────────────────────────────
                Frame::new().inner_margin(HEAD_MARGIN).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = HEAD_GAP;
                        Typography::title(ui, "Notifications");
                        if unread_count > 0 {
                            ui.add(
                                Typography::builder()
                                    .text(format!("{unread_count} unread"))
                                    .variant(TypographyVariant::Caption)
                                    .size(HEAD_COUNT_FONT)
                                    .build(),
                            );
                        }
                        // Design `.nhead .acts{margin-left:auto;gap:2px}` — ghost
                        // `.ib`s, in sheet order (checks, then close) once the
                        // right-to-left layout reverses them.
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = HEAD_ACTION_GAP;
                            if ui
                                .add(
                                    IconButton::builder()
                                        .icon(egui_phosphor::regular::X)
                                        .tooltip("Close")
                                        .frame(false)
                                        .size_px(HEAD_ACTION)
                                        .icon_size(HEAD_ACTION_GLYPH)
                                        .build(),
                                )
                                .clicked()
                            {
                                close_panel = true;
                            }
                            if ui
                                .add(
                                    IconButton::builder()
                                        .icon(egui_phosphor::regular::CHECKS)
                                        .tooltip("Mark all as read")
                                        .frame(false)
                                        .size_px(HEAD_ACTION)
                                        .icon_size(HEAD_ACTION_GLYPH)
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

                // ── Filter tabs ───────────────────────────────────────────────
                // Design `.tabs`: a segmented control on a crust track, sized to
                // its segments rather than the panel width.
                Frame::new().inner_margin(TABS_MARGIN).show(ui, |ui| {
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
                    .max_height(LIST_MAX_H)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        Frame::new().inner_margin(LIST_MARGIN).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
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

                                    render_date_separator(ui, bucket_label, colors);

                                    for row in bucket {
                                        let event = render_row(ui, row, colors);
                                        // `.card{margin-bottom:6px}` semantics: the
                                        // gap trails *every* row, so the last one
                                        // also supplies the bottom inset that
                                        // `.nlist{padding:0 8px}` leaves at zero.
                                        ui.add_space(ROW_STACK_GAP);
                                        match event {
                                            // Clicking a row fires the primary
                                            // (first) action only.
                                            Some(RowEvent::Activate) => {
                                                if let Some((_, cb)) = row.6.first() {
                                                    cb();
                                                }
                                            }
                                            Some(RowEvent::Dismiss) => {
                                                to_dismiss = Some(row.0.clone());
                                            }
                                            None => {}
                                        }
                                    }
                                }
                            }
                        });
                    });

                // ── Footer ────────────────────────────────────────────────────
                Frame::new()
                    .inner_margin(FOOT_MARGIN)
                    .fill(colors.bg_sunken)
                    // Design `.npanel{overflow:hidden}` — the footer is the only
                    // filled band touching the panel edge, so it carries the
                    // bottom corners itself.
                    .corner_radius(CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: RADIUS_POPOVER as u8,
                        se: RADIUS_POPOVER as u8,
                    })
                    .show(ui, |ui| {
                        // Design `.nfoot .btn{margin:0 auto 0 0}` — left-aligned.
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

/// Design `.ndate`: a tiny bold upper-case bucket label.
fn render_date_separator(ui: &mut egui::Ui, label: &str, colors: &ThemeColors) {
    Frame::new().inner_margin(DATE_MARGIN).show(ui, |ui| {
        ui.add(
            Typography::builder()
                .text(label.to_uppercase())
                .variant(TypographyVariant::Label)
                .size(DATE_FONT)
                .bold(true)
                .color(color_to_hex(colors.fg_faint()))
                .build(),
        );
    });
}

/// Design `.nrow`: kind glyph, text block, and a hover-revealed dismiss, over a
/// chip-radius wash with an unread stripe on its left edge.
///
/// Hand-built rather than an SDK `List`: `.li` is a fixed-height row with one
/// 12px title and one 11px muted description, where `.nrow` grows with its
/// content and carries three distinct type tiers plus a 3px kind-coloured
/// stripe.
fn render_row(ui: &mut egui::Ui, row: &NotifRow, colors: &ThemeColors) -> Option<RowEvent> {
    let (id, title, message, _, kind, unread, actions, created_at, pinned) = row;
    let (glyph, kind_color) = kind_icon(*kind, colors);

    let row_id = ui.make_persistent_id(("notification_row", id));
    // `.dismiss{opacity:0}` is revealed by the row's `:hover`, but a row's rect is
    // only known once it has laid out — so reveal on last frame's hover, as the
    // SDK's `List` does for its trailing actions.
    let was_hovered = ui
        .ctx()
        .memory(|m| m.data.get_temp::<bool>(row_id).unwrap_or(false));

    let mut event = None;

    // Reserve a paint slot before the content so the wash draws behind the glyph
    // and text.
    let bg_slot = ui.painter().add(egui::Shape::Noop);

    let response = ui
        .push_id(row_id, |ui| {
            Frame::new()
                .inner_margin(ROW_MARGIN)
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.set_min_width(ui.available_width());
                        // Every gap inside a row is explicit.
                        ui.spacing_mut().item_spacing.x = 0.0;

                        // Design `.nrow>i` — the kind glyph, 1px down from the top.
                        let galley = ui.painter().layout_no_wrap(
                            glyph.to_string(),
                            phosphor_font_id(KIND_GLYPH),
                            kind_color,
                        );
                        let (glyph_rect, _) = ui.allocate_exact_size(
                            galley.size() + egui::vec2(0.0, KIND_GLYPH_DROP),
                            Sense::hover(),
                        );
                        ui.painter().galley(
                            glyph_rect.min + egui::vec2(0.0, KIND_GLYPH_DROP),
                            galley,
                            kind_color,
                        );
                        ui.add_space(ROW_GAP);

                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if *pinned {
                                // A pinned row keeps its primary action in place of
                                // the dismiss — it can't be swiped away.
                                if let Some((label, _)) = actions.first()
                                    && ui
                                        .add(
                                            Button::builder()
                                                .label(label.as_str())
                                                .button_type(ButtonType::Elevated)
                                                .color(ButtonColor::Primary)
                                                .button_size(ButtonSize::Small)
                                                .build(),
                                        )
                                        .clicked()
                                {
                                    event = Some(RowEvent::Activate);
                                }
                            } else if was_hovered {
                                if ui
                                    .add(
                                        IconButton::builder()
                                            .icon(egui_phosphor::regular::X)
                                            .tooltip("Dismiss")
                                            .frame(false)
                                            .size_px(DISMISS_SIZE)
                                            .icon_size(DISMISS_GLYPH)
                                            .build(),
                                    )
                                    .clicked()
                                {
                                    event = Some(RowEvent::Dismiss);
                                }
                            } else {
                                // The space stays reserved either way so the text
                                // doesn't reflow when the pointer arrives.
                                ui.allocate_exact_size(
                                    egui::Vec2::splat(DISMISS_SIZE),
                                    Sense::hover(),
                                );
                            }
                            ui.add_space(ROW_GAP);
                            render_row_body(ui, title, message, *created_at, colors);
                        });
                    });
                })
                .response
        })
        .inner;

    let hovered = ui.rect_contains_pointer(response.rect);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.ctx().memory_mut(|m| m.data.insert_temp(row_id, hovered));

    // `.nrow.unread` comes after `.nrow:hover` at equal specificity, so the
    // unread wash wins while the pointer is over an unread row.
    let fill = if *unread {
        Some(with_alpha(colors.accent, ROW_UNREAD_ALPHA))
    } else if hovered || was_hovered {
        Some(with_alpha(colors.fg, ROW_HOVER_ALPHA))
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().set(
            bg_slot,
            egui::Shape::rect_filled(response.rect, RADIUS_CHIP, fill),
        );
    }

    // Design `.nrow.unread::before` — a kind-coloured bar on the left edge,
    // inset from the row's top and bottom.
    if *unread {
        let stripe = egui::Rect::from_min_size(
            egui::pos2(response.rect.left(), response.rect.top() + STRIPE_INSET_Y),
            egui::vec2(
                STRIPE_W,
                (response.rect.height() - STRIPE_INSET_Y * 2.0).max(0.0),
            ),
        );
        ui.painter().rect_filled(stripe, STRIPE_RADIUS, kind_color);
    }

    if event.is_none() && hovered && ui.input(|i| i.pointer.primary_clicked()) {
        event = Some(RowEvent::Activate);
    }

    event
}

/// Design `.nrow .body`: the title, the one-line message, and the timestamp, as
/// one tight block against the row's top edge.
fn render_row_body(
    ui: &mut egui::Ui,
    title: &str,
    message: &str,
    created_at: i64,
    colors: &ThemeColors,
) {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(width, 0.0),
        Layout::top_down(Align::LEFT),
        |ui| {
            // Every offset in the block is explicit — design `.nm{margin-top:1px}`
            // and `.tm{margin-top:2px}`.
            ui.spacing_mut().item_spacing.y = 0.0;

            // `.nt` — 12.5px semibold.
            ui.add(
                Typography::builder()
                    .text(title)
                    .size(FONT_CONTROL)
                    .bold(true)
                    .build(),
            );

            if !message.is_empty() {
                ui.add_space(MESSAGE_GAP);
                // `.nm` is a single ellipsised line; `Typography` has no
                // truncating mode, so this line is a plain label on the same
                // size/colour tokens.
                ui.add(
                    egui::Label::new(
                        RichText::new(message)
                            .size(MESSAGE_FONT)
                            .color(colors.fg_subtle()),
                    )
                    .truncate(),
                );
            }

            // `.tm` — the relative timestamp.
            ui.add_space(TIME_GAP);
            ui.add(
                Typography::builder()
                    .text(relative_time(created_at))
                    .variant(TypographyVariant::Label)
                    .size(TIME_FONT)
                    .color(color_to_hex(colors.fg_faint()))
                    .build(),
            );
        },
    );
}

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
