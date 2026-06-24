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

#[cfg(test)]
mod tests {
    use super::TextToken;
    use serde_json::{json, Value};

    // ── From<&Value> conversions ──────────────────────────────────────────────

    #[test]
    fn string_value_yields_str_token() {
        let v = Value::String("hello".into());
        assert_eq!(TextToken::from(&v), TextToken::Str);
    }

    #[test]
    fn number_value_yields_number_token() {
        let v = json!(42);
        assert_eq!(TextToken::from(&v), TextToken::Number);
    }

    #[test]
    fn float_value_yields_number_token() {
        let v = json!(3.14);
        assert_eq!(TextToken::from(&v), TextToken::Number);
    }

    #[test]
    fn bool_true_yields_boolean_token() {
        let v = Value::Bool(true);
        assert_eq!(TextToken::from(&v), TextToken::Boolean);
    }

    #[test]
    fn bool_false_yields_boolean_token() {
        let v = Value::Bool(false);
        assert_eq!(TextToken::from(&v), TextToken::Boolean);
    }

    #[test]
    fn null_yields_boolean_token() {
        let v = Value::Null;
        assert_eq!(TextToken::from(&v), TextToken::Boolean);
    }

    #[test]
    fn array_value_yields_bracket_token() {
        let v = json!([1, 2, 3]);
        assert_eq!(TextToken::from(&v), TextToken::Bracket);
    }

    #[test]
    fn object_value_yields_bracket_token() {
        let v = json!({"a": 1});
        assert_eq!(TextToken::from(&v), TextToken::Bracket);
    }

    // ── serde (kebab-case) ────────────────────────────────────────────────────

    #[test]
    fn serialises_key_as_kebab_case() {
        let s = serde_json::to_string(&TextToken::Key).unwrap();
        assert_eq!(s, r#""key""#);
    }

    #[test]
    fn serialises_str_as_kebab_case() {
        let s = serde_json::to_string(&TextToken::Str).unwrap();
        assert_eq!(s, r#""str""#);
    }

    #[test]
    fn serialises_number_as_kebab_case() {
        let s = serde_json::to_string(&TextToken::Number).unwrap();
        assert_eq!(s, r#""number""#);
    }

    #[test]
    fn serialises_boolean_as_kebab_case() {
        let s = serde_json::to_string(&TextToken::Boolean).unwrap();
        assert_eq!(s, r#""boolean""#);
    }

    #[test]
    fn serialises_bracket_as_kebab_case() {
        let s = serde_json::to_string(&TextToken::Bracket).unwrap();
        assert_eq!(s, r#""bracket""#);
    }

    #[test]
    fn deserialises_from_kebab_case() {
        let token: TextToken = serde_json::from_str(r#""key""#).unwrap();
        assert_eq!(token, TextToken::Key);
    }

    #[test]
    fn round_trips_all_variants() {
        for token in [
            TextToken::Key,
            TextToken::Str,
            TextToken::Number,
            TextToken::Boolean,
            TextToken::Bracket,
        ] {
            let json = serde_json::to_string(&token).unwrap();
            let back: TextToken = serde_json::from_str(&json).unwrap();
            assert_eq!(back, token);
        }
    }
}
