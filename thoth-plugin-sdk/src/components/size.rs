use serde::{Deserialize, Serialize};

/// A shared size preset for interactive components (buttons, selects, tabs,
/// icon buttons). Components map the level to their own dimensions so they stay
/// visually proportional, but all expose the same `size` prop for consistency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum Size {
    /// Compact.
    Small,
    /// Default.
    #[default]
    Medium,
    /// Prominent.
    Large,
}

impl Size {
    /// This size's `(font_size, height)` for buttons and icon buttons — design
    /// `.btn` (26px / 12.5px) and `.btn.sm` (22px / 11.5px).
    ///
    /// Note this is deliberately *shorter* than [`Self::field_metrics`]: the
    /// design sheet sizes buttons at 26px and text-entry controls at 28px, so a
    /// button never looks taller than the input it sits beside.
    pub fn metrics(self) -> (f32, f32) {
        match self {
            Size::Small => (11.5, 22.0),
            Size::Medium => (12.5, 26.0),
            Size::Large => (14.0, 30.0),
        }
    }

    /// This size's `(font_size, height)` for text-entry controls — inputs,
    /// select triggers, number inputs. Design `.field` / `.trigger` / `.num`.
    pub fn field_metrics(self) -> (f32, f32) {
        match self {
            Size::Small => (11.5, 24.0),
            Size::Medium => (12.5, 28.0),
            Size::Large => (14.0, 32.0),
        }
    }
}
