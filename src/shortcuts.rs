use eframe::egui;
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

    /// Convert to egui's KeyboardShortcut
    pub fn to_keyboard_shortcut(&self) -> egui::KeyboardShortcut {
        let mut modifiers = egui::Modifiers::default();

        if self.command {
            modifiers |= egui::Modifiers::COMMAND; // Cmd on Mac, Ctrl elsewhere
        }
        if self.ctrl {
            modifiers |= egui::Modifiers::CTRL;
        }
        if self.alt {
            modifiers |= egui::Modifiers::ALT;
        }
        if self.shift {
            modifiers |= egui::Modifiers::SHIFT;
        }

        let key = parse_key(&self.key);

        egui::KeyboardShortcut::new(modifiers, key)
    }

    /// Format shortcut for display (e.g., "Cmd+O", "Ctrl+Shift+F")
    pub fn format(&self) -> String {
        let mut parts = Vec::new();

        #[cfg(target_os = "macos")]
        {
            if self.ctrl {
                parts.push("⌃");
            }
            if self.alt {
                parts.push("⌥");
            }
            if self.shift {
                parts.push("⇧");
            }
            if self.command {
                parts.push("⌘");
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            if self.command || self.ctrl {
                parts.push("Ctrl");
            }
            if self.alt {
                parts.push("Alt");
            }
            if self.shift {
                parts.push("Shift");
            }
        }

        // Format the key name
        let key_display = format_key_name(&self.key);
        parts.push(&key_display);

        #[cfg(target_os = "macos")]
        {
            parts.join("")
        }

        #[cfg(not(target_os = "macos"))]
        {
            parts.join("+")
        }
    }
}

/// All keyboard shortcuts for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardShortcuts {
    // File operations
    pub open_file: Shortcut,
    pub clear_file: Shortcut,
    pub new_window: Shortcut,

    // Navigation
    pub focus_search: Shortcut,
    pub next_match: Shortcut,
    pub prev_match: Shortcut,
    pub escape: Shortcut,

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
            clear_file: Shortcut::new("W").command(),
            new_window: Shortcut::new("N").command(),

            // Navigation
            focus_search: Shortcut::new("F").command(),
            next_match: Shortcut::new("G").command(),
            prev_match: Shortcut::new("G").command().shift(),
            escape: Shortcut::new("Escape"),

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

/// Parse key string to egui Key
fn parse_key(key_str: &str) -> egui::Key {
    match key_str {
        // Special keys
        "Escape" => egui::Key::Escape,
        "Enter" => egui::Key::Enter,
        "Tab" => egui::Key::Tab,
        "Space" => egui::Key::Space,
        "Backspace" => egui::Key::Backspace,
        "Delete" => egui::Key::Delete,

        // Arrow keys
        "ArrowLeft" => egui::Key::ArrowLeft,
        "ArrowRight" => egui::Key::ArrowRight,
        "ArrowUp" => egui::Key::ArrowUp,
        "ArrowDown" => egui::Key::ArrowDown,

        // Function keys
        "F1" => egui::Key::F1,
        "F2" => egui::Key::F2,
        "F3" => egui::Key::F3,
        "F4" => egui::Key::F4,
        "F5" => egui::Key::F5,
        "F6" => egui::Key::F6,
        "F7" => egui::Key::F7,
        "F8" => egui::Key::F8,
        "F9" => egui::Key::F9,
        "F10" => egui::Key::F10,
        "F11" => egui::Key::F11,
        "F12" => egui::Key::F12,

        // Special characters that need mapping
        "Comma" => egui::Key::Comma,
        "Period" => egui::Key::Period,
        "Slash" => egui::Key::Slash,
        "Backslash" => egui::Key::Backslash,
        "Semicolon" => egui::Key::Semicolon,
        "Quote" => egui::Key::Quote,
        "Backtick" => egui::Key::Backtick,
        "Minus" => egui::Key::Minus,
        "Equal" => egui::Key::Equals,
        "BracketLeft" => egui::Key::OpenBracket,
        "BracketRight" => egui::Key::CloseBracket,

        // Numbers
        "0" => egui::Key::Num0,
        "1" => egui::Key::Num1,
        "2" => egui::Key::Num2,
        "3" => egui::Key::Num3,
        "4" => egui::Key::Num4,
        "5" => egui::Key::Num5,
        "6" => egui::Key::Num6,
        "7" => egui::Key::Num7,
        "8" => egui::Key::Num8,
        "9" => egui::Key::Num9,

        // Letters - single uppercase letter
        key if key.len() == 1 => {
            // Safe: we just checked len() == 1, so there's always a first char
            let ch = key.chars().next().map(|c| c.to_ascii_uppercase()).unwrap();
            match ch {
                'A' => egui::Key::A,
                'B' => egui::Key::B,
                'C' => egui::Key::C,
                'D' => egui::Key::D,
                'E' => egui::Key::E,
                'F' => egui::Key::F,
                'G' => egui::Key::G,
                'H' => egui::Key::H,
                'I' => egui::Key::I,
                'J' => egui::Key::J,
                'K' => egui::Key::K,
                'L' => egui::Key::L,
                'M' => egui::Key::M,
                'N' => egui::Key::N,
                'O' => egui::Key::O,
                'P' => egui::Key::P,
                'Q' => egui::Key::Q,
                'R' => egui::Key::R,
                'S' => egui::Key::S,
                'T' => egui::Key::T,
                'U' => egui::Key::U,
                'V' => egui::Key::V,
                'W' => egui::Key::W,
                'X' => egui::Key::X,
                'Y' => egui::Key::Y,
                'Z' => egui::Key::Z,
                _ => egui::Key::A, // Fallback
            }
        }

        // Default fallback
        _ => egui::Key::A,
    }
}

/// Format key name for display
fn format_key_name(key: &str) -> String {
    match key {
        "ArrowLeft" => "←".to_string(),
        "ArrowRight" => "→".to_string(),
        "ArrowUp" => "↑".to_string(),
        "ArrowDown" => "↓".to_string(),
        "Escape" => "Esc".to_string(),
        "Comma" => ",".to_string(),
        "Period" => ".".to_string(),
        "Slash" => "/".to_string(),
        "Backslash" => "\\".to_string(),
        "Semicolon" => ";".to_string(),
        "Quote" => "'".to_string(),
        "Backtick" => "`".to_string(),
        "Minus" => "-".to_string(),
        "Equal" => "=".to_string(),
        "BracketLeft" => "[".to_string(),
        "BracketRight" => "]".to_string(),
        _ => key.to_string(),
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
    fn test_format_shortcut() {
        let shortcut = Shortcut::new("O").command();
        let formatted = shortcut.format();

        #[cfg(target_os = "macos")]
        assert_eq!(formatted, "⌘O");

        #[cfg(not(target_os = "macos"))]
        assert_eq!(formatted, "Ctrl+O");
    }

    #[test]
    fn test_parse_key() {
        assert_eq!(parse_key("O"), egui::Key::O);
        assert_eq!(parse_key("Escape"), egui::Key::Escape);
        assert_eq!(parse_key("ArrowLeft"), egui::Key::ArrowLeft);
        assert_eq!(parse_key("Comma"), egui::Key::Comma);
    }

    #[test]
    fn test_default_shortcuts() {
        let shortcuts = KeyboardShortcuts::default();
        assert!(shortcuts.open_file.command);
        assert_eq!(shortcuts.open_file.key, "O");
    }
}
