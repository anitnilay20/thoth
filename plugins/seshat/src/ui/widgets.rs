//! Small shared UiNode builders used across the Seshat view modules.

use serde_json::{json, Value};

/// A single- or multi-line text input. `grow` makes it fill remaining row width.
pub(crate) fn text_input(
    id: &str,
    label: &str,
    value: &str,
    grow: bool,
    placeholder: &str,
) -> Value {
    json!({
        "type": "text-input", "id": id, "label": label,
        "value": value, "placeholder": placeholder, "grow": grow
    })
}

/// A `button` node. `btype` is `"Elevated"`/`"Text"`; `color` is `"Primary"`/`"Default"`/….
#[allow(clippy::too_many_arguments)]
pub(crate) fn button(
    id: &str,
    label: &str,
    btype: &str,
    color: &str,
    icon: Option<&str>,
    enabled: bool,
    full_width: bool,
) -> Value {
    let mut props = json!({
        "label": label,
        "button-type": btype,
        "color": color,
        "enabled": enabled,
        "full-width": full_width
    });
    if let Some(icon) = icon {
        props["icon"] = json!(icon);
    }
    json!({ "type": "button", "id": id, "props": props })
}

/// Small muted text (used for hints, columns, loading rows).
pub(crate) fn muted(text: &str) -> Value {
    json!({ "type": "text", "value": text, "muted": true, "size": "sm" })
}

/// A tree row backed by the host `DataRow` component: indent + optional caret +
/// optional leading icon + label + optional trailing. Emits `"toggle"` (caret)
/// and `"click"` (row body). `caret` is `Some(expanded)` for an expandable node,
/// `None` for a leaf; `icon` is `(glyph, semantic-color)`.
pub(crate) fn data_row(
    id: &str,
    label: &str,
    indent: usize,
    caret: Option<bool>,
    icon: Option<(&str, &str)>,
    trailing: Option<&str>,
) -> Value {
    let mut node = json!({ "type": "data-row", "id": id, "label": label, "indent": indent });
    if let Some(expanded) = caret {
        node["caret"] = json!(expanded);
    }
    if let Some((glyph, color)) = icon {
        node["icon"] = json!({ "glyph": glyph, "color": color });
    }
    if let Some(t) = trailing {
        node["trailing"] = json!(t);
    }
    node
}
