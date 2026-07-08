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
    /// This size's `(font_size, height)` in points/pixels for text controls
    /// like buttons and selects.
    pub fn metrics(self) -> (f32, f32) {
        match self {
            Size::Small => (11.0, 24.0),
            Size::Medium => (13.0, 28.0),
            Size::Large => (15.0, 32.0),
        }
    }
}
