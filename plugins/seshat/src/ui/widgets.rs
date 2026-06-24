//! Small shared `RenderNode` builders used across the Seshat view modules.

use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonType, DataRow, DataRowIcon, Input, Typography, TypographyVariant,
};
use thoth_plugin_sdk::render_node::RenderNode;
use thoth_plugin_sdk::tokens::TextToken;

/// A single-line text input; `grow` makes it fill remaining row width.
pub(crate) fn text_input(
    id: &str,
    label: &str,
    value: &str,
    grow: bool,
    placeholder: &str,
) -> RenderNode {
    RenderNode::Input(
        Input::builder()
            .id(id)
            .label(label)
            .value(value.to_string())
            .placeholder(placeholder.to_string())
            .grow(grow)
            .build(),
    )
}

/// A button node. `btype` is `"Elevated"`/`"Text"`; `color` is `"Primary"`/`"Default"`/….
#[allow(clippy::too_many_arguments)]
pub(crate) fn button(
    id: &str,
    label: &str,
    btype: &str,
    color: &str,
    icon: Option<&str>,
    enabled: bool,
    full_width: bool,
) -> RenderNode {
    let button_type = match btype {
        "Text" => ButtonType::Text,
        _ => ButtonType::Elevated,
    };
    let color = match color {
        "Primary" => ButtonColor::Primary,
        "Secondary" => ButtonColor::Secondary,
        "Danger" => ButtonColor::Danger,
        "Success" => ButtonColor::Success,
        _ => ButtonColor::Default,
    };
    RenderNode::Button(
        Button::builder()
            .id(id)
            .label(label)
            .button_type(button_type)
            .color(color)
            .enabled(enabled)
            .full_width(full_width)
            .maybe_icon(icon.map(|s| s.to_string()))
            .build(),
    )
}

/// Small muted text (hints, columns, loading rows).
pub(crate) fn muted(text: &str) -> RenderNode {
    RenderNode::Text(
        Typography::builder()
            .text(text)
            .variant(TypographyVariant::Caption)
            .build(),
    )
}

/// A tree row backed by the shared `DataRow` component. `caret` is
/// `Some(expanded)` for an expandable node, `None` for a leaf; `icon` is
/// `(glyph, semantic-color)`.
pub(crate) fn data_row(
    id: &str,
    label: &str,
    indent: usize,
    caret: Option<bool>,
    icon: Option<(&str, &str)>,
    trailing: Option<&str>,
) -> RenderNode {
    // `color` is a semantic token (e.g. "muted", "string", "warning"); the SDK
    // resolves it against the active theme.
    let leading =
        icon.map(|(glyph, color)| DataRowIcon::builder().glyph(glyph).color(color).build());
    RenderNode::DataRow(
        DataRow::builder()
            .row_id(id)
            .display_text(label.to_string())
            .key_token(TextToken::Key)
            .indent(indent)
            .maybe_caret(caret)
            .maybe_leading_icon(leading)
            .maybe_trailing(trailing.map(|s| s.to_string()))
            .build(),
    )
}
