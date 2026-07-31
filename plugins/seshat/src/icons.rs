//! Engine logos embedded in the wasm, shown as tiles in the connection picker.
//!
//! These are simple placeholders — swap them for real brand logos by replacing
//! the PNGs under `assets/icons/` (the host decodes the bytes via egui's image
//! loaders, so no filesystem access is involved).

use crate::db::Engine;

const POSTGRES: &[u8] = include_bytes!("../assets/icons/postgres.png");
const MYSQL: &[u8] = include_bytes!("../assets/icons/mysql.png");
const ELASTICSEARCH: &[u8] = include_bytes!("../assets/icons/elasticsearch.png");

/// The logo `(cache-uri, bytes)` for an engine — feeds a `ListItemPrefix::Image`.
pub(crate) fn engine_logo(e: Engine) -> (&'static str, &'static [u8]) {
    match e {
        Engine::Postgres => ("bytes://seshat/postgres", POSTGRES),
        Engine::Mysql => ("bytes://seshat/mysql", MYSQL),
        Engine::Elasticsearch => ("bytes://seshat/elasticsearch", ELASTICSEARCH),
    }
}
