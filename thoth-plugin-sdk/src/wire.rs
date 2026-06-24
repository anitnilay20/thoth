//! Plugin-side wire protocol: turning DSL node trees into the JSON the host
//! consumes across the FFI boundary. The host travels the opposite direction
//! (JSON → node via `Deserialize`), so this module is gated to plugin builds.

use serde::Serialize;

/// Convert any DSL node into the wire JSON the host consumes.
///
/// Blanket-implemented for every [`Serialize`] type, so each component (and the
/// composed node tree as a whole) gets `to_json()` for free — there is nothing
/// to implement per component. Serialization of these plain-data DSL structs is
/// effectively infallible; the only way `to_value` can fail is a non-finite
/// float (e.g. a `NaN` width), which is a programming error worth surfacing
/// loudly rather than silently emitting a broken node.
pub trait ToNodeJson: Serialize {
    /// Serialize this node into a [`serde_json::Value`].
    ///
    /// Panics only if the node contains a non-finite float — see the trait
    /// docs. Use [`ToNodeJson::try_to_json`] when a value may legitimately fail.
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("DSL node serialization should be infallible")
    }

    /// Fallible variant of [`ToNodeJson::to_json`] for callers that want to
    /// handle serialization errors explicitly.
    fn try_to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

impl<T: Serialize + ?Sized> ToNodeJson for T {}

#[cfg(test)]
mod tests {
    use super::ToNodeJson;
    use serde::Serialize;
    use serde_json::{json, Value};

    #[derive(Serialize)]
    struct Simple {
        x: u32,
        label: String,
    }

    // ── to_json ───────────────────────────────────────────────────────────────

    #[test]
    fn to_json_serialises_struct_to_value() {
        let s = Simple { x: 7, label: "hi".into() };
        let v = s.to_json();
        assert_eq!(v, json!({"x": 7, "label": "hi"}));
    }

    #[test]
    fn to_json_on_string_produces_json_string() {
        let s = "hello".to_string();
        let v = s.to_json();
        assert_eq!(v, Value::String("hello".into()));
    }

    #[test]
    fn to_json_on_number() {
        let v = 42u32.to_json();
        assert_eq!(v, json!(42));
    }

    #[test]
    fn to_json_on_vec() {
        let v = vec![1u32, 2, 3].to_json();
        assert_eq!(v, json!([1, 2, 3]));
    }

    #[test]
    fn to_json_on_option_none() {
        let v: Option<u32> = None;
        assert_eq!(v.to_json(), Value::Null);
    }

    #[test]
    fn to_json_on_option_some() {
        let v: Option<u32> = Some(5);
        assert_eq!(v.to_json(), json!(5));
    }

    // ── try_to_json ───────────────────────────────────────────────────────────

    #[test]
    fn try_to_json_returns_ok_for_valid_value() {
        let s = Simple { x: 1, label: "ok".into() };
        assert!(s.try_to_json().is_ok());
    }

    #[test]
    fn try_to_json_matches_to_json_for_normal_values() {
        let s = Simple { x: 99, label: "test".into() };
        let expected = s.to_json();
        let actual = s.try_to_json().unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn try_to_json_fails_for_nan_float() {
        // f32/f64 NaN is not representable as JSON — try_to_json returns Err.
        #[derive(Serialize)]
        struct WithFloat {
            v: f32,
        }
        let bad = WithFloat { v: f32::NAN };
        assert!(bad.try_to_json().is_err());
    }

    // ── blanket impl covers &T ────────────────────────────────────────────────

    #[test]
    fn to_json_works_on_ref() {
        let s = Simple { x: 3, label: "ref".into() };
        let v = (&s).to_json();
        assert_eq!(v["x"], json!(3));
    }
}
