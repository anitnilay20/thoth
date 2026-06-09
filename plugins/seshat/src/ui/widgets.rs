//! Small shared UiNode builders used across the Seshat view modules.

use serde_json::{json, Value};

use crate::{ICON_CARET_DOWN, ICON_CARET_RIGHT};

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

/// A display-only icon glyph. `color` is a semantic token (warning/info/string/
/// number/secondary/muted/…) or a hex; defaults muted.
pub(crate) fn icon(glyph: &str, color: &str) -> Value {
    json!({ "type": "icon", "glyph": glyph, "color": color })
}

/// A display-only icon glyph at a specific point size.
pub(crate) fn icon_sized(glyph: &str, color: &str, size: f32) -> Value {
    json!({ "type": "icon", "glyph": glyph, "color": color, "size": size })
}

/// An expand/collapse caret icon-button for tree rows.
pub(crate) fn caret(id: &str, expanded: bool) -> Value {
    json!({
        "type": "icon-button", "id": id,
        "icon": if expanded { ICON_CARET_DOWN } else { ICON_CARET_RIGHT },
        "frame": false, "button-size": "Small"
    })
}

/// Indent a block of tree rows by wrapping them in a small left-padded column.
pub(crate) fn indent(children: Vec<Value>) -> Value {
    json!({ "type": "row", "padding": 8, "children": [
        { "type": "column", "gap": 2, "children": children }
    ]})
}
