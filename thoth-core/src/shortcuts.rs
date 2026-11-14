use serde::{Deserialize, Serialize};

/// Keyboard shortcut configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shortcut {
    /// The key to press (e.g., "O", "F", "ArrowRight")
    pub key: String,
    /// Ctrl modifier (always Ctrl, not Command)
    #[serde(default)]
    pub ctrl: bool,
    /// Alt/Option modifier
    #[serde(default)]
    pub alt: bool,
    /// Shift modifier
    #[serde(default)]
    pub shift: bool,
    /// Command/Meta modifier (Cmd on Mac, Ctrl on others - use this for primary shortcuts)
    #[serde(default)]
    pub command: bool,
}

impl Shortcut {
    /// Create a new shortcut
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            ctrl: false,
            alt: false,
            shift: false,
            command: false,
        }
    }

    /// Builder method to add Command modifier (cross-platform primary modifier)
    pub fn command(mut self) -> Self {
        self.command = true;
        self
    }

    /// Builder method to add Ctrl modifier
    #[allow(dead_code)]
    pub fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    /// Builder method to add Alt modifier
    pub fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// Builder method to add Shift modifier
    pub fn shift(mut self) -> Self {
        self.shift = true;
        self
    }
}

/// All keyboard shortcuts for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardShortcuts {
    // File operations
    pub open_file: Shortcut,
    pub new_window: Shortcut,

    // Tab operations — bump serde key names when defaults change to avoid stale persisted values.
    #[serde(rename = "tab_close")]
    pub close_tab: Shortcut,
    #[serde(rename = "tab_new")]
    pub new_tab: Shortcut,
    // v3: ⌘⌥→ / ⌘⌥← (arrow keys, no composition issues, not OS-intercepted)
    #[serde(rename = "tab_cycle_next")]
    pub next_tab: Shortcut,
    #[serde(rename = "tab_cycle_prev")]
    pub prev_tab: Shortcut,

    // Query builder (file tabs only)
    pub toggle_query: Shortcut,
    pub run_query: Shortcut,
    pub add_filter: Shortcut,
    pub group_by: Shortcut,
    pub add_aggregate: Shortcut,
    pub add_sort: Shortcut,

    // Navigation
    pub nav_back: Shortcut,
    pub nav_forward: Shortcut,
    pub escape: Shortcut,

    // Saved queries
    pub save_query: Shortcut,
    pub open_saved_queries: Shortcut,

    // Tree operations
    pub expand_node: Shortcut,
    pub collapse_node: Shortcut,
    pub expand_all: Shortcut,
    pub collapse_all: Shortcut,

    // Clipboard
    pub copy_key: Shortcut,
    pub copy_value: Shortcut,
    pub copy_object: Shortcut,
    pub copy_path: Shortcut,

    // Movement
    pub move_up: Shortcut,
    pub move_down: Shortcut,

    // UI
    pub settings: Shortcut,
    pub toggle_theme: Shortcut,

    // Developer
    pub toggle_profiler: Shortcut,
}

impl Default for KeyboardShortcuts {
    fn default() -> Self {
        Self {
            // File operations - use COMMAND for cross-platform (Cmd on Mac, Ctrl elsewhere)
            open_file: Shortcut::new("O").command(),
            new_window: Shortcut::new("N").command(),

            // Tab operations
            close_tab: Shortcut::new("W").command(),
            new_tab: Shortcut::new("T").command(),
            // ⌘⌥→ / ⌘⌥← — Firefox-style, arrow keys have no char-composition issues.
            next_tab: Shortcut::new("ArrowRight").command().alt(),
            prev_tab: Shortcut::new("ArrowLeft").command().alt(),

            // Query builder lanes. ⌘F and ⌘G are the design handoff's; ⌘⇧A
            // and ⌘⇧S continue the pattern. None of the four is claimed by
            // macOS or Windows, which is why ⌘⇧Esc, ⌘Space, ⌘⇧3/4/5 and the
            // ⌘Tab family are all avoided.
            toggle_query: Shortcut::new("Slash").command(),
            run_query: Shortcut::new("Enter").command(),
            add_filter: Shortcut::new("F").command(),
            group_by: Shortcut::new("G").command(),
            add_aggregate: Shortcut::new("A").command().shift(),
            add_sort: Shortcut::new("S").command().shift(),

            // Navigation
            nav_back: Shortcut::new("BracketLeft").command(),
            nav_forward: Shortcut::new("BracketRight").command(),
            escape: Shortcut::new("Escape"),

            // Saved queries. ⌘S is the handoff's; ⌘⇧D keeps the slot the
            // bookmark panel used, since "show me my saved things" is the
            // same request it always was.
            save_query: Shortcut::new("S").command(),
            open_saved_queries: Shortcut::new("D").command().shift(),

            // Tree operations
            expand_node: Shortcut::new("ArrowRight"),
            collapse_node: Shortcut::new("ArrowLeft"),
            expand_all: Shortcut::new("ArrowRight").command(),
            collapse_all: Shortcut::new("ArrowLeft").command(),

            // Clipboard
            copy_key: Shortcut::new("C").command(),
            copy_value: Shortcut::new("C").command().shift(),
            copy_object: Shortcut::new("C").command().alt(),
            copy_path: Shortcut::new("P").command().shift(),

            // Movement
            move_up: Shortcut::new("ArrowUp"),
            move_down: Shortcut::new("ArrowDown"),

            // UI
            settings: Shortcut::new("Comma").command(),
            toggle_theme: Shortcut::new("T").command().shift(),

            // Developer
            toggle_profiler: Shortcut::new("P").command().alt(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_builder() {
        let shortcut = Shortcut::new("O").command();
        assert_eq!(shortcut.key, "O");
        assert!(shortcut.command);
        assert!(!shortcut.ctrl);

        let complex = Shortcut::new("G").command().shift();
        assert!(complex.command);
        assert!(complex.shift);
    }

    #[test]
    fn test_default_shortcuts() {
        let shortcuts = KeyboardShortcuts::default();
        assert!(shortcuts.open_file.command);
        assert_eq!(shortcuts.open_file.key, "O");
        assert_eq!(shortcuts.close_tab.key, "W");
        assert!(shortcuts.close_tab.command);
        assert_eq!(shortcuts.next_tab.key, "ArrowRight");
        assert!(shortcuts.next_tab.command && shortcuts.next_tab.alt);
        assert_eq!(shortcuts.prev_tab.key, "ArrowLeft");
    }
}
