//! egui-facing helpers for keyboard shortcuts.
//!
//! The serialisable [`Shortcut`]/[`KeyboardShortcuts`] data lives in
//! `thoth-core`; matching against egui input and rendering the key chips is a
//! display concern and lives here.

pub use thoth_core::shortcuts::{KeyboardShortcuts, Shortcut};

use eframe::egui;

/// Display marks for the keys a shortcut is written with.
///
/// Phosphor glyphs, never the Unicode symbols (`⌘`, `⌥`, `↵`). Whether a
/// plain-text symbol has a glyph depends on the fonts the machine happens to
/// carry: `⌘` resolves through a system fallback on macOS while `↵` does not,
/// so a hand-written "⌘↵" renders as a command mark followed by an empty box.
/// Phosphor ships inside the binary, so every mark is present everywhere.
///
/// One definition, used by every shortcut chip in the app — a second copy is
/// how the boxes got in.
pub mod marks {
    use egui_phosphor::regular as ph;

    /// Ctrl.
    pub const CONTROL: &str = ph::CONTROL;
    /// Alt / Option.
    pub const OPTION: &str = ph::OPTION;
    /// Shift.
    pub const SHIFT: &str = ph::ARROW_FAT_UP;
    /// Return / Enter — the `↵` arrow, not `KEY_RETURN`.
    ///
    /// Phosphor's `key-return` draws the whole *key cap*: a rounded box with
    /// the arrow inside it. At the 11–13px a shortcut chip is set in, the box
    /// closes up and the mark reads as a smudge. The bare elbow arrow is the
    /// symbol people actually know.
    pub const RETURN: &str = ph::ARROW_ELBOW_DOWN_LEFT;

    /// The "command" modifier: ⌘ on macOS, Ctrl everywhere else.
    pub fn command() -> &'static str {
        if cfg!(target_os = "macos") {
            ph::COMMAND
        } else {
            CONTROL
        }
    }
}

/// egui-facing behaviour for a [`Shortcut`].
pub trait ShortcutExt {
    /// Convert to egui's KeyboardShortcut.
    fn to_keyboard_shortcut(&self) -> egui::KeyboardShortcut;
    /// Format the shortcut for display using egui-phosphor icons.
    fn format(&self) -> String;
}

impl ShortcutExt for Shortcut {
    fn to_keyboard_shortcut(&self) -> egui::KeyboardShortcut {
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

    fn format(&self) -> String {
        let mut parts = Vec::new();

        // Add modifier icons (using egui-phosphor)
        if self.ctrl {
            parts.push(marks::CONTROL);
        }
        if self.alt {
            parts.push(marks::OPTION);
        }
        if self.shift {
            parts.push(marks::SHIFT);
        }
        if self.command {
            parts.push(marks::command());
        }

        // Format the key name
        let key_display = format_key_name(&self.key);
        parts.push(&key_display);

        parts.join(" ")
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

/// Format key name for display using egui-phosphor icons where applicable
fn format_key_name(key: &str) -> String {
    match key {
        "ArrowLeft" => egui_phosphor::regular::ARROW_LEFT.to_string(),
        "ArrowRight" => egui_phosphor::regular::ARROW_RIGHT.to_string(),
        "ArrowUp" => egui_phosphor::regular::ARROW_UP.to_string(),
        "ArrowDown" => egui_phosphor::regular::ARROW_DOWN.to_string(),
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
    fn test_format_shortcut() {
        let shortcut = Shortcut::new("O").command();
        let formatted = shortcut.format();

        // Should contain the command/control icon and the key
        assert!(formatted.contains("O"));

        #[cfg(target_os = "macos")]
        assert!(formatted.contains(egui_phosphor::regular::COMMAND));

        #[cfg(not(target_os = "macos"))]
        assert!(formatted.contains(egui_phosphor::regular::CONTROL));
    }

    #[test]
    fn test_parse_key() {
        assert_eq!(parse_key("O"), egui::Key::O);
        assert_eq!(parse_key("Escape"), egui::Key::Escape);
        assert_eq!(parse_key("ArrowLeft"), egui::Key::ArrowLeft);
        assert_eq!(parse_key("Comma"), egui::Key::Comma);
    }
}

#[cfg(test)]
mod mark_tests {
    use super::*;

    /// Unicode symbols that render as an empty box wherever the machine's
    /// fonts happen not to carry them. Phosphor has a glyph for each.
    ///
    /// The list is the *tofu* risk, not every character that could have been
    /// an icon. A dash or an arrow inside a sentence ("v1.2 → v1.3") is
    /// punctuation and stays; a mark standing on its own as a tick, a
    /// direction or a key is an icon and belongs to Phosphor.
    const TOFU_RISK: [(&str, &str); 10] = [
        ("\u{21B5}", "marks::RETURN"),                 // ↵
        ("\u{23CE}", "marks::RETURN"),                 // ⏎
        ("\u{2318}", "marks::command()"),              // ⌘
        ("\u{2325}", "marks::OPTION"),                 // ⌥
        ("\u{21E7}", "marks::SHIFT"),                  // ⇧
        ("\u{2303}", "marks::CONTROL"),                // ⌃
        ("\u{2713}", "egui_phosphor::regular::CHECK"), // ✓
        ("\u{2714}", "egui_phosphor::regular::CHECK"), // ✔
        ("\u{2717}", "egui_phosphor::regular::X"),     // ✗
        ("\u{2718}", "egui_phosphor::regular::X"),     // ✘
    ];

    /// Every mark the app writes shortcuts with must be a Phosphor glyph.
    #[test]
    fn every_key_mark_is_a_phosphor_glyph() {
        let phosphor = |s: &str| s.chars().all(|c| ('\u{E000}'..='\u{F8FF}').contains(&c));
        for (name, mark) in [
            ("CONTROL", marks::CONTROL),
            ("OPTION", marks::OPTION),
            ("SHIFT", marks::SHIFT),
            ("RETURN", marks::RETURN),
        ] {
            assert!(phosphor(mark), "marks::{name} is not a Phosphor glyph");
        }
        #[cfg(target_os = "macos")]
        assert!(
            phosphor(marks::command()),
            "marks::command() is not Phosphor"
        );
    }

    /// No source file writes one of the risky symbols into a user-facing
    /// string. Comments are exempt — naming the character is how the rule is
    /// explained — and so is this file, which has to spell them to ban them.
    #[test]
    fn no_source_file_hand_writes_a_key_symbol() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("thoth-ui lives one level below the workspace root");
        let mut offenders = Vec::new();

        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if name != "target" && !name.starts_with('.') {
                        walk(&path, out);
                    }
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }

        let mut files = Vec::new();
        for crate_src in ["thoth-core", "thoth-cli", "thoth-ui", "thoth-plugin-sdk"] {
            walk(&root.join(crate_src).join("src"), &mut files);
        }

        for file in files {
            // This file spells the symbols in order to forbid them.
            if file.ends_with("shortcuts.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            for (lineno, line) in text.lines().enumerate() {
                let code = line.trim_start();
                // Comments may name the character; only code is checked.
                if code.starts_with("//") || code.starts_with("///") || code.starts_with("*") {
                    continue;
                }
                for (symbol, instead) in TOFU_RISK {
                    if line.contains(symbol) {
                        offenders.push(format!(
                            "{}:{} writes {symbol:?} — use {instead}",
                            file.strip_prefix(root).unwrap_or(&file).display(),
                            lineno + 1,
                        ));
                    }
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "a Unicode key symbol has no glyph on some machines and renders as \
             an empty box; use the Phosphor mark instead:\n  {}",
            offenders.join("\n  ")
        );
    }
}
