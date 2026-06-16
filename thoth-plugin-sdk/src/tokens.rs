//! Pure-data syntax token classes, shared by the DSL and the renderer.
//!
//! This lives outside the egui-gated [`crate::theme`] module so the data layer
//! (which compiles without egui, e.g. for wasm plugins) can reference it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A syntax-highlighting token class for JSON/code values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextToken {
    /// Object key / identifier.
    Key,
    /// String literal.
    Str,
    /// Numeric literal.
    Number,
    /// Boolean or null literal.
    Boolean,
    /// Bracket / punctuation (arrays, objects).
    Bracket,
}

impl From<&Value> for TextToken {
    fn from(value: &Value) -> Self {
        match value {
            Value::String(_) => Self::Str,
            Value::Number(_) => Self::Number,
            Value::Bool(_) => Self::Boolean,
            Value::Array(_) | Value::Object(_) => Self::Bracket,
            Value::Null => Self::Boolean,
        }
    }
}
