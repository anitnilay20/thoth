//! A reusable error modal — shown whenever `State::error` is set.

use serde_json::{json, Value};

use crate::state::State;
use crate::ui::widgets::button;

/// Renders the error modal (open when `st.error` is `Some`). The close (×) and
/// the Dismiss button both emit `error-close`, which clears the error.
pub(crate) fn error_modal(st: &State) -> Value {
    let message = st.error.as_deref().unwrap_or_default();
    json!({
        "type": "modal",
        "id": "error-modal",
        "title": "Connection error",
        "open": st.error.is_some(),
        "close-id": "error-close",
        "width-pct": 0.4,
        "children": [
            { "type": "column", "gap": 14, "children": [
                { "type": "colored", "color": "#f38ba8",
                  "child": { "type": "text", "value": message } },
                { "type": "row", "gap": 8, "align": "fill", "children": [
                    { "type": "spacer" },
                    button("error-close", "Dismiss", "Elevated", "Primary", None, true, false)
                ]}
            ]}
        ]
    })
}
