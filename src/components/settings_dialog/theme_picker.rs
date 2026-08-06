// ThemePicker — design `screens.html` §2 “ThemePicker”: the `.tp-active` summary
// card, the `.tp-filter` bar, then `.tp-fam` labelled `.tp-grid`s of `.pc` mini
// chrome previews.

use eframe::egui::{
    self, Color32,
    text::{LayoutJob, TextFormat},
};

use thoth_plugin_sdk::components::{
    ButtonGroupItem, ButtonGroups, Input, Typography, TypographyVariant,
};
use thoth_plugin_sdk::theme::{
    FIELD_HEIGHT, FONT_BODY, FONT_CAPTION, FONT_CONTROL, RADIUS_PANEL, edge_stroke, with_alpha,
};

use crate::{
    components::{settings_dialog::helpers::dirty_dot, traits::StatelessComponent},
    settings::Settings,
    theme::{Theme, ThemeColors, phosphor_font_id},
};

pub struct ThemePicker;

pub struct ThemePickerProps<'a> {
    pub setting: &'a Settings,
    pub baseline: &'a Settings,
    pub colors: &'a ThemeColors,
}

#[derive(Debug, Clone)]
pub enum ThemePickerEvent {
    ThemeSelected(String),
}

pub struct ThemePickerOutput {
    pub events: Vec<ThemePickerEvent>,
}

/// The colours a preview card samples out of the theme it stands for.
/// These are the *previewed* theme's own palette, not the host's, which is why
/// the card is painted by hand instead of using the SDK's `Card`.
struct CardSwatches {
    /// `.pc{background:var(--bg)}` — the code area behind the JSON sample.
    base: Color32,
    /// `.pc .bar` / `.pc .foot{background:var(--mantle)}`.
    mantle: Color32,
    /// The 5th footer chip, `#313244` in the sheet.
    surface: Color32,
    /// `.pc{box-shadow:inset 0 0 0 1px var(--surface1)}` — the card's hairline.
    surface_raised: Color32,
    text: Color32,
    primary: Color32,
    key: Color32,
    string: Color32,
    number: Color32,
    boolean: Color32,
    punctuation: Color32,
}

impl CardSwatches {
    fn from_theme(theme: &Theme) -> Self {
        let c = theme.colors();
        Self {
            base: c.bg,
            mantle: c.bg_panel,
            surface: c.surface,
            surface_raised: c.surface_raised,
            text: c.fg,
            primary: c.accent,
            key: c.syntax_key,
            string: c.syntax_string,
            number: c.syntax_number,
            boolean: c.syntax_bool,
            punctuation: c.syntax_punctuation,
        }
    }
}

// ── Active theme card — design `.tp-active` ──────────────────────────────────

/// `.tp-active{padding:9px 11px}`.
const ACTIVE_PAD_X: i8 = 11;
const ACTIVE_PAD_Y: i8 = 9;
/// `.tp-active .dot{width:14px;height:14px}`.
const ACTIVE_DOT: f32 = 14.0;
/// `.tp-active{gap:10px}`.
const ACTIVE_GAP: f32 = 10.0;
/// `.tp-active .nm{font-size:13px;font-weight:600}`.
const ACTIVE_NAME: f32 = FONT_BODY;
/// `.tp-active{margin-bottom:12px}`.
const FILTER_TOP: f32 = 12.0;

// ── Filter bar — design `.tp-filter` ────────────────────────────────────────

/// `.tp-filter{gap:10px}` between the search field and the mode segments.
const FILTER_GAP: f32 = 10.0;
/// `.tp-filter{margin-bottom:14px}`.
const FILTER_BOTTOM: f32 = 14.0;

// ── Card grid — design `.tp-fam` + `.tp-grid` ───────────────────────────────

/// `.tp-fam{margin:6px 0 8px}`.
const FAMILY_TOP: f32 = 6.0;
const FAMILY_LABEL_GAP: f32 = 8.0;
/// `.tp-grid{margin-bottom:14px}`.
const FAMILY_BOTTOM: f32 = 14.0;
/// Space around the "nothing matched" line.
const EMPTY_PAD: f32 = 8.0;

/// `.tp-grid{grid-template-columns:repeat(auto-fill,minmax(220px,1fr))}` — cards
/// are at least this wide and then share the row equally.
const CARD_MIN_W: f32 = 220.0;
/// `.tp-grid{gap:12px}`, horizontally and between rows.
const CARD_GAP: f32 = 12.0;
/// `.pc .bar{height:24px}` — the faux window bar.
const CHROME_H: f32 = 24.0;
/// `.pc .code{padding:8px 10px}` around 4 lines at `line-height:1.5` × 10px.
const SAMPLE_TOP: f32 = 8.0;
const SAMPLE_FONT: f32 = 10.0;
const SAMPLE_LINE_H: f32 = 15.0;
const SAMPLE_LINES: f32 = 4.0;
const SAMPLE_H: f32 = SAMPLE_TOP * 2.0 + SAMPLE_LINE_H * SAMPLE_LINES;
/// `.pc .foot{padding:6px 9px}` around an 11px name line.
const FOOT_PAD_Y: f32 = 6.0;
const FOOT_H: f32 = FOOT_PAD_Y * 2.0 + 15.0;
/// The whole card is the three bands stacked.
const CARD_H: f32 = CHROME_H + SAMPLE_H + FOOT_H;

/// `.pc .bar{padding:0 9px}` / `.pc .code{padding:… 10px}` / `.pc .foot{padding:… 9px}`
/// — one inset for all three bands.
const CARD_PAD: f32 = 9.0;
/// `.pc.sel{box-shadow:inset 0 0 0 2px var(--accent)}`, else a hairline.
const CARD_BORDER_ON: f32 = 2.0;
const CARD_BORDER_OFF: f32 = 1.0;
/// `.lights i{width:11px}` with `.lights{gap:7px}`.
const LIGHT_R: f32 = 5.5;
const LIGHT_PITCH: f32 = 18.0;
/// `.pc .bar .mode{font-size:12px;opacity:.8}`.
const CHROME_GLYPH: f32 = FONT_CONTROL;
const CHROME_GLYPH_ALPHA: u8 = 204; // .8 of 255
/// `.pc .chip{width:12px;height:8px}` with `.foot{gap:5px}`.
const CHIP_W: f32 = 12.0;
const CHIP_H: f32 = 8.0;
const CHIP_GAP: f32 = 5.0;
const CHIP_PITCH: f32 = CHIP_W + CHIP_GAP;
/// `.pc .chip{border-radius:2px}` — below the radius ladder's smallest rung, so
/// (like the SDK's `.badge` chips) the handoff pins it as a raw value.
const CHIP_RADIUS: f32 = 2.0;
/// `.pc .foot .nm{margin-left:5px}` — on top of the 5px flex gap.
const NAME_GAP: f32 = CHIP_GAP * 2.0;
/// `.pc .check{width:16px;height:16px;top:-9px;right:9px}` with a 2px ring.
const BADGE_R: f32 = 8.0;
const BADGE_RING: f32 = 2.0;
const BADGE_TOP: f32 = -9.0;
const BADGE_INSET: f32 = 9.0;

impl StatelessComponent for ThemePicker {
    type Output = ThemePickerOutput;
    type Props<'a> = ThemePickerProps<'a>;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let colors = props.colors;
        let current_name = &props.setting.theme.name;
        let baseline_name = &props.baseline.theme.name;

        // ── Active theme card — `.tp-active` ─────────────────────────────────
        let active_sw = CardSwatches::from_theme(&props.setting.theme);
        let dirty = current_name != baseline_name;
        let (_, is_dark, family) = Theme::catalog()
            .iter()
            .find(|(n, _, _)| n == current_name)
            .cloned()
            .unwrap_or((String::new(), true, String::new()));
        let mode = if is_dark { "dark" } else { "light" };

        egui::Frame::NONE
            .fill(colors.surface)
            .stroke(edge_stroke(colors))
            .corner_radius(RADIUS_PANEL)
            .inner_margin(egui::Margin::symmetric(ACTIVE_PAD_X, ACTIVE_PAD_Y))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    // Every gap in the row is explicit.
                    ui.spacing_mut().item_spacing.x = 0.0;

                    // `.dot` — the active theme's own accent.
                    let (dot_rect, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(ACTIVE_DOT), egui::Sense::hover());
                    ui.painter().circle_filled(
                        dot_rect.center(),
                        ACTIVE_DOT / 2.0,
                        active_sw.primary,
                    );
                    ui.add_space(ACTIVE_GAP);

                    ui.add(
                        Typography::builder()
                            .text(current_name)
                            .variant(TypographyVariant::BodyLarge)
                            .size(ACTIVE_NAME)
                            .bold(true)
                            .build(),
                    );
                    if dirty {
                        dirty_dot(ui, colors);
                    }

                    // `.meta{margin-left:auto}` — family · mode at the far right.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        Typography::caption(ui, &format!("{family} · {mode}"));
                    });
                });
            });

        // ── Filter bar — `.tp-filter` ────────────────────────────────────────
        let filter_id = egui::Id::new("theme_picker_filter");
        let mode_id = egui::Id::new("theme_picker_mode");
        let mut filter: String = ui.ctx().data(|d| d.get_temp(filter_id).unwrap_or_default());
        let mut mode_filter: u8 = ui.ctx().data(|d| d.get_temp(mode_id).unwrap_or(0u8)); // 0=All 1=Dark 2=Light

        ui.add_space(FILTER_TOP);
        // The search field fills the row and the All/Dark/Light segments sit at
        // its right. Right-to-left so the fixed-width segments are allocated
        // first and the search takes what's left.
        //
        // The row is allocated one field tall: a centre-aligned horizontal
        // layout stretches each item's frame to the *whole* height it is given,
        // so laying this out in the pane's remaining space would open a void
        // above and below it.
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), FIELD_HEIGHT),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                // Every gap in the row is explicit.
                ui.spacing_mut().item_spacing.x = 0.0;

                const MODES: [(u8, &str); 3] = [(0, "All"), (1, "Dark"), (2, "Light")];
                let group = ButtonGroups::builder()
                    .id("theme_picker_mode")
                    .items(
                        MODES
                            .iter()
                            .map(|(v, label)| {
                                ButtonGroupItem::builder()
                                    .value(v.to_string())
                                    .label(*label)
                                    .build()
                            })
                            .collect::<Vec<_>>(),
                    )
                    .active(mode_filter.to_string())
                    .build();
                if let Some(picked) = group.show(ui).inner
                    && let Ok(v) = picked.parse::<u8>()
                {
                    mode_filter = v;
                }

                ui.add_space(FILTER_GAP);

                // Search input fills the remaining width (left of the segments)
                let mut input = Input::builder()
                    .value(filter.clone())
                    .placeholder("Filter themes…")
                    .icon(egui_phosphor::regular::MAGNIFYING_GLASS)
                    .rows(1)
                    .build();
                input.show(ui);
                filter = input.value;
            },
        );

        ui.ctx().data_mut(|d| {
            d.insert_temp(filter_id, filter.clone());
            d.insert_temp(mode_id, mode_filter);
        });

        // ── Filtered + grouped catalog ────────────────────────────────────────
        let filter_lower = filter.to_lowercase();
        let catalog = Theme::catalog();

        let filtered: Vec<(String, bool, String)> = catalog
            .iter()
            .filter(|&(name, is_dark, _)| {
                let mode_ok = match mode_filter {
                    1 => *is_dark,
                    2 => !is_dark,
                    _ => true,
                };
                let text_ok =
                    filter_lower.is_empty() || name.to_lowercase().contains(&filter_lower);
                mode_ok && text_ok
            })
            .cloned()
            .collect();

        if filtered.is_empty() {
            ui.add_space(EMPTY_PAD);
            Typography::body_muted(ui, &format!("No themes match \"{filter}\"."));
            ui.add_space(EMPTY_PAD);
            return ThemePickerOutput { events };
        }

        let mut families: Vec<String> = Vec::new();
        for (_, _, family) in &filtered {
            if !families.contains(family) {
                families.push(family.clone());
            }
        }

        // `repeat(auto-fill,minmax(220px,1fr))`: fit as many 220px columns as the
        // width allows, then let them share the leftover equally. Cards carry
        // their own gaps (item spacing is zeroed per row), so this is the exact
        // width a row of `cols` cards occupies.
        let avail = ui.available_width();
        let cols = ((avail + CARD_GAP) / (CARD_MIN_W + CARD_GAP))
            .floor()
            .max(1.0);
        let card_w = ((avail - CARD_GAP * (cols - 1.0)) / cols).max(CARD_MIN_W);
        let cols = cols as usize;

        ui.add_space(FILTER_BOTTOM);

        // No scroll area of its own: the pane around it already scrolls, and a
        // nested one would claim the pane's remaining height and strand the
        // grid at the bottom.
        for family in &families {
            let family_themes: Vec<_> = filtered.iter().filter(|(_, _, f)| f == family).collect();

            ui.add_space(FAMILY_TOP);
            Typography::group_label(ui, family);
            ui.add_space(FAMILY_LABEL_GAP);

            for row in family_themes.chunks(cols) {
                // `horizontal_top`, not `horizontal`: a centre-aligned row
                // would stretch each card's frame to the row's full height.
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (i, (name, is_dark, _)) in row.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(CARD_GAP);
                        }
                        let theme = Theme::from_name(name);
                        let sw = CardSwatches::from_theme(&theme);
                        let selected = name == current_name;

                        if render_card(ui, name, *is_dark, &sw, selected, card_w, colors) {
                            events.push(ThemePickerEvent::ThemeSelected(name.to_string()));
                        }
                    }
                });
                ui.add_space(CARD_GAP);
            }

            ui.add_space(FAMILY_BOTTOM - CARD_GAP);
        }

        ThemePickerOutput { events }
    }
}

/// Lay out one line of the card's JSON sample as a single galley, so the tokens
/// sit flush against each other whatever the monospace font's real advance is.
fn sample_line(
    painter: &egui::Painter,
    pos: egui::Pos2,
    font: &egui::FontId,
    tokens: &[(Color32, &str)],
) {
    let mut job = LayoutJob::default();
    for (color, text) in tokens {
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: *color,
                ..Default::default()
            },
        );
    }
    let galley = painter.layout_job(job);
    painter.galley(pos, galley, Color32::PLACEHOLDER);
}

/// One `.pc`: a mini window painted in the previewed theme's palette — chrome
/// bar, JSON sample, and a swatch footer carrying the theme's name.
fn render_card(
    ui: &mut egui::Ui,
    name: &str,
    is_dark: bool,
    sw: &CardSwatches,
    selected: bool,
    card_w: f32,
    host: &ThemeColors,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, CARD_H), egui::Sense::click());

    if !ui.is_rect_visible(rect) {
        return resp.clicked();
    }

    let p = ui.painter();
    let radius = egui::CornerRadius::from(RADIUS_PANEL);

    // `.pc{background:var(--bg)}`
    p.rect_filled(rect, radius, sw.base);

    // ── Chrome bar — `.pc .bar` ──────────────────────────────────────────────
    let chrome_rect = egui::Rect::from_min_size(rect.min, egui::vec2(card_w, CHROME_H));
    p.rect_filled(
        chrome_rect,
        egui::CornerRadius {
            nw: radius.nw,
            ne: radius.ne,
            sw: 0,
            se: 0,
        },
        sw.mantle,
    );

    let dot_y = chrome_rect.center().y;
    for (i, &dot_c) in [host.error, host.warning, host.success].iter().enumerate() {
        p.circle_filled(
            egui::pos2(
                rect.min.x + CARD_PAD + LIGHT_R + i as f32 * LIGHT_PITCH,
                dot_y,
            ),
            LIGHT_R,
            dot_c,
        );
    }

    // `.mode` — a Phosphor glyph, so it renders in every theme.
    let mode_icon = if is_dark {
        egui_phosphor::regular::MOON
    } else {
        egui_phosphor::regular::SUN
    };
    p.text(
        egui::pos2(chrome_rect.max.x - CARD_PAD, dot_y),
        egui::Align2::RIGHT_CENTER,
        mode_icon,
        phosphor_font_id(CHROME_GLYPH),
        with_alpha(sw.text, CHROME_GLYPH_ALPHA),
    );

    // ── JSON sample — `.pc .code` ────────────────────────────────────────────
    let sample_top = rect.min.y + CHROME_H + SAMPLE_TOP;
    let font = egui::FontId::monospace(SAMPLE_FONT);
    let ox = rect.min.x + CARD_PAD;
    let bool_txt = if is_dark { "true" } else { "false" };
    let lines: [&[(Color32, &str)]; 4] = [
        &[
            (sw.punctuation, "{ "),
            (sw.key, "\"name\""),
            (sw.punctuation, ": "),
            (sw.string, "\"thoth\""),
        ],
        &[
            (sw.key, "  \"version\""),
            (sw.punctuation, ": "),
            (sw.number, "42"),
            (sw.punctuation, ","),
        ],
        &[
            (sw.key, "  \"dark\""),
            (sw.punctuation, ": "),
            (sw.boolean, bool_txt),
        ],
        &[(sw.punctuation, "}")],
    ];
    // Keep a long sample inside the card even in a theme with a wide mono font.
    let sample_painter = p.with_clip_rect(
        p.clip_rect()
            .intersect(rect.shrink2(egui::vec2(CARD_PAD, 0.0))),
    );
    for (i, tokens) in lines.iter().enumerate() {
        sample_line(
            &sample_painter,
            egui::pos2(ox, sample_top + i as f32 * SAMPLE_LINE_H),
            &font,
            tokens,
        );
    }

    // ── Footer — `.pc .foot` ─────────────────────────────────────────────────
    let foot_top = rect.min.y + CHROME_H + SAMPLE_H;
    let foot_rect =
        egui::Rect::from_min_size(egui::pos2(rect.min.x, foot_top), egui::vec2(card_w, FOOT_H));
    p.rect_filled(
        foot_rect,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: radius.sw,
            se: radius.se,
        },
        sw.mantle,
    );

    // `.chip` — text, accent, string, number, surface, centred in the footer.
    let chips = [sw.text, sw.primary, sw.string, sw.number, sw.surface];
    let chip_y = foot_rect.center().y - CHIP_H / 2.0;
    for (i, &cc) in chips.iter().enumerate() {
        p.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.min.x + CARD_PAD + i as f32 * CHIP_PITCH, chip_y),
                egui::vec2(CHIP_W, CHIP_H),
            ),
            egui::CornerRadius::from(CHIP_RADIUS),
            cc,
        );
    }

    // `.foot .nm` — the theme's name after the chips, clear of the check badge.
    let name_x = rect.min.x + CARD_PAD + chips.len() as f32 * CHIP_PITCH + NAME_GAP - CHIP_GAP;
    let name_right = rect.max.x - CARD_PAD - if selected { BADGE_R * 2.0 } else { 0.0 };
    let name_galley = p.layout(
        name.to_owned(),
        egui::FontId::proportional(FONT_CAPTION),
        sw.text,
        (name_right - name_x).max(0.0),
    );
    p.with_clip_rect(p.clip_rect().intersect(foot_rect)).galley(
        egui::pos2(name_x, foot_rect.center().y - name_galley.size().y / 2.0),
        name_galley,
        Color32::PLACEHOLDER,
    );

    // `.check` — an accent disc straddling the footer's top edge, ringed in the
    // card's own background so it reads on any palette.
    if selected {
        let badge_center = egui::pos2(
            rect.max.x - BADGE_INSET - BADGE_R,
            foot_top + BADGE_TOP + BADGE_R,
        );
        p.circle_filled(badge_center, BADGE_R + BADGE_RING, sw.base);
        p.circle_filled(badge_center, BADGE_R, host.accent);
        p.text(
            badge_center,
            egui::Align2::CENTER_CENTER,
            egui_phosphor::regular::CHECK,
            phosphor_font_id(FONT_CAPTION),
            crate::theme::get_contrast_text_color(host.accent),
        );
    }

    // Border last, so it draws over the bands' square corners.
    let border_color = if selected {
        host.accent
    } else {
        sw.surface_raised
    };
    let border_width = if selected {
        CARD_BORDER_ON
    } else {
        CARD_BORDER_OFF
    };
    p.rect_stroke(
        rect.shrink(border_width * 0.5),
        radius,
        egui::Stroke::new(border_width, border_color),
        egui::StrokeKind::Middle,
    );

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    resp.clicked()
}
