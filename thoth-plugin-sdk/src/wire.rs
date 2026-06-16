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
