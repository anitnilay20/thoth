//! A reusable error modal — shown whenever `State::error` is set.

use thoth_plugin_sdk::components::{Align, Colored, Column, Modal, Row, Typography};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::State;
use crate::ui::widgets::button;

/// Renders the error modal (open when `st.error` is `Some`). The close (×) and
/// the Dismiss button both emit `error-close`, which clears the error.
pub(crate) fn error_modal(st: &State) -> RenderNode {
    let message = st.error.as_deref().unwrap_or_default().to_string();
    RenderNode::Modal(Box::new(
        Modal::builder()
            .id("error-modal")
            .title("Connection error")
            .open(st.error.is_some())
            .close_id("error-close")
            .width_pct(0.4)
            .children(vec![RenderNode::Column(
                Column::builder()
                    .gap(14.0)
                    .children(vec![
                        RenderNode::Colored(
                            Colored::builder()
                                .color("error")
                                .child(RenderNode::Text(
                                    Typography::builder().text(message).build(),
                                ))
                                .build(),
                        ),
                        RenderNode::Row(
                            Row::builder()
                                .align(Align::End)
                                .children(vec![button(
                                    "error-close",
                                    "Dismiss",
                                    "Elevated",
                                    "Primary",
                                    None,
                                    true,
                                    false,
                                )])
                                .build(),
                        ),
                    ])
                    .build(),
            )])
            .build(),
    ))
}
