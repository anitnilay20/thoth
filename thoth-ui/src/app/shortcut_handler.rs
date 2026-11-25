use crate::shortcuts::ShortcutExt;
use eframe::egui;

use crate::shortcuts::{KeyboardShortcuts, Shortcut};

/// Actions that can be triggered by keyboard shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    // File operations
    OpenFile,
    NewWindow,

    // Navigation
    ToggleQueryBuilder,
    RunQuery,
    AddFilter,
    GroupBy,
    AddAggregate,
    AddSort,
    NavBack,
    NavForward,
    Escape,

    // Bookmarks
    SaveQuery,
    OpenSavedQueries,

    // Tree operations
    ExpandNode,
    CollapseNode,
    ExpandAll,
    CollapseAll,

    // Clipboard
    CopyKey,
    CopyValue,
    CopyObject,
    CopyPath,

    // Movement
    MoveUp,
    MoveDown,

    // UI
    ToggleSidebar,
    Settings,
    ToggleTheme,

    // Developer
    ToggleProfiler,

    // Tabs
    CloseTab,
    NewTab,
    NextTab,
    PrevTab,
    /// Switch to the tab at 0-based visual index in the focused leaf.
    SwitchToTab(usize),
    /// Open or focus the Nth (0-based) UI/data-source plugin.
    SwitchToPlugin(usize),
}

/// Handle keyboard shortcuts and return triggered actions
pub struct ShortcutHandler;

/// True when a key event with exactly these modifiers is present.
fn has_exact_key_event(ctx: &egui::Context, kc: egui::KeyboardShortcut) -> bool {
    ctx.input(|i| {
        i.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key { key, pressed: true, modifiers, .. }
                    if *key == kc.logical_key && modifiers.matches_exact(kc.modifiers)
            )
        })
    })
}

/// Consume only an exact modifier match. egui's logical matching ignores extra
/// Shift/Alt, which lets a plain chord shadow a shifted chord on the same key.
fn consume_exact(ctx: &egui::Context, shortcut: &Shortcut) -> bool {
    let kc = shortcut.to_keyboard_shortcut();
    if !has_exact_key_event(ctx, kc) {
        return false;
    }
    ctx.input_mut(|i| {
        let before = i.events.len();
        i.events.retain(|event| {
            !matches!(
                event,
                egui::Event::Key { key, pressed: true, modifiers, .. }
                    if *key == kc.logical_key && modifiers.matches_exact(kc.modifiers)
            )
        });
        i.events.len() != before
    })
}

/// Consume matching presses (including key repeats) and report whether at
/// least one was a fresh press, so holding a key does not retrigger.
fn consume_non_repeat(ctx: &egui::Context, shortcut: &Shortcut) -> bool {
    let kc = shortcut.to_keyboard_shortcut();
    let mut fired = false;
    ctx.input_mut(|i| {
        i.events.retain(|event| match event {
            egui::Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } if *key == kc.logical_key && modifiers.matches_logically(kc.modifiers) => {
                if !*repeat {
                    fired = true;
                }
                false
            }
            _ => true,
        });
    });
    fired
}

/// Native integrations turn the platform copy chord (`⌘`/`Ctrl+C`) into
/// `Event::Copy` and never emit the key event. Recover the configured chord
/// from the current modifiers; text editors keep their `Event::Copy`.
fn consume_native_copy(
    ctx: &egui::Context,
    shortcuts: &KeyboardShortcuts,
    modifiers: egui::Modifiers,
) -> Option<ShortcutAction> {
    let has_copy = ctx.input(|i| {
        i.events
            .iter()
            .any(|event| matches!(event, egui::Event::Copy))
    });
    if !has_copy {
        return None;
    }
    let ordered = [
        (&shortcuts.copy_object, ShortcutAction::CopyObject),
        (&shortcuts.copy_value, ShortcutAction::CopyValue),
        (&shortcuts.copy_path, ShortcutAction::CopyPath),
        (&shortcuts.copy_key, ShortcutAction::CopyKey),
    ];
    for (shortcut, action) in ordered {
        let kc = shortcut.to_keyboard_shortcut();
        let is_copy_key = kc.logical_key == egui::Key::C || kc.logical_key == egui::Key::Copy;
        if is_copy_key && modifiers.matches_exact(kc.modifiers) {
            ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Copy)));
            return Some(action);
        }
    }
    None
}

/// Clipboard chords: exact matches first, then the previous logical fallback.
fn consume_clipboard(ctx: &egui::Context, shortcuts: &KeyboardShortcuts) -> Option<ShortcutAction> {
    let ordered = [
        (&shortcuts.copy_object, ShortcutAction::CopyObject),
        (&shortcuts.copy_value, ShortcutAction::CopyValue),
        (&shortcuts.copy_path, ShortcutAction::CopyPath),
        (&shortcuts.copy_key, ShortcutAction::CopyKey),
    ];
    for (shortcut, action) in ordered {
        if consume_exact(ctx, shortcut) {
            return Some(action);
        }
    }
    for (shortcut, action) in ordered {
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut.to_keyboard_shortcut())) {
            return Some(action);
        }
    }
    None
}

/// History chords: exact matches first so a shifted forward chord is not
/// swallowed by a plain back chord that accepts extra modifiers.
fn consume_nav(ctx: &egui::Context, shortcuts: &KeyboardShortcuts) -> Option<ShortcutAction> {
    let ordered = [
        (&shortcuts.nav_back, ShortcutAction::NavBack),
        (&shortcuts.nav_forward, ShortcutAction::NavForward),
    ];
    for (shortcut, action) in ordered {
        if consume_exact(ctx, shortcut) {
            return Some(action);
        }
    }
    for (shortcut, action) in ordered {
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut.to_keyboard_shortcut())) {
            return Some(action);
        }
    }
    None
}

impl ShortcutHandler {
    /// Check for keyboard shortcuts and return any triggered actions
    pub fn handle_shortcuts(
        ctx: &egui::Context,
        shortcuts: &KeyboardShortcuts,
    ) -> Vec<ShortcutAction> {
        let mut actions = Vec::new();

        // Check if any text input has focus - if so, skip navigation shortcuts
        let text_input_has_focus = ctx.memory(|mem| mem.focused().is_some());
        // `Event::Copy` carries no modifier payload, so capture the frame's
        // modifiers once, before any matching can consume events this frame.
        let frame_modifiers = ctx.input(|i| i.modifiers);

        // File operations
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.open_file.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::OpenFile);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.new_window.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::NewWindow);
        }

        // Query builder lanes. Shift-bearing bindings are tested first so
        // ⌘⇧A is not swallowed by a plain-⌘ shortcut on the same letter.
        for (shortcut, action) in [
            (&shortcuts.toggle_query, ShortcutAction::ToggleQueryBuilder),
            (&shortcuts.run_query, ShortcutAction::RunQuery),
            (&shortcuts.add_aggregate, ShortcutAction::AddAggregate),
            (&shortcuts.add_sort, ShortcutAction::AddSort),
            (&shortcuts.add_filter, ShortcutAction::AddFilter),
            (&shortcuts.group_by, ShortcutAction::GroupBy),
        ] {
            if ctx.input_mut(|i| i.consume_shortcut(&shortcut.to_keyboard_shortcut())) {
                actions.push(action);
            }
        }

        // Navigation back/forward, driven by the configured bindings.
        if let Some(action) = consume_nav(ctx, shortcuts) {
            actions.push(action);
        }

        // Tab cycling (⌘⌥→ / ⌘⌥← by default), also from the configured bindings.
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.next_tab.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::NextTab);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.prev_tab.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::PrevTab);
        }

        // Browser-style mouse back/forward buttons
        if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Extra1)) {
            actions.push(ShortcutAction::NavBack);
        }

        if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Extra2)) {
            actions.push(ShortcutAction::NavForward);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.escape.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::Escape);
        }

        // Saved queries — the shift-bearing binding first, so ⌘⇧D is not
        // swallowed by a plain-⌘ shortcut on the same letter.
        if ctx
            .input_mut(|i| i.consume_shortcut(&shortcuts.open_saved_queries.to_keyboard_shortcut()))
        {
            actions.push(ShortcutAction::OpenSavedQueries);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.save_query.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::SaveQuery);
        }

        // Skip tree operations, clipboard, and movement shortcuts when text input has focus
        if !text_input_has_focus {
            // Tree operations
            if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.expand_node.to_keyboard_shortcut()))
            {
                actions.push(ShortcutAction::ExpandNode);
            }

            if ctx
                .input_mut(|i| i.consume_shortcut(&shortcuts.collapse_node.to_keyboard_shortcut()))
            {
                actions.push(ShortcutAction::CollapseNode);
            }

            if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.expand_all.to_keyboard_shortcut())) {
                actions.push(ShortcutAction::ExpandAll);
            }

            if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.collapse_all.to_keyboard_shortcut()))
            {
                actions.push(ShortcutAction::CollapseAll);
            }

            // Movement
            if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.move_up.to_keyboard_shortcut())) {
                actions.push(ShortcutAction::MoveUp);
            }

            if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.move_down.to_keyboard_shortcut())) {
                actions.push(ShortcutAction::MoveDown);
            }
        }

        // Clipboard works on the selected tree node. Only a focused text
        // editor needs `Event::Copy` for its own selection, so this is gated
        // on that rather than on any focus.
        if !ctx.text_edit_focused()
            && let Some(action) = consume_native_copy(ctx, shortcuts, frame_modifiers)
                .or_else(|| consume_clipboard(ctx, shortcuts))
        {
            actions.push(action);
        }

        // UI
        // Holding the key should consume the repeats without flickering the
        // sidebar; only a fresh press toggles it.
        if consume_non_repeat(ctx, &shortcuts.toggle_sidebar) {
            actions.push(ShortcutAction::ToggleSidebar);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.settings.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::Settings);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.toggle_theme.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::ToggleTheme);
        }

        // Developer
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.toggle_profiler.to_keyboard_shortcut()))
        {
            actions.push(ShortcutAction::ToggleProfiler);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.close_tab.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::CloseTab);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.new_tab.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::NewTab);
        }

        // ⌘⇧1–⌘⇧9 / Ctrl+Shift+1–9: open or focus the Nth plugin (checked
        // before SwitchToTab so `Cmd+Shift+1` isn't matched as `Cmd+1`).
        const DIGIT_KEYS: [egui::Key; 9] = [
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
            egui::Key::Num6,
            egui::Key::Num7,
            egui::Key::Num8,
            egui::Key::Num9,
        ];
        let plugin_modifiers = shortcuts.switch_to_plugin.to_keyboard_shortcut().modifiers;
        let allow_plugin_switch = !ctx.text_edit_focused() || plugin_modifiers.command;
        if allow_plugin_switch {
            for (idx, key) in DIGIT_KEYS.iter().enumerate() {
                let kc = egui::KeyboardShortcut::new(plugin_modifiers, *key);
                if has_exact_key_event(ctx, kc) && ctx.input_mut(|i| i.consume_key(plugin_modifiers, *key)) {
                    actions.push(ShortcutAction::SwitchToPlugin(idx));
                    break;
                }
            }
        }

        // ⌘1–⌘9 / Ctrl+1–9: switch to tab by position.
        // Only the modifiers are configurable here; the digits are fixed.
        // While a text editor is focused, Shift/Alt-only and bare-digit
        // bindings are typing input; only a command-based chord is safe.
        let tab_modifiers = shortcuts.switch_to_tab.to_keyboard_shortcut().modifiers;
        let allow_tab_switch = !ctx.text_edit_focused() || tab_modifiers.command;
        if allow_tab_switch {
            for (idx, key) in DIGIT_KEYS.iter().enumerate() {
                if ctx.input_mut(|i| i.consume_key(tab_modifiers, *key)) {
                    actions.push(ShortcutAction::SwitchToTab(idx));
                    break;
                }
            }
        }

        actions
    }

    /// Match and consume any plugin-declared custom shortcut (`[[shortcuts]]`)
    /// that does not conflict with a global built-in shortcut.
    pub fn match_plugin_shortcuts(
        ctx: &egui::Context,
        plugin_shortcuts: &[thoth_core::plugin::PluginShortcut],
        global: &KeyboardShortcuts,
    ) -> Option<String> {
        let reserved = [
            &global.open_file,
            &global.new_window,
            &global.close_tab,
            &global.new_tab,
            &global.next_tab,
            &global.prev_tab,
            &global.toggle_sidebar,
            &global.settings,
            &global.toggle_theme,
            &global.toggle_profiler,
        ];
        for ps in plugin_shortcuts {
            let Some(parsed) = Shortcut::parse_chord(&ps.keys) else {
                continue;
            };
            let kc = parsed.to_keyboard_shortcut();
            let conflicts = reserved
                .iter()
                .any(|r| r.to_keyboard_shortcut() == kc);
            if conflicts {
                continue;
            }
            if (!ctx.text_edit_focused() || kc.modifiers.command || kc.modifiers.ctrl)
                && consume_exact(ctx, &parsed)
            {
                return Some(ps.id.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcuts::Shortcut;

    fn key_event(key: egui::Key, modifiers: egui::Modifiers, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers,
        }
    }

    /// Run one egui frame with the given events and return the actions the
    /// shortcut handler reports.
    fn actions_for_events(
        sc: &KeyboardShortcuts,
        modifiers: egui::Modifiers,
        events: Vec<egui::Event>,
    ) -> Vec<ShortcutAction> {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            modifiers,
            events,
            ..Default::default()
        };

        let mut actions = Vec::new();
        let _ = ctx.run_ui(raw, |ui| {
            actions = ShortcutHandler::handle_shortcuts(ui.ctx(), sc);
        });
        actions
    }

    fn actions_for(
        sc: &KeyboardShortcuts,
        key: egui::Key,
        modifiers: egui::Modifiers,
    ) -> Vec<ShortcutAction> {
        actions_for_events(sc, modifiers, vec![key_event(key, modifiers, false)])
    }

    fn actions_for_shortcut(sc: &KeyboardShortcuts, shortcut: &Shortcut) -> Vec<ShortcutAction> {
        let kc = shortcut.to_keyboard_shortcut();
        actions_for(sc, kc.logical_key, kc.modifiers)
    }

    /// Every binding in `KeyboardShortcuts` must fire exactly its action.
    /// `matches_logically` ignores extra shift/alt modifiers, so a plain
    /// shortcut checked before a shifted one on the same key silently shadows
    /// it; asserting the whole action list catches that.
    #[test]
    fn every_default_shortcut_fires_its_own_action() {
        let sc = KeyboardShortcuts::default();
        let cases: [(&str, &Shortcut, ShortcutAction); 32] = [
            ("open_file", &sc.open_file, ShortcutAction::OpenFile),
            ("new_window", &sc.new_window, ShortcutAction::NewWindow),
            ("close_tab", &sc.close_tab, ShortcutAction::CloseTab),
            ("new_tab", &sc.new_tab, ShortcutAction::NewTab),
            ("next_tab", &sc.next_tab, ShortcutAction::NextTab),
            ("prev_tab", &sc.prev_tab, ShortcutAction::PrevTab),
            (
                "switch_to_tab",
                &sc.switch_to_tab,
                ShortcutAction::SwitchToTab(0),
            ),
            (
                "toggle_query",
                &sc.toggle_query,
                ShortcutAction::ToggleQueryBuilder,
            ),
            ("run_query", &sc.run_query, ShortcutAction::RunQuery),
            ("add_filter", &sc.add_filter, ShortcutAction::AddFilter),
            ("group_by", &sc.group_by, ShortcutAction::GroupBy),
            (
                "add_aggregate",
                &sc.add_aggregate,
                ShortcutAction::AddAggregate,
            ),
            ("add_sort", &sc.add_sort, ShortcutAction::AddSort),
            ("nav_back", &sc.nav_back, ShortcutAction::NavBack),
            ("nav_forward", &sc.nav_forward, ShortcutAction::NavForward),
            ("escape", &sc.escape, ShortcutAction::Escape),
            ("save_query", &sc.save_query, ShortcutAction::SaveQuery),
            (
                "open_saved_queries",
                &sc.open_saved_queries,
                ShortcutAction::OpenSavedQueries,
            ),
            ("expand_node", &sc.expand_node, ShortcutAction::ExpandNode),
            (
                "collapse_node",
                &sc.collapse_node,
                ShortcutAction::CollapseNode,
            ),
            ("expand_all", &sc.expand_all, ShortcutAction::ExpandAll),
            (
                "collapse_all",
                &sc.collapse_all,
                ShortcutAction::CollapseAll,
            ),
            ("copy_key", &sc.copy_key, ShortcutAction::CopyKey),
            ("copy_value", &sc.copy_value, ShortcutAction::CopyValue),
            ("copy_object", &sc.copy_object, ShortcutAction::CopyObject),
            ("copy_path", &sc.copy_path, ShortcutAction::CopyPath),
            ("move_up", &sc.move_up, ShortcutAction::MoveUp),
            ("move_down", &sc.move_down, ShortcutAction::MoveDown),
            (
                "toggle_sidebar",
                &sc.toggle_sidebar,
                ShortcutAction::ToggleSidebar,
            ),
            ("settings", &sc.settings, ShortcutAction::Settings),
            (
                "toggle_theme",
                &sc.toggle_theme,
                ShortcutAction::ToggleTheme,
            ),
            (
                "toggle_profiler",
                &sc.toggle_profiler,
                ShortcutAction::ToggleProfiler,
            ),
        ];

        let mut failures = Vec::new();
        for (name, shortcut, expected) in cases {
            let actions = actions_for_shortcut(&sc, shortcut);
            if actions != vec![expected] {
                failures.push(format!(
                    "{name} ({}): expected [{expected:?}], got {actions:?}",
                    shortcut.format()
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "these shortcuts did not fire their own action:\n  {}",
            failures.join("\n  ")
        );
    }

    #[test]
    fn command_digit_switches_to_tab() {
        let sc = KeyboardShortcuts::default();
        let actions = actions_for(&sc, egui::Key::Num3, egui::Modifiers::COMMAND);
        assert_eq!(actions, vec![ShortcutAction::SwitchToTab(2)]);
    }

    #[test]
    fn handler_honours_rebound_bindings() {
        let sc = KeyboardShortcuts {
            nav_back: Shortcut::new("H").command(),
            nav_forward: Shortcut::new("H").command().shift(),
            next_tab: Shortcut::new("K").command().alt(),
            switch_to_tab: Shortcut::new("1").command().alt(),
            ..Default::default()
        };

        // Exact matching: Shift+H must not be swallowed by plain H.
        assert_eq!(
            actions_for_shortcut(&sc, &sc.nav_back),
            vec![ShortcutAction::NavBack]
        );
        assert_eq!(
            actions_for_shortcut(&sc, &sc.nav_forward),
            vec![ShortcutAction::NavForward]
        );
        assert_eq!(
            actions_for_shortcut(&sc, &sc.next_tab),
            vec![ShortcutAction::NextTab]
        );

        // The digits stay fixed; only the modifiers come from the setting.
        let tab_modifiers = sc.switch_to_tab.to_keyboard_shortcut().modifiers;
        assert_eq!(
            actions_for(&sc, egui::Key::Num5, tab_modifiers),
            vec![ShortcutAction::SwitchToTab(4)]
        );
        // The old modifier must no longer switch, proving Alt is not ignored.
        assert!(actions_for(&sc, egui::Key::Num5, egui::Modifiers::COMMAND).is_empty());
    }

    /// Focus a `TextEdit` in one frame, then run the handler on the next
    /// frame so `text_edit_focused` sees it.
    fn actions_with_text_focus(
        sc: &KeyboardShortcuts,
        event: egui::Event,
        modifiers: egui::Modifiers,
    ) -> Vec<ShortcutAction> {
        let ctx = egui::Context::default();
        let id = egui::Id::new("shortcut-test-text-edit");

        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut text = String::new();
            ui.add(egui::TextEdit::singleline(&mut text).id(id));
            ui.memory_mut(|m| m.request_focus(id));
        });

        let raw = egui::RawInput {
            modifiers,
            events: vec![event],
            ..Default::default()
        };
        let mut actions = Vec::new();
        let _ = ctx.run_ui(raw, |ui| {
            actions = ShortcutHandler::handle_shortcuts(ui.ctx(), sc);
        });
        actions
    }

    #[test]
    fn text_editor_keeps_native_copy_and_bare_digits() {
        let sc = KeyboardShortcuts::default();
        let copy_modifiers = sc.copy_value.to_keyboard_shortcut().modifiers;
        assert!(
            actions_with_text_focus(&sc, egui::Event::Copy, copy_modifiers).is_empty(),
            "a text editor must keep Event::Copy for its own selection"
        );

        // A bare-digit tab switch would steal digits from the editor.
        let sc = KeyboardShortcuts {
            switch_to_tab: Shortcut::new("1"),
            ..Default::default()
        };
        assert!(
            actions_with_text_focus(
                &sc,
                key_event(egui::Key::Num3, egui::Modifiers::NONE, false),
                egui::Modifiers::NONE,
            )
            .is_empty(),
            "bare digits must type while a text editor is focused"
        );

        // Shift-only also produces text (e.g. Shift+3 is "#"), so suppress it.
        let sc = KeyboardShortcuts {
            switch_to_tab: Shortcut::new("1").shift(),
            ..Default::default()
        };
        assert!(
            actions_with_text_focus(
                &sc,
                key_event(egui::Key::Num3, egui::Modifiers::SHIFT, false),
                egui::Modifiers::SHIFT,
            )
            .is_empty(),
            "Shift-only tab bindings must not fire while typing"
        );

        // A command-based chord is safe and stays available while editing.
        let sc = KeyboardShortcuts::default();
        let tab_modifiers = sc.switch_to_tab.to_keyboard_shortcut().modifiers;
        assert_eq!(
            actions_with_text_focus(
                &sc,
                key_event(egui::Key::Num3, tab_modifiers, false),
                tab_modifiers,
            ),
            vec![ShortcutAction::SwitchToTab(2)],
            "command-based tab switching must keep working while editing"
        );
    }

    #[test]
    fn native_copy_event_dispatches_the_configured_chord() {
        let sc = KeyboardShortcuts::default();
        for (shortcut, expected) in [
            (&sc.copy_key, ShortcutAction::CopyKey),
            (&sc.copy_value, ShortcutAction::CopyValue),
            (&sc.copy_object, ShortcutAction::CopyObject),
        ] {
            let modifiers = shortcut.to_keyboard_shortcut().modifiers;
            assert_eq!(
                actions_for_events(&sc, modifiers, vec![egui::Event::Copy]),
                vec![expected],
                "native Event::Copy for {}",
                shortcut.format()
            );
        }
    }

    #[test]
    fn held_sidebar_toggle_fires_once_and_consumes_repeats() {
        let sc = KeyboardShortcuts::default();
        let kc = sc.toggle_sidebar.to_keyboard_shortcut();
        let actions = actions_for_events(
            &sc,
            kc.modifiers,
            vec![
                key_event(kc.logical_key, kc.modifiers, false),
                key_event(kc.logical_key, kc.modifiers, true),
            ],
        );
        assert_eq!(actions, vec![ShortcutAction::ToggleSidebar]);
    }

    #[test]
    fn exact_copy_value_beats_logical_copy_object() {
        // Native copy event carries Shift for Copy Value; a Copy Object bound
        // to the plain chord must not capture it.
        let sc = KeyboardShortcuts {
            copy_object: Shortcut::new("C").command(),
            ..Default::default()
        };
        let copy_value_modifiers = sc.copy_value.to_keyboard_shortcut().modifiers;
        assert_eq!(
            actions_for_events(&sc, copy_value_modifiers, vec![egui::Event::Copy]),
            vec![ShortcutAction::CopyValue]
        );

        // Same precedence on the key-event path for non-copy-key rebinds.
        let sc = KeyboardShortcuts {
            copy_object: Shortcut::new("Y").command(),
            copy_value: Shortcut::new("Y").command().shift(),
            ..Default::default()
        };
        assert_eq!(
            actions_for_shortcut(&sc, &sc.copy_value),
            vec![ShortcutAction::CopyValue]
        );
    }

    #[test]
    fn switch_to_plugin_beats_switch_to_tab() {
        let sc = KeyboardShortcuts::default();
        let mods = sc.switch_to_plugin.to_keyboard_shortcut().modifiers;
        let actions = actions_for_events(
            &sc,
            mods,
            vec![key_event(egui::Key::Num2, mods, false)],
        );
        assert_eq!(actions, vec![ShortcutAction::SwitchToPlugin(1)]);
    }

    #[test]
    fn plugin_shortcut_dispatches_unless_conflicting_with_global() {
        use thoth_core::plugin::PluginShortcut;
        let sc = KeyboardShortcuts::default();
        let plugin_shortcuts = vec![
            PluginShortcut {
                id: "hijack_close".to_string(),
                keys: "cmd+w".to_string(),
                description: String::new(),
            },
            PluginShortcut {
                id: "execute".to_string(),
                keys: "cmd+enter".to_string(),
                description: "Run".to_string(),
            },
        ];

        // Global Cmd+W must be ignored for plugins.
        let ctx_w = egui::Context::default();
        let mods_cmd = egui::Modifiers::COMMAND;
        let _ = ctx_w.run_ui(
            egui::RawInput {
                modifiers: mods_cmd,
                events: vec![key_event(egui::Key::W, mods_cmd, false)],
                ..Default::default()
            },
            |_| {},
        );
        assert_eq!(
            ShortcutHandler::match_plugin_shortcuts(&ctx_w, &plugin_shortcuts, &sc),
            None
        );

        // Plugin Cmd+Enter should match.
        let ctx_enter = egui::Context::default();
        let _ = ctx_enter.run_ui(
            egui::RawInput {
                modifiers: mods_cmd,
                events: vec![key_event(egui::Key::Enter, mods_cmd, false)],
                ..Default::default()
            },
            |_| {},
        );
        assert_eq!(
            ShortcutHandler::match_plugin_shortcuts(&ctx_enter, &plugin_shortcuts, &sc),
            Some("execute".to_string())
        );
    }
}
