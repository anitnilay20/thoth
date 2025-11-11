use eframe::egui;

use crate::shortcuts::KeyboardShortcuts;

/// Actions that can be triggered by keyboard shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    // File operations
    OpenFile,
    ClearFile,
    NewWindow,

    // Navigation
    FocusSearch,
    NextMatch,
    PrevMatch,
    Escape,

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
    Settings,
    ToggleTheme,
}

/// Handle keyboard shortcuts and return triggered actions
pub struct ShortcutHandler;

impl ShortcutHandler {
    /// Check for keyboard shortcuts and return any triggered actions
    pub fn handle_shortcuts(
        ctx: &egui::Context,
        shortcuts: &KeyboardShortcuts,
    ) -> Vec<ShortcutAction> {
        let mut actions = Vec::new();

        // File operations
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.open_file.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::OpenFile);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.clear_file.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::ClearFile);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.new_window.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::NewWindow);
        }

        // Navigation
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.focus_search.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::FocusSearch);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.next_match.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::NextMatch);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.prev_match.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::PrevMatch);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.escape.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::Escape);
        }

        // Tree operations
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.expand_node.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::ExpandNode);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.collapse_node.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::CollapseNode);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.expand_all.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::ExpandAll);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.collapse_all.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::CollapseAll);
        }

        // Clipboard
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.copy_key.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::CopyKey);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.copy_value.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::CopyValue);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.copy_object.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::CopyObject);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.copy_path.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::CopyPath);
        }

        // Movement
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.move_up.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::MoveUp);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.move_down.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::MoveDown);
        }

        // UI
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.settings.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::Settings);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.toggle_theme.to_keyboard_shortcut())) {
            actions.push(ShortcutAction::ToggleTheme);
        }

        actions
    }
}
