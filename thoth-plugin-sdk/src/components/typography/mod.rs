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
#[non_exhaustive]
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

#[cfg(test)]
mod tests {
    use super::{Typography, TypographyVariant};
    use serde_json::Value;

    // ── builder defaults ──────────────────────────────────────────────────────

    #[test]
    fn builder_requires_only_text() {
        let t = Typography::builder().text("hello").build();
        assert_eq!(t.text, "hello");
        assert_eq!(t.variant, TypographyVariant::Body);
        assert!(t.color.is_none());
        assert!(t.size.is_none());
        assert!(!t.bold);
        assert!(!t.italic);
        assert!(!t.underline);
    }

    #[test]
    fn builder_sets_all_fields() {
        let t = Typography::builder()
            .text("Title")
            .variant(TypographyVariant::Heading)
            .color("#ff0000")
            .size(16.0)
            .bold(true)
            .italic(true)
            .underline(true)
            .build();
        assert_eq!(t.text, "Title");
        assert_eq!(t.variant, TypographyVariant::Heading);
        assert_eq!(t.color.as_deref(), Some("#ff0000"));
        assert_eq!(t.size, Some(16.0));
        assert!(t.bold);
        assert!(t.italic);
        assert!(t.underline);
    }

    // ── TypographyVariant serde (kebab-case) ──────────────────────────────────

    #[test]
    fn variant_body_serialises_as_kebab_case() {
        let s = serde_json::to_string(&TypographyVariant::Body).unwrap();
        assert_eq!(s, r#""body""#);
    }

    #[test]
    fn variant_panel_header_serialises_as_kebab_case() {
        let s = serde_json::to_string(&TypographyVariant::PanelHeader).unwrap();
        assert_eq!(s, r#""panel-header""#);
    }

    #[test]
    fn variant_body_muted_serialises_as_kebab_case() {
        let s = serde_json::to_string(&TypographyVariant::BodyMuted).unwrap();
        assert_eq!(s, r#""body-muted""#);
    }

    #[test]
    fn variant_caption_serialises() {
        let s = serde_json::to_string(&TypographyVariant::Caption).unwrap();
        assert_eq!(s, r#""caption""#);
    }

    #[test]
    fn all_variants_round_trip_through_json() {
        for variant in [
            TypographyVariant::PanelHeader,
            TypographyVariant::SectionHeader,
            TypographyVariant::GroupLabel,
            TypographyVariant::Title,
            TypographyVariant::Heading,
            TypographyVariant::BodyLarge,
            TypographyVariant::Body,
            TypographyVariant::BodyMuted,
            TypographyVariant::Subtitle,
            TypographyVariant::Caption,
            TypographyVariant::Label,
            TypographyVariant::Mono,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let restored: TypographyVariant = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, variant, "round-trip failed for {:?}", variant);
        }
    }

    // ── Typography serialisation ──────────────────────────────────────────────

    #[test]
    fn typography_serialises_text_field() {
        let t = Typography::builder().text("hi").build();
        let v: Value = serde_json::to_value(&t).unwrap();
        assert_eq!(v["text"], "hi");
    }

    #[test]
    fn typography_with_defaults_serialises_false_bool_fields() {
        let t = Typography::builder().text("x").build();
        let v: Value = serde_json::to_value(&t).unwrap();
        assert_eq!(v["bold"], false);
        assert_eq!(v["italic"], false);
        assert_eq!(v["underline"], false);
    }

    #[test]
    fn typography_round_trips_with_variant() {
        let original = Typography::builder()
            .text("section")
            .variant(TypographyVariant::Caption)
            .bold(true)
            .build();
        let json = serde_json::to_string(&original).unwrap();
        let restored: Typography = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.text, "section");
        assert_eq!(restored.variant, TypographyVariant::Caption);
        assert!(restored.bold);
    }
}
