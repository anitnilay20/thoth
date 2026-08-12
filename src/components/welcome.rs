// WelcomePanel — design sheet `welcome.html` (`.welcome`), which supersedes the
// slimmer `screens.html` §1 sheet this module was first built from. One calm
// composition: a hero, three `.acard` start actions, a drop affordance, and a
// Recent / Tips column pair, all inside a 980px `.wrap` centred in the pane.
//
// The `.welcome` panel *chrome* (base fill, `--p-shadow`, `--edge`, 16px radius)
// is deliberately not painted here: `ThothApp::render_dock` already wraps the
// whole dock area — this panel included — in exactly that frame
// (`fill(bg)` + `RADIUS_PANEL` + `panel_shadow` + `edge_stroke`), so painting it
// again would double the edge and the shadow. This module owns everything inside
// it, including the `.wrap` padding: `CentralPanel::render_ui` drops its 8px
// content margin for the welcome screen so 40/44 lands exactly.
//
// Most boxes on the sheet have no SDK counterpart (`.mark`, `.acard`, `.drop`,
// the tinted `.tile`s, the `.kbd`/`.ext` chips), and every clickable box is a
// single hover/click target with its contents *painted* into it — a child widget
// laid on top would shadow the box's own hover and eat its click. Those are
// hand-painted from theme tokens; `Typography` carries the plain text flows.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use eframe::egui;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align2, Color32, FontId, Galley, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use egui_phosphor::regular as ph;
use thoth_plugin_sdk::components::{Typography, TypographyVariant};

use crate::theme::{
    FONT_CAPTION, FONT_CONTROL, ICON_CONTROL, RADIUS_CHECK, RADIUS_CHIP, RADIUS_CONTROL,
    RADIUS_PANEL, RADIUS_PILL, ThemeColors, edge_stroke, phosphor_font_id, semibold_font_id,
    with_alpha,
};

// ── Alpha ladder — the sheet's `color-mix(in oklab, X N%, transparent)` ───────

/// `.rrow:hover{background:text 5%}`.
const A_05: u8 = 13;
/// `.drop:hover{background:mauve 8%}`.
const A_08: u8 = 20;
/// The `.tile` / `.ic` tints (`t-mauve`/`t-blue`/`t-green`/`t-peach`, 16%).
const A_16: u8 = 41;
/// `.drop{background:surface0 30%}`.
const A_30: u8 = 77;
/// `.mark{box-shadow:… mauve 34%}`.
const A_34: u8 = 87;
/// `.drop{box-shadow:inset … surface1 55%}` and its mauve hover twin.
const A_55: u8 = 140;
/// The plugins card's `.kbd` carries `style="opacity:.35"` — no shortcut is
/// bound to it yet, so the chip is present but dimmed.
const A_35_OPACITY: u8 = 89;

// ── Wrap — design `.wrap` ────────────────────────────────────────────────────

/// `.wrap{max-width:980px}`. `box-sizing:border-box`, so the side padding below
/// is *inside* this width.
///
/// Public so the layout tests assert against the token rather than a copy of it.
pub const WRAP_MAX_W: f32 = 980.0;
/// `.wrap{padding:40px 44px}`.
///
/// Public so the layout tests assert against the token rather than a copy of it.
pub const WRAP_PAD_X: f32 = 44.0;
const WRAP_PAD_Y: f32 = 40.0;

// ── Hero — design `.hero` ────────────────────────────────────────────────────

/// `.hero{gap:16px}` — between the mark, the text block, and the version pill.
const HERO_GAP: f32 = 16.0;
/// `.hero{margin-bottom:30px}`.
const HERO_BOTTOM: f32 = 30.0;
/// `.mark{width:52px;height:52px}`.
const MARK: f32 = 52.0;
/// `.mark{font-size:28px}`.
const MARK_GLYPH: f32 = 28.0;
/// `.mark{box-shadow:0 6px 18px color-mix(in oklab,var(--mauve) 34%,transparent)}`
/// — hand-rolled rather than [`crate::theme::glow_shadow`], which is the
/// primary button's tighter `0 3px 10px @38%`.
const MARK_GLOW_OFFSET: i8 = 6;
/// Public so the layout tests can budget for the glow's bleed past the wrap edge.
pub const MARK_GLOW_BLUR: u8 = 18;
/// `.htext h1{font-size:26px;font-weight:700;line-height:1.1}`.
const TITLE_FONT: f32 = 26.0;
const TITLE_LINE: f32 = 1.1;
/// `.htext .tag{font-size:13.5px}`.
const TAG_FONT: f32 = 13.5;
/// `.htext .tag{margin-top:4px}`.
const TAG_TOP: f32 = 4.0;
/// `.ver{font:600 11px/1 var(--mono)}`.
const VER_FONT: f32 = FONT_CAPTION;
/// `.ver{padding:5px 9px}`.
const VER_PAD_X: f32 = 9.0;
const VER_PAD_Y: f32 = 5.0;

/// `.hero h1`.
const TITLE: &str = "Welcome to Thoth";
/// `.htext .tag` — two runs; the trailing sentence is the accented one
/// (`<span style="color:var(--mauve)">`).
const TAGLINE: &str = "A fast, keyboard-first viewer for JSON, NDJSON & data files. ";
const TAGLINE_ACCENT: &str = "Wisdom for your JSON.";

// ── Section label — design `.slabel` ─────────────────────────────────────────

/// `.slabel{font-size:10.5px;font-weight:700;text-transform:uppercase}`. egui has
/// no letter-spacing, so `letter-spacing:.09em` is dropped; the uppercasing and
/// the weight are honoured.
const SLABEL_FONT: f32 = 10.5;
/// `.slabel{margin-bottom:10px}`.
const SLABEL_BOTTOM: f32 = 10.0;
/// `.slabel .a{font-weight:600;font-size:11px}` — the right-aligned action link.
const SLABEL_ACTION_FONT: f32 = FONT_CAPTION;

// ── Start actions — design `.actions` / `.acard` ─────────────────────────────

/// `.actions{grid-template-columns:repeat(3,1fr);gap:10px}`.
const ACTIONS_GAP: f32 = 10.0;
/// `.actions{margin-bottom:12px}`.
const ACTIONS_BOTTOM: f32 = 12.0;
/// `.acard{padding:12px 13px}`.
const ACARD_PAD_X: f32 = 13.0;
const ACARD_PAD_Y: f32 = 12.0;
/// `.acard{gap:12px}`.
const ACARD_GAP: f32 = 12.0;
/// `.acard .tile{width:38px;height:38px}` — the tallest child, so it sets the
/// card's height together with the vertical padding.
const ACARD_TILE: f32 = 38.0;
/// `.acard .tile{font-size:20px}`.
const ACARD_TILE_GLYPH: f32 = 20.0;
/// `.acard .t{font-size:14px;font-weight:600}`.
const ACARD_TITLE_FONT: f32 = 14.0;
/// `.acard .s{font-size:11.5px}`.
const ACARD_SUB_FONT: f32 = 11.5;
/// `.acard .s{margin-top:2px}`.
const ACARD_SUB_TOP: f32 = 2.0;

// ── Key chip — design `.kbd` ─────────────────────────────────────────────────

/// `.kbd{font-family:var(--mono);font-size:11px}`.
const KBD_FONT: f32 = FONT_CAPTION;
/// `.kbd{padding:3px 7px}`.
const KBD_PAD_X: f32 = 7.0;
const KBD_PAD_Y: f32 = 3.0;

// ── Drop affordance — design `.drop` ─────────────────────────────────────────

/// `.drop{height:52px}`.
const DROP_H: f32 = 52.0;
/// `.drop{gap:10px}`.
const DROP_GAP: f32 = 10.0;
/// `.drop{margin-bottom:28px}`.
const DROP_BOTTOM: f32 = 28.0;
/// `.drop{font-size:12.5px}`.
const DROP_FONT: f32 = FONT_CONTROL;
/// `.drop i{font-size:18px}`.
const DROP_ICON: f32 = 18.0;
/// `.drop{box-shadow:inset 0 0 0 1.5px …}` — an *inset* ring, so it is stroked
/// inside the rect rather than centred on its edge.
const DROP_BORDER: f32 = 1.5;
/// `.drop .fmt{font-family:var(--mono);font-size:11px}`.
const DROP_FMT_FONT: f32 = FONT_CAPTION;

/// `.drop b`.
const DROP_LABEL: &str = "Drop a file to open";
/// `.drop .fmt`.
const DROP_FORMATS: &str = "JSON · NDJSON · CSV · or paste with ⌘V";

// ── Lower columns — design `.cols` ───────────────────────────────────────────

/// `.cols{gap:34px}`.
const COLS_GAP: f32 = 34.0;
/// `.cols{grid-template-columns:1.1fr 1fr}` — the left (Recent) share.
const COLS_LEFT_FR: f32 = 1.1;

// ── Recent rows — design `.rrow` ─────────────────────────────────────────────

/// `.recent{gap:2px}`.
const RECENT_GAP: f32 = 2.0;
/// `.rrow{padding:8px 10px}`.
const RROW_PAD_X: f32 = 10.0;
const RROW_PAD_Y: f32 = 8.0;
/// `.rrow{gap:11px}`.
const RROW_GAP: f32 = 11.0;
/// `.rrow .ic{width:30px;height:30px}` — sets the row height with the padding.
const RROW_TILE: f32 = 30.0;
/// `.rrow .ic{font-size:15px}`.
const RROW_TILE_GLYPH: f32 = ICON_CONTROL;
/// `.rrow .fn{font-size:13px;font-family:var(--mono)}`.
const RROW_NAME_FONT: f32 = 13.0;
/// `.rrow .meta{font-size:11px}`.
const RROW_META_FONT: f32 = FONT_CAPTION;
/// `.rrow .meta{margin-top:1px}`.
const RROW_META_TOP: f32 = 1.0;
/// `.rrow .ext{font:600 9.5px/1 var(--mono)}`.
const EXT_FONT: f32 = 9.5;
/// `.rrow .ext{padding:3px 6px}`.
const EXT_PAD_X: f32 = 6.0;
const EXT_PAD_Y: f32 = 3.0;
/// The sheet draws four `.rrow`s; five keeps the column in balance with the five
/// tips beside it. The sidebar's Recent Files panel remains the full history.
const RECENT_MAX: usize = 5;
/// `.meta`'s separator.
const META_SEP: &str = " · ";

// ── Tips — design `.tips` / `.tip` ───────────────────────────────────────────

/// `.tips{gap:9px}`.
const TIPS_GAP: f32 = 9.0;
/// `.tip{gap:11px}`.
const TIP_GAP: f32 = 11.0;
/// `.tip .bd{font-size:12.5px;line-height:1.45}`.
const TIP_FONT: f32 = FONT_CONTROL;
const TIP_LINE: f32 = 1.45;

/// The sheet's five `.tip` rows: `.kbd` chip, then body. The shortcut glyphs are
/// the sheet's own literals, matching the macOS bindings in `shortcut_handler`.
const TIPS: [(&str, &str); 5] = [
    (
        "Drag → edge",
        "Drop on the left, right, top or bottom of a pane to split it.",
    ),
    (
        "Right-click tab",
        "Pin, close others, split right / down — full menu.",
    ),
    (
        "⌘W",
        "Close the active tab. The last welcome tab closes the window.",
    ),
    ("⌘⌥ → / ←", "Cycle to the next or previous tab."),
    ("⌘1 – ⌘9", "Jump to a tab by position."),
];

// ── Recent-file metadata cache ───────────────────────────────────────────────

/// egui-memory key for the [`MetaCache`].
const META_CACHE_ID: &str = "welcome_recent_meta";
/// Seconds a cached stat is trusted for. `.meta`'s size and modified time need a
/// `fs::metadata` syscall, which must not run per row per frame; five seconds
/// keeps at most one stat per row per five seconds (five syscalls) while still
/// reflecting a file that changed under us.
const META_TTL: f64 = 5.0;

/// A recent file's `fs::metadata`, as far as `.meta` needs it.
#[derive(Clone, Copy)]
struct FileMeta {
    /// `.meta`'s size run.
    size: u64,
    /// `.meta`'s time run. The *string* is re-derived from this every frame, so a
    /// cached entry never shows a stale “2m ago”.
    modified: Option<SystemTime>,
}

/// One cache slot: when it was statted, and what came back (`None` = the stat
/// failed, e.g. the file is gone — then `.meta` shows only the free parts).
#[derive(Clone, Copy)]
struct MetaEntry {
    stat_at: f64,
    meta: Option<FileMeta>,
}

type MetaCache = HashMap<PathBuf, MetaEntry>;

/// One `.rrow`'s resolved content. Built once per frame, before layout, because
/// the composition is measured and then drawn (see [`WelcomePanel::render`]).
struct RecentEntry {
    /// What clicking the row opens.
    path: PathBuf,
    /// `.fn` — the file's own name.
    name: String,
    /// `.meta` — `~/work/thoth · 4.2 KB · 2m ago`. The directory is free; the
    /// size and time runs appear only when the stat succeeded, never faked.
    meta: String,
    /// `.ext` — the trailing chip, upper-cased.
    ext: Option<String>,
    /// `.ic`'s glyph and tint, by file type.
    glyph: &'static str,
    tint_role: TintRole,
}

/// Which palette role a `.rrow .ic` tint comes from — resolved against the live
/// palette at paint time, so the entry list stays theme-independent.
#[derive(Clone, Copy)]
enum TintRole {
    /// `t-mauve` — JSON.
    Accent,
    /// `t-blue` — NDJSON.
    Info,
    /// `t-peach` — CSV.
    Peach,
    /// Anything else: a muted glyph, no colour claim.
    Muted,
}

impl TintRole {
    fn color(self, c: &ThemeColors) -> Color32 {
        match self {
            Self::Accent => c.accent,
            Self::Info => c.info,
            Self::Peach => c.syntax_number,
            Self::Muted => c.fg_muted,
        }
    }
}

/// What the user did on the welcome screen this frame.
pub enum WelcomeEvent {
    /// `.acard[data-od-id=action-open]`, and the `.drop` affordance.
    OpenFilePicker,
    /// A `.rrow` was clicked.
    OpenRecentFile(PathBuf),
    /// `.acard[data-od-id=action-new-window]` (⌘N).
    NewWindow,
    /// `.acard[data-od-id=action-plugins]`.
    BrowsePlugins,
    /// The Recent `.slabel`'s `Clear` action.
    ClearRecentFiles,
}

/// One `.acard`'s content.
struct ActionCard<'a> {
    /// `.tile`'s glyph.
    glyph: &'a str,
    /// `.tile`'s tint (`t-mauve` / `t-blue` / `t-green`).
    tint: Color32,
    /// `.acard .t`.
    title: &'a str,
    /// `.acard .s`.
    subtitle: &'a str,
    /// The trailing `.kbd` chip.
    key: &'a str,
    /// The chip is present but dimmed — the sheet's `style="opacity:.35"`.
    dim_key: bool,
}

pub struct WelcomePanel;

impl WelcomePanel {
    pub fn render(
        ui: &mut egui::Ui,
        recent_files: &[String],
        colors: Option<ThemeColors>,
    ) -> Vec<WelcomeEvent> {
        let mut events = Vec::new();

        let c = colors.unwrap_or_else(|| {
            ui.ctx()
                .memory(|m| {
                    m.data
                        .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                })
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        // Resolved (and stat-cached) up front: the composition below is laid out
        // twice per frame — once to measure, once to draw — and neither pass
        // should touch the filesystem.
        let recent = recent_entries(ui.ctx(), recent_files);

        // `.welcome{overflow:auto;display:flex;flex-direction:column;
        // justify-content:center}` — the panel scrolls, and while the content is
        // shorter than the viewport it sits centred in it.
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let viewport_h = ui.available_height();
                let avail_w = ui.available_width();

                // `.wrap{width:100%;max-width:980px;margin:0 auto;padding:40px 44px}`
                let wrap_w = avail_w.min(WRAP_MAX_W);
                let indent = ((avail_w - wrap_w) / 2.0).max(0.0) + WRAP_PAD_X;
                let content_w = (wrap_w - WRAP_PAD_X * 2.0).max(0.0);

                // `justify-content:center` needs the block's height *before* it is
                // placed. An invisible sizing pass measures it at the final width
                // within this same frame (a height remembered from the last frame
                // would lag, and the hero would visibly drift on resize). Its
                // events are thrown away, and its widgets report no hover because
                // the pass is disabled.
                //
                // `new_child`, not `scope_builder`: the latter allocates the child's
                // min_rect in the parent, which would reserve the whole block's
                // height as empty space and push the real content off the bottom.
                let measured = {
                    let mut probe = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("welcome-sizing")
                            .sizing_pass()
                            .invisible()
                            .max_rect(Rect::from_min_size(
                                ui.cursor().min,
                                vec2(content_w, viewport_h.max(1.0)),
                            )),
                    );
                    wrap(&mut probe, &recent, &c, &mut Vec::new());
                    probe.min_rect().height()
                };
                ui.add_space(((viewport_h - measured) / 2.0).max(0.0));

                // `ui.horizontal_top` (not `with_layout`, whose centred cross-align
                // would claim the full remaining height) indents the wrap; the
                // column inside it is pinned to the wrap's content width.
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.add_space(indent);
                    ui.vertical(|ui| {
                        ui.set_width(content_w);
                        wrap(ui, &recent, &c, &mut events);
                    });
                });
            });

        events
    }
}

// ── Composition ───────────────────────────────────────────────────────────────

/// The `.wrap` block: every gap between its children is explicit, so the ui's own
/// item spacing is zeroed first.
fn wrap(
    ui: &mut egui::Ui,
    recent: &[RecentEntry],
    c: &ThemeColors,
    events: &mut Vec<WelcomeEvent>,
) {
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let w = ui.available_width();

    ui.add_space(WRAP_PAD_Y);

    hero(ui, w, c);
    ui.add_space(HERO_BOTTOM);

    slabel(ui, w, "Start", None, c);
    start_actions(ui, w, c, events);
    ui.add_space(ACTIONS_BOTTOM);

    if drop_zone(ui, w, c) {
        // The sheet's drop affordance *teaches* the gesture; clicking it does what
        // Open file… does. Real drag-and-drop lives in `drag_and_drop.rs`.
        events.push(WelcomeEvent::OpenFilePicker);
    }
    ui.add_space(DROP_BOTTOM);

    lower_columns(ui, w, recent, c, events);
    ui.add_space(WRAP_PAD_Y);
}

/// `.hero`: the 52px `.mark`, the title + tagline block (both vertically centred
/// — `align-items:center`), and the `.ver` pill pushed right and pinned to the
/// top (`margin-left:auto;align-self:flex-start`).
fn hero(ui: &mut egui::Ui, w: f32, c: &ThemeColors) {
    // The pill is measured first: it keeps its natural width, and the text block
    // gets whatever is left — the flex row's `margin-left:auto` in reverse.
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    // No 600-weight mono face is registered (only the proportional 500/600 ones),
    // so `.ver`'s `font:600 … var(--mono)` renders at the mono regular weight.
    let ver = line(ui, &version, FontId::monospace(VER_FONT), c.fg_subtle());
    let ver_size = ver.size() + vec2(VER_PAD_X * 2.0, VER_PAD_Y * 2.0);

    let text_w = (w - MARK - HERO_GAP - HERO_GAP - ver_size.x).max(0.0);
    // `font-weight:700` has no registered face; semibold (600) is the heaviest
    // real one, and beats smearing a galley over itself.
    let title = line_h(
        ui,
        TITLE,
        semibold_font_id(ui.ctx(), TITLE_FONT),
        c.fg,
        TITLE_FONT * TITLE_LINE,
    );
    let tag = tagline(ui, text_w, c);

    let text_h = title.size().y + TAG_TOP + tag.size().y;
    let (rect, _) = ui.allocate_exact_size(vec2(w, MARK.max(text_h)), Sense::hover());
    let p = ui.painter();

    // `.mark` — gradient tile, mauve glow, 28px glyph in `--crust`.
    let mark = Rect::from_min_size(
        pos2(rect.left(), rect.center().y - MARK / 2.0),
        Vec2::splat(MARK),
    );
    p.add(
        egui::Shadow {
            offset: [0, MARK_GLOW_OFFSET],
            blur: MARK_GLOW_BLUR,
            spread: 0,
            color: with_alpha(c.accent, A_34),
        }
        .as_shape(mark, RADIUS_PANEL),
    );
    gradient_tile(p, mark, RADIUS_PANEL, c.accent, c.accent_secondary);
    p.text(
        mark.center(),
        Align2::CENTER_CENTER,
        ph::TREE_STRUCTURE,
        phosphor_font_id(MARK_GLYPH),
        c.bg_sunken,
    );

    let tx = mark.right() + HERO_GAP;
    let ty = rect.center().y - text_h / 2.0;
    p.galley(pos2(tx, ty), title.clone(), c.fg);
    p.galley(pos2(tx, ty + title.size().y + TAG_TOP), tag, c.fg_muted);

    // `.ver` — mono pill on `--surface0` with the shared hairline edge.
    let ver_rect = Rect::from_min_size(pos2(rect.right() - ver_size.x, rect.top()), ver_size);
    p.rect_filled(ver_rect, RADIUS_PILL, c.surface);
    p.rect_stroke(ver_rect, RADIUS_PILL, edge_stroke(c), StrokeKind::Inside);
    p.galley(
        ver_rect.min + vec2(VER_PAD_X, VER_PAD_Y),
        ver,
        c.fg_subtle(),
    );
}

/// `.htext .tag` — one wrapped paragraph in two runs: muted body, then the
/// accented closing sentence.
fn tagline(ui: &egui::Ui, max_w: f32, c: &ThemeColors) -> Arc<Galley> {
    let font = FontId::proportional(TAG_FONT);
    let mut job = LayoutJob {
        wrap: TextWrapping::wrap_at_width(max_w),
        ..Default::default()
    };
    job.append(
        TAGLINE,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color: c.fg_muted,
            ..Default::default()
        },
    );
    job.append(
        TAGLINE_ACCENT,
        0.0,
        TextFormat {
            font_id: font,
            color: c.accent,
            ..Default::default()
        },
    );
    ui.painter().layout_job(job)
}

/// `.slabel`: a bold upper-case label with an optional right-aligned action link
/// (`.slabel .a`). Returns whether that link was clicked.
fn slabel(ui: &mut egui::Ui, w: f32, text: &str, action: Option<&str>, c: &ThemeColors) -> bool {
    let label = line(
        ui,
        &text.to_uppercase(),
        semibold_font_id(ui.ctx(), SLABEL_FONT),
        c.fg_muted,
    );
    let act = action.map(|a| {
        line(
            ui,
            a,
            semibold_font_id(ui.ctx(), SLABEL_ACTION_FONT),
            c.fg_faint(),
        )
    });
    let h = label.size().y.max(act.as_ref().map_or(0.0, |g| g.size().y));
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());

    let mut clicked = false;
    if let Some(act) = act {
        // `.slabel .a{cursor:pointer}` — its own hit target, on top of the row.
        let act_rect = Rect::from_min_size(
            pos2(
                rect.right() - act.size().x,
                rect.center().y - act.size().y / 2.0,
            ),
            act.size(),
        );
        let resp = ui.interact(
            act_rect,
            ui.id().with(("slabel-action", text)),
            Sense::click(),
        );
        if resp.hovered() && !ui.is_sizing_pass() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        // `.slabel .a:hover{color:var(--text)}`
        let color = if resp.hovered() { c.fg } else { c.fg_faint() };
        ui.painter().galley(act_rect.min, act, color);
        clicked = resp.clicked();
    }
    ui.painter().galley(
        pos2(rect.left(), rect.center().y - label.size().y / 2.0),
        label,
        c.fg_muted,
    );

    ui.add_space(SLABEL_BOTTOM);
    clicked
}

/// `.actions` — three equal `.acard` columns with a 10px gutter.
fn start_actions(ui: &mut egui::Ui, w: f32, c: &ThemeColors, events: &mut Vec<WelcomeEvent>) {
    let card_w = ((w - ACTIONS_GAP * 2.0) / 3.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        if acard(
            ui,
            card_w,
            &ActionCard {
                glyph: ph::FOLDER_OPEN,
                tint: c.accent,
                title: "Open file…",
                subtitle: "JSON · NDJSON · CSV",
                key: "⌘O",
                dim_key: false,
            },
            c,
        ) {
            events.push(WelcomeEvent::OpenFilePicker);
        }
        ui.add_space(ACTIONS_GAP);

        if acard(
            ui,
            card_w,
            &ActionCard {
                glyph: ph::APP_WINDOW,
                tint: c.info,
                title: "New window",
                subtitle: "Fresh workspace",
                key: "⌘N",
                dim_key: false,
            },
            c,
        ) {
            events.push(WelcomeEvent::NewWindow);
        }
        ui.add_space(ACTIONS_GAP);

        if acard(
            ui,
            card_w,
            &ActionCard {
                glyph: ph::PUZZLE_PIECE,
                tint: c.success,
                title: "Browse plugins…",
                subtitle: "Databases · URLs · themes",
                key: "↵",
                dim_key: true,
            },
            c,
        ) {
            events.push(WelcomeEvent::BrowsePlugins);
        }
    });
}

/// One `.acard`: a hairline-edged base panel that washes to `--surface0` on
/// hover, holding a tinted tile, a two-line label block, and a trailing chip.
///
/// Allocated as a single click target with its contents painted in, so nothing
/// can shadow the card's hover or swallow its click.
fn acard(ui: &mut egui::Ui, w: f32, card: &ActionCard<'_>, c: &ThemeColors) -> bool {
    let h = ACARD_PAD_Y * 2.0 + ACARD_TILE;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let hovered = resp.hovered();
    if hovered && !ui.is_sizing_pass() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // `.acard{background:var(--base);box-shadow:var(--edge)}`
    // `.acard:hover{background:var(--surface0)}`
    let p = ui.painter();
    p.rect_filled(rect, RADIUS_PANEL, if hovered { c.surface } else { c.bg });
    p.rect_stroke(rect, RADIUS_PANEL, edge_stroke(c), StrokeKind::Inside);

    // `.acard .tile` — 38px tinted square, `RADIUS_CONTROL` for the sheet's 11px.
    let tile = Rect::from_min_size(
        pos2(
            rect.left() + ACARD_PAD_X,
            rect.center().y - ACARD_TILE / 2.0,
        ),
        Vec2::splat(ACARD_TILE),
    );
    tint_tile(
        ui,
        tile,
        RADIUS_CONTROL,
        card.glyph,
        ACARD_TILE_GLYPH,
        card.tint,
    );

    // `.kbd{margin-left:auto…}` — measured and placed first, so the label block
    // takes what is left, exactly as the flex row does.
    let chip = kbd_size(ui, card.key);
    let chip_pos = pos2(
        rect.right() - ACARD_PAD_X - chip.x,
        rect.center().y - chip.y / 2.0,
    );
    kbd_chip(ui, chip_pos, card.key, card.dim_key, c);

    // `.acard .l{flex:1;min-width:0}` with `.s{text-overflow:ellipsis}`.
    let lx = tile.right() + ACARD_GAP;
    let lw = (chip_pos.x - ACARD_GAP - lx).max(0.0);
    let title = line_clipped(
        ui,
        card.title,
        semibold_font_id(ui.ctx(), ACARD_TITLE_FONT),
        c.fg,
        lw,
    );
    let sub = line_clipped(
        ui,
        card.subtitle,
        FontId::proportional(ACARD_SUB_FONT),
        c.fg_muted,
        lw,
    );
    let block_h = title.size().y + ACARD_SUB_TOP + sub.size().y;
    let ly = rect.center().y - block_h / 2.0;
    let p = ui.painter();
    p.galley(pos2(lx, ly), title.clone(), c.fg);
    p.galley(
        pos2(lx, ly + title.size().y + ACARD_SUB_TOP),
        sub,
        c.fg_muted,
    );

    resp.clicked()
}

/// `.drop`: a 52px dashed-feeling well — `surface0@30%` behind a 1.5px inset
/// `surface1@55%` ring — whose text, ring and fill all go mauve on hover.
/// Returns whether it was clicked.
fn drop_zone(ui: &mut egui::Ui, w: f32, c: &ThemeColors) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, DROP_H), Sense::click());
    let hovered = resp.hovered();
    if hovered && !ui.is_sizing_pass() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let (fill, ring, fg) = if hovered {
        (
            with_alpha(c.accent, A_08),
            with_alpha(c.accent, A_55),
            c.accent,
        )
    } else {
        (
            with_alpha(c.surface, A_30),
            with_alpha(c.surface_raised, A_55),
            c.fg_muted,
        )
    };

    // `.drop b{color:var(--subtext0)}` loses to `.drop:hover{color:var(--mauve)}`
    // on specificity, so the bold run follows the hover; `.drop .fmt` ties and
    // comes later, so the format hint keeps `--overlay0` throughout.
    let icon = line(ui, ph::DOWNLOAD_SIMPLE, phosphor_font_id(DROP_ICON), fg);
    let label = line(
        ui,
        DROP_LABEL,
        semibold_font_id(ui.ctx(), DROP_FONT),
        if hovered { c.accent } else { c.fg_subtle() },
    );
    let fmt = line(
        ui,
        DROP_FORMATS,
        FontId::monospace(DROP_FMT_FONT),
        c.fg_faint(),
    );

    let p = ui.painter();
    p.rect_filled(rect, RADIUS_PANEL, fill);
    p.rect_stroke(
        rect,
        RADIUS_PANEL,
        Stroke::new(DROP_BORDER, ring),
        StrokeKind::Inside,
    );

    // `justify-content:center` with a 10px gap between the three runs.
    let total = icon.size().x + DROP_GAP + label.size().x + DROP_GAP + fmt.size().x;
    let mut x = rect.center().x - total / 2.0;
    for (galley, color) in [
        (icon, fg),
        (label, if hovered { c.accent } else { c.fg_subtle() }),
        (fmt, c.fg_faint()),
    ] {
        let size = galley.size();
        p.galley(pos2(x, rect.center().y - size.y / 2.0), galley, color);
        x += size.x + DROP_GAP;
    }

    resp.clicked()
}

/// `.cols{grid-template-columns:1.1fr 1fr;gap:34px}` — Recent beside Tips, both
/// starting at the same y.
fn lower_columns(
    ui: &mut egui::Ui,
    w: f32,
    recent: &[RecentEntry],
    c: &ThemeColors,
    events: &mut Vec<WelcomeEvent>,
) {
    let inner = (w - COLS_GAP).max(0.0);
    let left_w = inner * COLS_LEFT_FR / (COLS_LEFT_FR + 1.0);
    let right_w = inner - left_w;

    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        ui.vertical(|ui| {
            ui.set_width(left_w);
            recent_column(ui, left_w, recent, c, events);
        });
        ui.add_space(COLS_GAP);
        ui.vertical(|ui| {
            ui.set_width(right_w);
            tips_column(ui, right_w, c);
        });
    });
}

/// The Recent column — `.slabel` with its `Clear` action over the `.recent` list.
fn recent_column(
    ui: &mut egui::Ui,
    w: f32,
    recent: &[RecentEntry],
    c: &ThemeColors,
    events: &mut Vec<WelcomeEvent>,
) {
    let clear = (!recent.is_empty()).then_some("Clear");
    if slabel(ui, w, "Recent", clear, c) {
        events.push(WelcomeEvent::ClearRecentFiles);
    }

    if recent.is_empty() {
        ui.add(
            Typography::builder()
                .text("No recent files")
                .variant(TypographyVariant::BodyMuted)
                .build(),
        );
        return;
    }

    for (i, entry) in recent.iter().enumerate() {
        if i > 0 {
            ui.add_space(RECENT_GAP);
        }
        if rrow(ui, w, entry, c) {
            events.push(WelcomeEvent::OpenRecentFile(entry.path.clone()));
        }
    }
}

/// One `.rrow`: a tinted 30px type tile, the file name in mono, a metadata line,
/// and the `.ext` chip — one click target with its contents painted in.
fn rrow(ui: &mut egui::Ui, w: f32, entry: &RecentEntry, c: &ThemeColors) -> bool {
    let h = RROW_PAD_Y * 2.0 + RROW_TILE;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let hovered = resp.hovered();
    if hovered && !ui.is_sizing_pass() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if hovered {
        // `.rrow:hover{background:color-mix(in oklab,var(--text) 5%,transparent)}`
        ui.painter()
            .rect_filled(rect, RADIUS_CONTROL, with_alpha(c.fg, A_05));
    }

    // `.rrow .ic` — the sheet's 9px corner sits on the control rung.
    let tile = Rect::from_min_size(
        pos2(rect.left() + RROW_PAD_X, rect.center().y - RROW_TILE / 2.0),
        Vec2::splat(RROW_TILE),
    );
    tint_tile(
        ui,
        tile,
        RADIUS_CONTROL,
        entry.glyph,
        RROW_TILE_GLYPH,
        entry.tint_role.color(c),
    );

    // `.rrow .ext{flex:0 0 auto}` — placed from the right edge first.
    let mut text_right = rect.right() - RROW_PAD_X;
    if let Some(ext) = &entry.ext {
        let ext_galley = line(ui, ext, FontId::monospace(EXT_FONT), c.fg_muted);
        let size = ext_galley.size() + vec2(EXT_PAD_X * 2.0, EXT_PAD_Y * 2.0);
        let ext_rect = Rect::from_min_size(
            pos2(text_right - size.x, rect.center().y - size.y / 2.0),
            size,
        );
        let p = ui.painter();
        p.rect_filled(ext_rect, RADIUS_CHECK, c.surface);
        p.galley(
            ext_rect.min + vec2(EXT_PAD_X, EXT_PAD_Y),
            ext_galley,
            c.fg_muted,
        );
        text_right = ext_rect.left() - RROW_GAP;
    }

    // `.rrow .m{flex:1;min-width:0}`: the mono name ellipsises, the meta line
    // below it does not (the sheet only clips `.fn`).
    let tx = tile.right() + RROW_GAP;
    let tw = (text_right - tx).max(0.0);
    let name = line_clipped(ui, &entry.name, FontId::monospace(RROW_NAME_FONT), c.fg, tw);
    let meta = line_clipped(
        ui,
        &entry.meta,
        FontId::proportional(RROW_META_FONT),
        c.fg_muted,
        tw,
    );
    let block_h = name.size().y + RROW_META_TOP + meta.size().y;
    let ty = rect.center().y - block_h / 2.0;
    let p = ui.painter();
    p.galley(pos2(tx, ty), name.clone(), c.fg);
    p.galley(
        pos2(tx, ty + name.size().y + RROW_META_TOP),
        meta,
        c.fg_muted,
    );

    resp.clicked()
}

/// The Tips column — `.slabel` over five `.tip` rows.
fn tips_column(ui: &mut egui::Ui, w: f32, c: &ThemeColors) {
    slabel(ui, w, "Tips · keyboard", None, c);
    for (i, (keys, body)) in TIPS.iter().enumerate() {
        if i > 0 {
            ui.add_space(TIPS_GAP);
        }
        tip(ui, w, keys, body, c);
    }
}

/// One `.tip`: a `.kbd` chip and a wrapped body, both aligned to the row's top
/// (`align-items:flex-start`).
fn tip(ui: &mut egui::Ui, w: f32, keys: &str, body: &str, c: &ThemeColors) {
    let chip = kbd_size(ui, keys);
    let body_w = (w - chip.x - TIP_GAP).max(0.0);
    // `line-height:1.45` — `Typography` has no line-height control, so the body
    // is laid out here with the design's leading.
    let mut job = LayoutJob {
        wrap: TextWrapping::wrap_at_width(body_w),
        ..Default::default()
    };
    job.append(
        body,
        0.0,
        TextFormat {
            font_id: FontId::proportional(TIP_FONT),
            color: c.fg_subtle(),
            line_height: Some(TIP_FONT * TIP_LINE),
            ..Default::default()
        },
    );
    let text = ui.painter().layout_job(job);

    let (rect, _) = ui.allocate_exact_size(vec2(w, chip.y.max(text.size().y)), Sense::hover());
    kbd_chip(ui, rect.min, keys, false, c);
    ui.painter().galley(
        pos2(rect.left() + chip.x + TIP_GAP, rect.top()),
        text,
        c.fg_subtle(),
    );
}

// ── Painted primitives ────────────────────────────────────────────────────────

/// A tinted glyph tile — design `.acard .tile` / `.rrow .ic`, whose `t-*` classes
/// are the same colour at 16% behind the glyph at full strength.
fn tint_tile(ui: &egui::Ui, rect: Rect, radius: f32, glyph: &str, glyph_size: f32, tint: Color32) {
    let p = ui.painter();
    p.rect_filled(rect, radius, with_alpha(tint, A_16));
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        phosphor_font_id(glyph_size),
        tint,
    );
}

/// The size a [`kbd_chip`] will occupy — design `.kbd{padding:3px 7px}`.
fn kbd_size(ui: &egui::Ui, text: &str) -> Vec2 {
    let galley = line(ui, text, FontId::monospace(KBD_FONT), Color32::PLACEHOLDER);
    galley.size() + vec2(KBD_PAD_X * 2.0, KBD_PAD_Y * 2.0)
}

/// A key chip — design `.kbd`: mono 11px `--subtext0` on `--surface0` with the
/// shared hairline edge, at the chip rung (the sheet's 6px). `dim` reproduces the
/// plugins card's `opacity:.35`.
fn kbd_chip(ui: &egui::Ui, top_left: Pos2, text: &str, dim: bool, c: &ThemeColors) {
    let fade = |color: Color32| {
        if dim {
            with_alpha(color, A_35_OPACITY)
        } else {
            color
        }
    };
    let galley = line(ui, text, FontId::monospace(KBD_FONT), fade(c.fg_subtle()));
    let rect = Rect::from_min_size(
        top_left,
        galley.size() + vec2(KBD_PAD_X * 2.0, KBD_PAD_Y * 2.0),
    );
    let p = ui.painter();
    p.rect_filled(rect, RADIUS_CHIP, fade(c.surface));
    let edge = edge_stroke(c);
    p.rect_stroke(
        rect,
        RADIUS_CHIP,
        Stroke::new(edge.width, fade(edge.color)),
        StrokeKind::Inside,
    );
    p.galley(
        rect.min + vec2(KBD_PAD_X, KBD_PAD_Y),
        galley,
        fade(c.fg_subtle()),
    );
}

/// The `.mark` tile's `linear-gradient(150deg,var(--mauve),var(--lavender))`.
///
/// egui has no gradient shape, so the rounded rect's outline is tessellated and
/// fanned into a mesh whose vertex colours are interpolated along the gradient
/// axis (CSS `150deg` points 150° clockwise from “up”: mostly down, a little
/// right). Mesh edges carry no feathering, so a hairline in the gradient's
/// midpoint colour is stroked over the outline to keep the corners smooth.
fn gradient_tile(p: &egui::Painter, rect: Rect, radius: f32, from: Color32, to: Color32) {
    const ANGLE_DEG: f32 = 150.0;
    let (sin, cos) = ANGLE_DEG.to_radians().sin_cos();
    let dir = vec2(sin, -cos);
    // How far the box extends along that axis, so `t` spans exactly 0…1 over it.
    let span = ((rect.width() * dir.x).abs() + (rect.height() * dir.y).abs()).max(1.0);

    let mut outline = Vec::new();
    egui::epaint::tessellator::path::rounded_rectangle(
        &mut outline,
        rect,
        egui::epaint::CornerRadiusF32::same(radius),
    );

    let center = rect.center();
    let color_at = |pos: Pos2| lerp_color(from, to, 0.5 + (pos - center).dot(dir) / span);
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color_at(center));
    for pos in &outline {
        mesh.colored_vertex(*pos, color_at(*pos));
    }
    let n = outline.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(egui::Shape::mesh(mesh));
    p.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, lerp_color(from, to, 0.5)),
        StrokeKind::Inside,
    );
}

/// Channel-wise mix of two palette colours. The `.mark` gradient is the one place
/// the design needs a colour that is not itself a token, and this keeps it a mix
/// of `accent` and `accent_secondary` rather than a literal.
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(ch(a.r(), b.r()), ch(a.g(), b.g()), ch(a.b(), b.b()))
}

// ── Text helpers ──────────────────────────────────────────────────────────────

/// One unwrapped line.
fn line(ui: &egui::Ui, text: &str, font: FontId, color: Color32) -> Arc<Galley> {
    ui.painter().layout_no_wrap(text.to_owned(), font, color)
}

/// One unwrapped line at an explicit `line-height`.
fn line_h(ui: &egui::Ui, text: &str, font: FontId, color: Color32, height: f32) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font,
            color,
            line_height: Some(height),
            ..Default::default()
        },
    );
    ui.painter().layout_job(job)
}

/// One line, ellipsised at `max_w` — the design's
/// `white-space:nowrap;overflow:hidden;text-overflow:ellipsis`. `Typography` has
/// no truncating mode, so this is the same primitive the SDK's `List` rows use.
fn line_clipped(
    ui: &egui::Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_w: f32,
) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job.wrap = TextWrapping::truncate_at_width(max_w);
    ui.painter().layout_job(job)
}

// ── Recent-file resolution ────────────────────────────────────────────────────

/// Resolve the recent list into `.rrow` content, statting each file at most once
/// per [`META_TTL`] (see [`MetaEntry`]). The whole cache is read and written once
/// per frame rather than per row, so a full pass costs one memory clone.
fn recent_entries(ctx: &egui::Context, paths: &[String]) -> Vec<RecentEntry> {
    let now = ctx.input(|i| i.time);
    let id = egui::Id::new(META_CACHE_ID);
    let mut cache: MetaCache = ctx.memory(|m| m.data.get_temp(id)).unwrap_or_default();
    let mut dirty = false;

    let entries = paths
        .iter()
        .take(RECENT_MAX)
        .map(|raw| {
            let path = PathBuf::from(raw);

            let meta = match cache.get(&path) {
                Some(hit) if now - hit.stat_at < META_TTL => hit.meta,
                _ => {
                    let meta = std::fs::metadata(&path).ok().map(|m| FileMeta {
                        size: m.len(),
                        modified: m.modified().ok(),
                    });
                    cache.insert(path.clone(), MetaEntry { stat_at: now, meta });
                    dirty = true;
                    meta
                }
            };

            // `~/work/thoth · 4.2 KB · 2m ago` — the directory is free, the other
            // two runs come from the stat and are simply absent without it.
            let mut runs = Vec::new();
            if let Some(dir) = path.parent().map(home_relative).filter(|d| !d.is_empty()) {
                runs.push(dir);
            }
            if let Some(m) = meta {
                runs.push(human_size(m.size));
                if let Some(t) = m.modified {
                    runs.push(relative_time(t));
                }
            }

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_uppercase);
            let (glyph, tint_role) = file_type_icon(ext.as_deref());

            RecentEntry {
                name: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(raw.as_str())
                    .to_owned(),
                meta: runs.join(META_SEP),
                ext,
                glyph,
                tint_role,
                path,
            }
        })
        .collect();

    if dirty {
        // Drop cache slots for files no longer on the list, so the map cannot grow
        // without bound over a long session.
        cache.retain(|k, _| paths.iter().take(RECENT_MAX).any(|p| Path::new(p) == k));
        ctx.memory_mut(|m| m.data.insert_temp(id, cache));
    }

    entries
}

/// `.rrow .ic`'s glyph and tint by file type — design's braces/mauve for JSON,
/// dashed list/blue for NDJSON, and the CSV sheet in peach.
fn file_type_icon(ext: Option<&str>) -> (&'static str, TintRole) {
    match ext.unwrap_or_default() {
        "JSON" | "JSON5" | "JSONC" => (ph::BRACKETS_CURLY, TintRole::Accent),
        "NDJSON" | "JSONL" => (ph::LIST_DASHES, TintRole::Info),
        "CSV" | "TSV" => (ph::FILE_CSV, TintRole::Peach),
        _ => (ph::FILE_TEXT, TintRole::Muted),
    }
}

/// `~/work/thoth` — a path under the user's home rendered home-relative, as the
/// design's `.meta` does. Anything outside home is left as-is.
fn home_relative(dir: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = dir.strip_prefix(&home)
    {
        if rest.as_os_str().is_empty() {
            return "~".to_owned();
        }
        return format!("~/{}", rest.display());
    }
    dir.display().to_string()
}

/// `4.2 KB` / `18 MB` — the design's `.meta` size run: decimal units, one
/// decimal below ten of the unit and none above.
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    const STEP: f64 = 1000.0;

    if bytes < STEP as u64 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / STEP;
    let mut unit = 0;
    while value >= STEP && unit + 1 < UNITS.len() {
        value /= STEP;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

/// `2m ago` / `yesterday` / `Mon` / `Aug 1` — the design's `.meta` time run,
/// derived fresh from the cached mtime on every frame.
fn relative_time(modified: SystemTime) -> String {
    let when: chrono::DateTime<chrono::Local> = modified.into();
    let now = chrono::Local::now();
    let elapsed = now.signed_duration_since(when);

    if elapsed.num_minutes() < 1 {
        // Also covers a clock-skewed future mtime, which must never read as a
        // negative age.
        "just now".to_owned()
    } else if elapsed.num_minutes() < 60 {
        format!("{}m ago", elapsed.num_minutes())
    } else if when.date_naive() == now.date_naive() {
        format!("{}h ago", elapsed.num_hours().max(1))
    } else if now.date_naive().pred_opt() == Some(when.date_naive()) {
        "yesterday".to_owned()
    } else if elapsed.num_days() < 7 {
        when.format("%a").to_string()
    } else {
        when.format("%b %-d").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{human_size, relative_time};
    use std::time::{Duration, SystemTime};

    #[test]
    fn size_matches_the_design_samples() {
        assert_eq!(human_size(4_200), "4.2 KB");
        assert_eq!(human_size(18_000_000), "18 MB");
        assert_eq!(human_size(512_000), "512 KB");
        assert_eq!(human_size(2_100), "2.1 KB");
        assert_eq!(human_size(812), "812 B");
    }

    #[test]
    fn relative_time_ladder() {
        let now = SystemTime::now();
        assert_eq!(relative_time(now), "just now");
        assert_eq!(
            relative_time(now - Duration::from_secs(2 * 60)),
            "2m ago",
            "minutes are the design's `2m ago`"
        );
        // A week back is never a relative phrase — it is a calendar date.
        let old = relative_time(now - Duration::from_secs(30 * 24 * 3600));
        assert!(
            !old.ends_with("ago") && old != "yesterday",
            "old files show a date, got {old}"
        );
    }
}
