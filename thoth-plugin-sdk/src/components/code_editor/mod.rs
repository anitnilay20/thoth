use bon::Builder;
use serde::{Deserialize, Serialize};

/// A user-supplied syntax definition for the [`CodeEditor`], for languages the
/// editor doesn't recognise built-in. Set it via
/// [`CodeEditor::custom_syntax`]; it takes precedence over the built-in
/// [`syntax`](CodeEditor::syntax) name.
///
/// ```
/// use thoth_plugin_sdk::components::CustomSyntax;
///
/// let toml = CustomSyntax::builder()
///     .language("toml")
///     .comment("#")
///     .keywords(vec!["true".to_string(), "false".to_string()])
///     .build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct CustomSyntax {
    /// Display name of the language (e.g. `"toml"`).
    pub language: String,
    /// Whether keyword/type/special matching is case-sensitive.
    #[builder(default)]
    #[serde(default)]
    pub case_sensitive: bool,
    /// Single-line comment prefix (e.g. `"#"` or `"//"`).
    #[serde(default)]
    pub comment: Option<String>,
    /// Multi-line comment delimiters as `(start, end)` (e.g. `("/*", "*/")`).
    #[serde(default)]
    pub comment_multiline: Option<(String, String)>,
    /// Words highlighted as keywords.
    #[builder(default)]
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Words highlighted as type names.
    #[builder(default)]
    #[serde(default)]
    pub types: Vec<String>,
    /// Words highlighted as special / built-in identifiers.
    #[builder(default)]
    #[serde(default)]
    pub special: Vec<String>,
}

/// An editable, syntax-highlighted code editor. Owns its `value`;
/// [`CodeEditor::show`] edits it in place.
///
/// ```
/// use thoth_plugin_sdk::components::CodeEditor;
///
/// let mut ed = CodeEditor::builder().value("{}").syntax("json").build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct CodeEditor {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// The editor's text content.
    #[builder(default)]
    #[serde(default)]
    pub value: String,
    /// Font size in points; defaults to 13.
    #[serde(default)]
    pub font_size: Option<f32>,
    /// Optional syntax language (e.g. `"rust"`, `"sql"`); defaults to plain.
    #[serde(default)]
    pub syntax: Option<String>,
    /// Minimum number of visible text rows. Defaults to the editor's own
    /// default when unset.
    #[serde(default)]
    pub rows: Option<usize>,
    /// Disable editing (renders read-only / dimmed).
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
    /// Draw a themed border around the whole editor. Defaults to `true`.
    #[builder(default = true)]
    #[serde(default = "default_true")]
    pub bordered: bool,

    /// Optional custom syntax definition, for languages not built into the editor. If set, this overrides the built-in syntax for the given
    /// [`CodeEditor::syntax`] language name.
    #[serde(default)]
    pub custom_syntax: Option<CustomSyntax>,

    /// Character offsets at which to draw a clickable ▶ run-marker in the left
    /// gutter (e.g. the start of each SQL statement). A click reports the
    /// offset via [`CodeEditorOutput::run_marker`]. Empty = no gutter.
    #[builder(default)]
    #[serde(default)]
    pub run_markers: Vec<usize>,
}

fn default_true() -> bool {
    true
}

/// A run request raised from the editor via a keyboard shortcut.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunRequest {
    /// `true` for the "run everything" shortcut (⌘/Ctrl+Shift+Enter); `false`
    /// for "run the statement at the caret" (⌘/Ctrl+Enter).
    pub all: bool,
    /// Caret position as a character offset into the text.
    pub caret: usize,
    /// Selected character range `(start, end)` when the user has a selection —
    /// callers run exactly this instead of the statement at the caret.
    pub selection: Option<(usize, usize)>,
}

/// Outcome of rendering a [`CodeEditor`] for one frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct CodeEditorOutput {
    /// The text changed this frame.
    pub changed: bool,
    /// Set when the user pressed a run shortcut (⌘/Ctrl+Enter, or +Shift for all).
    pub run: Option<RunRequest>,
    /// Character offset of a clicked run-marker (▶ gutter button), if any.
    pub run_marker: Option<usize>,
    /// Set when the user pressed a format shortcut (Option/Alt+Shift+F).
    pub format: Option<()>,
}

/// Intern a runtime string into a `&'static str`, deduping so repeated calls
/// with the same text don't leak. Needed because `egui_code_editor::Syntax`
/// only accepts `&'static str`, yet custom-syntax strings arrive owned.
#[cfg(feature = "egui")]
fn intern(s: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&existing) = set.get(s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    set.insert(leaked);
    leaked
}

#[cfg(feature = "egui")]
impl CodeEditor {
    /// A hash of the syntax word lists that seed the autocomplete trie, so the
    /// cached completer can be rebuilt when they change (e.g. a plugin streams
    /// table names into a custom syntax's `special` list after the first frame).
    fn words_fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if let Some(custom) = &self.custom_syntax {
            custom.case_sensitive.hash(&mut h);
            custom.keywords.hash(&mut h);
            custom.types.hash(&mut h);
            custom.special.hash(&mut h);
        } else {
            // Built-in syntaxes have fixed word lists; the language name fully
            // determines them.
            self.syntax.hash(&mut h);
        }
        h.finish()
    }

    /// Render the editor, editing [`value`](CodeEditor::value) in place.
    /// Returns what happened this frame (text change and/or submit shortcut).
    pub fn show(&mut self, ui: &mut egui::Ui) -> CodeEditorOutput {
        use crate::theme::ThemeColors;
        use egui_code_editor::{CodeEditor as Editor, Completer, Syntax};
        use std::collections::BTreeSet;
        let colors = ThemeColors::from_ctx(ui.ctx());

        let syntax = if let Some(custom) = &self.custom_syntax {
            // `egui_code_editor::Syntax` only accepts `&'static str`, but a custom
            // syntax arrives as owned `String`s (from plugin JSON), so intern them
            // into leaked `&'static str`s. `intern` dedups, so re-rendering the
            // same syntax every frame doesn't leak.
            let kw: BTreeSet<&'static str> = custom.keywords.iter().map(|k| intern(k)).collect();
            let ty: BTreeSet<&'static str> = custom.types.iter().map(|t| intern(t)).collect();
            let sp: BTreeSet<&'static str> = custom.special.iter().map(|s| intern(s)).collect();
            let mut custom_syntax = Syntax::new(intern(&custom.language))
                .with_case_sensitive(custom.case_sensitive)
                .with_keywords(kw)
                .with_types(ty)
                .with_special(sp);
            if let Some(comment) = &custom.comment {
                custom_syntax = custom_syntax.with_comment(intern(comment));
            }
            if let Some((start, end)) = &custom.comment_multiline {
                custom_syntax = custom_syntax.with_comment_multiline([intern(start), intern(end)]);
            }

            custom_syntax
        } else {
            match self.syntax.as_deref() {
                Some("rust") => Syntax::rust(),
                Some("sql") => Syntax::sql(),
                Some("shell") | Some("sh") | Some("bash") => Syntax::shell(),
                // The crate has no built-in JSON syntax; approximate it so JSON
                // content isn't rendered as plain text: highlight the literals and
                // treat strings via the default quote handling.
                Some("json") => Syntax::new("json")
                    .with_comment("//")
                    .with_keywords(["true", "false", "null"]),
                _ => Syntax::default(),
            }
        };
        // Fall back to a per-`ui` id when no explicit id is set, so multiple
        // anonymous editors don't share egui state (focus/cursor/scroll).
        // `id_source` wants a `String`; derive a stable-but-unique one from the
        // current ui's auto id when no explicit id was provided.
        let id_source: String = if self.id.is_empty() {
            format!("sdk_code_editor_{:?}", ui.next_auto_id())
        } else {
            self.id.clone()
        };
        let theme = colors.code_editor_theme();

        // A single themed border around the whole editor; the inner `TextEdit`'s
        // own frame/focus stroke is suppressed so it doesn't draw a second border
        // around the code area on hover/selection.
        let stroke = if self.bordered {
            egui::Stroke::new(1.0, colors.surface_raised)
        } else {
            egui::Stroke::NONE
        };
        // Reserve a left gutter for ▶ run-markers when present.
        let gutter: i8 = if self.run_markers.is_empty() { 0 } else { 18 };
        // Stable id base for marker hit-testing (computed before `id_source` is
        // moved into the editor below). Derived from the `ui` (not a global
        // `Id::new`) so two editors sharing a string id — e.g. the same plugin
        // open in two tabs — don't collide on the same widget id.
        let marker_id_base = ui.make_persistent_id(("sdk_code_editor_markers", id_source.as_str()));

        let frame_resp = egui::Frame::new()
            .fill(colors.bg)
            .stroke(stroke)
            .corner_radius(4)
            // Top padding so the first line sits a little below the border.
            .inner_margin(egui::Margin {
                left: gutter,
                right: 0,
                top: 6,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().widgets.hovered.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().selection.stroke = egui::Stroke::NONE;
                ui.add_enabled_ui(!self.disabled, |ui| {
                    // Autocomplete: drive egui_code_editor's `Completer`, seeded
                    // from the syntax keyword/type/special word lists. The
                    // completer carries pop-up + selection state, so it's cached
                    // in egui memory (keyed by the editor id) across frames rather
                    // than rebuilt — the SDK rebuilds `CodeEditor` from JSON each
                    // frame. An empty syntax yields an empty trie (no pop-up).
                    //
                    // The completer's word trie is built once from the syntax, so
                    // it must be rebuilt when that word set changes — e.g. a plugin
                    // streams table names into `special` after the first frame.
                    // Cache it alongside a fingerprint of the words and rebuild on
                    // change; within a stable set the pop-up/selection state is
                    // preserved.
                    let words_fingerprint = self.words_fingerprint();
                    let completer_id =
                        ui.make_persistent_id(("sdk_code_editor_completer", id_source.as_str()));
                    // Focus flag id (computed here while `id_source` is still
                    // owned; it's moved into the editor below).
                    let focus_id =
                        ui.make_persistent_id(("sdk_code_editor_focus", id_source.as_str()));
                    let mut completer = ui
                        .ctx()
                        .memory(|m| m.data.get_temp::<(u64, Completer)>(completer_id))
                        .filter(|(fp, _)| *fp == words_fingerprint)
                        .map(|(_, c)| c)
                        .unwrap_or_else(|| Completer::new_with_syntax(&syntax));

                    let mut editor = Editor::default()
                        .id_source(id_source)
                        .with_fontsize(self.font_size.unwrap_or(13.0))
                        .with_theme(theme)
                        .with_syntax(syntax);
                    if let Some(rows) = self.rows {
                        editor = editor.with_rows(rows);
                    }

                    // Accept the highlighted completion on Enter as well as Tab.
                    // The crate only completes on Tab, so when its pop-up is
                    // showing (the "Completer" layer was visible last frame) we
                    // rewrite a plain Enter into a Tab before the completer's
                    // input handler runs — otherwise Enter would insert a newline.
                    // Shift+Enter is left alone, so it still adds a newline.
                    let completer_visible = ui.ctx().memory(|m| {
                        m.areas()
                            .visible_layer_ids()
                            .iter()
                            .any(|l| l.id == egui::Id::new("Completer"))
                    });
                    if completer_visible {
                        ui.input_mut(|i| {
                            if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                                i.events.push(egui::Event::Key {
                                    key: egui::Key::Tab,
                                    physical_key: None,
                                    pressed: true,
                                    repeat: false,
                                    modifiers: egui::Modifiers::NONE,
                                });
                            }
                        });
                    }

                    // Format shortcut (Option/Alt+Shift+F, matching VS Code).
                    // Must run BEFORE the text editor consumes input: on macOS
                    // the Option key is a *compose* modifier, so the OS turns
                    // Option+Shift+F into the character "Ï" and delivers it as an
                    // `Event::Text` that the TextEdit would otherwise insert.
                    // Detect by PHYSICAL key — the logical key is mangled by
                    // composition, so `consume_key(Key::F, …)` never matches on
                    // macOS — then drop both the key and the composed text this
                    // frame so nothing is typed.
                    //
                    // Gated on this editor having had focus last frame: with
                    // several editors visible side-by-side (split tabs) they all
                    // render each frame and read the same global input, so an
                    // unguarded consume would route the shortcut to whichever
                    // renders last, not the one the user is typing in. Focus is
                    // only known after `show()`, so we use the previous frame's
                    // value (a keypress can't land the same frame focus changes).
                    let had_focus = ui
                        .ctx()
                        .memory(|m| m.data.get_temp::<bool>(focus_id).unwrap_or(false));
                    let format = if had_focus {
                        ui.input_mut(|i| {
                            let is_fmt = |e: &egui::Event| {
                                matches!(
                                    e,
                                    egui::Event::Key { key, physical_key, pressed: true, modifiers, .. }
                                        if modifiers.alt
                                            && modifiers.shift
                                            && (*key == egui::Key::F
                                                || *physical_key == Some(egui::Key::F))
                                )
                            };
                            let hit = i.events.iter().any(is_fmt);
                            if hit {
                                i.events
                                    .retain(|e| !(is_fmt(e) || matches!(e, egui::Event::Text(_))));
                            }
                            hit
                        })
                        .then_some(())
                    } else {
                        None
                    };

                    let output = editor.show_with_completer(ui, &mut self.value, &mut completer);
                    let changed = output.response.changed();

                    // Caret + selection as character offsets, for run-at-cursor /
                    // run-selection.
                    let (caret, selection) = match &output.cursor_range {
                        Some(cr) => {
                            let (p, s) = (cr.primary.index, cr.secondary.index);
                            let sel = (p != s).then(|| (p.min(s), p.max(s)));
                            (p, sel)
                        }
                        None => (self.value.chars().count(), None),
                    };

                    // Run shortcuts while focused: ⌘/Ctrl+Shift+Enter runs
                    // everything; ⌘/Ctrl+Enter runs the statement at the caret.
                    // `Modifiers::COMMAND` is ⌘ on macOS and Ctrl elsewhere. The
                    // editor binds newline to a plain Enter, so consuming these
                    // modified combos doesn't insert a line break.
                    let run = if output.response.has_focus() {
                        let all_mods = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
                        if ui.input_mut(|i| i.consume_key(all_mods, egui::Key::Enter)) {
                            Some(RunRequest {
                                all: true,
                                caret,
                                selection,
                            })
                        } else if ui.input_mut(|i| {
                            i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                        }) {
                            Some(RunRequest {
                                all: false,
                                caret,
                                selection,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Remember focus so next frame's format-shortcut gate can
                    // route the key to the editor the user is actually in.
                    let has_focus = output.response.has_focus();
                    ui.ctx().memory_mut(|m| {
                        m.data.insert_temp(focus_id, has_focus);
                        m.data
                            .insert_temp(completer_id, (words_fingerprint, completer))
                    });
                    (
                        CodeEditorOutput {
                            changed,
                            run,
                            format,
                            run_marker: None,
                        },
                        output.galley.clone(),
                        output.galley_pos,
                    )
                })
                .inner
            });

        let (mut out, galley, galley_pos) = frame_resp.inner;

        // ▶ run-markers: paint a clickable play glyph in the left gutter at each
        // marker offset, positioned from the galley (so it tracks scrolling) and
        // clipped to the editor's visible area. Painted after the frame so it
        // isn't clipped to the inner content rect.
        if !self.run_markers.is_empty() {
            let frame_rect = frame_resp.response.rect;
            let gutter_x = frame_rect.left() + gutter as f32 / 2.0;
            for &off in &self.run_markers {
                let local = galley.pos_from_cursor(egui::text::CCursor::new(off));
                let cy = galley_pos.y + local.center().y;
                if cy < frame_rect.top() + 2.0 || cy > frame_rect.bottom() - 2.0 {
                    continue;
                }
                let rect =
                    egui::Rect::from_center_size(egui::pos2(gutter_x, cy), egui::vec2(16.0, 16.0));
                let resp = ui.interact(rect, marker_id_base.with(off), egui::Sense::click());
                let color = if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    colors.accent
                } else {
                    colors.fg_muted
                };
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    egui_phosphor::regular::PLAY,
                    crate::theme::phosphor_font_id(11.0),
                    color,
                );
                if resp.clicked() {
                    out.run_marker = Some(off);
                }
            }
        }
        out
    }
}
