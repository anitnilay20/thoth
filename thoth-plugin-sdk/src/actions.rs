//! Reserved, host-interpreted UI action ids.
//!
//! Most plugin button events are forwarded verbatim to the plugin's own
//! `handle_event`. A small set of well-known [`widget ids`](Self) are instead
//! intercepted by the host as *system actions*: when a plugin emits a
//! `Button`/`IconButton` whose id matches one of these, clicking it drives a
//! host capability (e.g. opening Chart Studio) rather than round-tripping back
//! to the plugin. Reusing these constants keeps the convention typo-proof on
//! both sides.

/// Open Chart Studio bound to the emitting plugin's own tab as the data source.
///
/// The plugin must be a data producer (declare the `data-producer` capability
/// and implement `data-producer.provide-dataset`) for the resulting chart to
/// have data to draw.
pub const OPEN_IN_CHARTS: &str = "thoth:open-in-charts";

/// Export a host-owned dataset through an exporter plugin. Emitted by a
/// [`DataView`](crate::components::DataView)'s Export dropdown; the event value
/// is `{"handle": "<dataset handle>", "exporter": "<plugin id>"}`. The host
/// reads the rows, runs the chosen exporter, and saves the file.
pub const EXPORT_DATASET: &str = "thoth:export-dataset";

/// Point a [`DataView`](crate::components::DataView) at another of the tables
/// its document holds. Emitted by the view's table picker; the event value is
/// the chosen [`DataTable::value`](crate::components::DataTable::value).
///
/// The producer owns what switching costs — a collection may have to be read
/// before it can be shown — so the picker only ever reports the choice and
/// leaves the work, and the new `selected_table`, to whoever built the node.
pub const SELECT_TABLE: &str = "thoth:select-table";
