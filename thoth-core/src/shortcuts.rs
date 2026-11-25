use serde::{Deserialize, Serialize};

/// Keyboard shortcut configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    /// Parse a human-readable chord like `"cmd+enter"`, `"ctrl+shift+k"`, or `"alt+f5"`.
    pub fn parse_chord(chord: &str) -> Option<Self> {
        let mut shortcut = Self::new("");
        for part in chord.split('+').map(str::trim).filter(|s| !s.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "cmd" | "command" | "meta" => shortcut.command = true,
                "ctrl" | "control" => shortcut.ctrl = true,
                "alt" | "option" | "opt" => shortcut.alt = true,
                "shift" => shortcut.shift = true,
                "enter" | "return" => shortcut.key = "Enter".to_string(),
                "esc" | "escape" => shortcut.key = "Escape".to_string(),
                "space" => shortcut.key = "Space".to_string(),
                "tab" => shortcut.key = "Tab".to_string(),
                "comma" | "," => shortcut.key = "Comma".to_string(),
                "slash" | "/" => shortcut.key = "Slash".to_string(),
                other => {
                    let mut chars = other.chars();
                    let first = chars.next()?;
                    if chars.next().is_none() && first.is_ascii_alphabetic() {
                        shortcut.key = first.to_ascii_uppercase().to_string();
                    } else {
                        shortcut.key = part.to_string();
                    }
                }
            }
        }
        (!shortcut.key.is_empty()).then_some(shortcut)
    }
}

/// All keyboard shortcuts for the application
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Switch to tab 1–9. The key is ignored; only the modifiers are used, so
    /// this is the binding's modifier set rather than a single key.
    pub switch_to_tab: Shortcut,
    /// Open or focus plugin 1–9 (key ignored; uses modifiers + 1–9, default `Cmd/Ctrl+Shift`).
    pub switch_to_plugin: Shortcut,

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
    pub toggle_sidebar: Shortcut,
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
            switch_to_tab: Shortcut::new("1").command(),
            switch_to_plugin: Shortcut::new("1").command().shift(),

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
            toggle_sidebar: Shortcut::new("B").command(),
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

    /// Every bindable action must appear in `settings.toml`, so a user can
    /// inspect and change it without touching code. This also catches a new
    /// field being added without serialisation or with a renamed key.
    #[test]
    fn all_shortcuts_are_serialised_to_settings() {
        let shortcuts = KeyboardShortcuts::default();
        let toml = toml::to_string(&shortcuts).expect("serialize shortcuts");
        let names = [
            "open_file",
            "new_window",
            "tab_close",
            "tab_new",
            "tab_cycle_next",
            "tab_cycle_prev",
            "switch_to_tab",
            "toggle_query",
            "run_query",
            "add_filter",
            "group_by",
            "add_aggregate",
            "add_sort",
            "nav_back",
            "nav_forward",
            "escape",
            "save_query",
            "open_saved_queries",
            "expand_node",
            "collapse_node",
            "expand_all",
            "collapse_all",
            "copy_key",
            "copy_value",
            "copy_object",
            "copy_path",
            "move_up",
            "move_down",
            "toggle_sidebar",
            "settings",
            "toggle_theme",
            "toggle_profiler",
            "switch_to_plugin",
        ];
        for name in names {
            assert!(
                toml.contains(&format!("[{name}]")),
                "`{name}` is missing from the serialised settings"
            );
        }

        let round_trip: KeyboardShortcuts = toml::from_str(&toml).expect("deserialize shortcuts");
        assert_eq!(round_trip, shortcuts);
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

    #[test]
    fn parse_chord_handles_modifiers_and_keys() {
        assert_eq!(
            Shortcut::parse_chord("cmd+enter"),
            Some(Shortcut::new("Enter").command())
        );
        assert_eq!(
            Shortcut::parse_chord("ctrl+shift+k"),
            Some(Shortcut::new("K").ctrl().shift())
        );
        assert_eq!(Shortcut::parse_chord(""), None);
    }
}
