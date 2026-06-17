#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

/// Visual scale of a [`Typography`] run — maps to a size, weight, and default
/// theme colour role.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TypographyVariant {
    /// Sidebar panel section titles: 11px · semi-bold · `sidebar_header`.
    PanelHeader,
    /// Content-area section divider labels: 12px · bold · `fg`.
    SectionHeader,
    /// Settings group card labels: 11px · semi-bold · `fg_muted`.
    GroupLabel,
    /// Dialog / window titles: 14px · bold · `fg`.
    Title,
    /// Section headings and card titles: 16px · bold · `fg`.
    Heading,
    /// Large body — setting row labels, toolbar labels: 13px · `fg`.
    BodyLarge,
    /// Standard body copy: 12px · `fg`.
    #[default]
    Body,
    /// Secondary / muted body: 12px · `fg_muted`.
    BodyMuted,
    /// Subtitle under a heading: 13px · `fg_muted`.
    Subtitle,
    /// Small metadata, hints, counts: 11px · `fg_muted`.
    Caption,
    /// Tiny badge / inline tag text: 10px · `fg_muted`.
    Label,
    /// Monospace code / path text: 12px · monospace · `fg`.
    Mono,
}

/// A styled run of text.
///
/// The [`variant`](Typography::variant) sets the size, weight, and default
/// colour (resolved against the active theme); the remaining fields layer
/// optional overrides on top.
///
/// ```
/// use thoth_plugin_sdk::components::{Typography, TypographyVariant};
///
/// let heading = Typography::builder()
///     .text("Results")
///     .variant(TypographyVariant::Heading)
///     .build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct Typography {
    /// The text to render.
    pub text: String,
    /// Visual scale. Defaults to [`TypographyVariant::Body`].
    #[builder(default)]
    #[serde(default)]
    pub variant: TypographyVariant,
    /// Colour override as a `#rrggbb` hex string. When unset, the variant's
    /// default theme colour is used.
    #[serde(default)]
    pub color: Option<String>,
    /// Font-size override in points — if unset, derived from `variant`.
    #[serde(default)]
    pub size: Option<f32>,
    /// Apply bold weight on top of the variant's default weight.
    #[builder(default)]
    #[serde(default)]
    pub bold: bool,
    /// Apply italic style.
    #[builder(default)]
    #[serde(default)]
    pub italic: bool,
    /// Apply underline decoration.
    #[builder(default)]
    #[serde(default)]
    pub underline: bool,
}
