//! Host-side registry for the plugin **datasets** channel — the pull half of
//! the plugin data ecosystem (#118).
//!
//! A producer plugin publishes a tabular `Dataset` (typed columns + string
//! cells for v1) through the `datasets` WIT import; the host stores it here and
//! the Datasets sidebar panel browses/previews it. Consumer plugins (#114) read
//! it paged. The registry LRU-evicts old datasets and drops a producer's
//! datasets when its instance closes (reconciled each frame, like signals).
//!
//! The row payload is intentionally a `Vec<Vec<String>>` (row-major strings) so
//! v1 stays simple; the seam is designed to swap to Apache Arrow IPC later
//! without changing the public shape.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

/// Most datasets kept before LRU eviction of the least-recently-accessed.
const MAX_DATASETS: usize = 32;
/// Aggregate memory budget across all stored datasets. The least-recently-
/// accessed are evicted until the total fits (a single dataset larger than
/// this is still kept — we can't do better than one).
const MAX_BYTES: usize = 128 * 1024 * 1024;
/// Hard cap on rows returned by a single `read`, so a huge dataset never
/// crosses the boundary at once.
pub const MAX_READ_LIMIT: u32 = 1000;

#[derive(Clone, Debug)]
pub struct DatasetColumn {
    pub name: String,
    pub type_hint: String,
}

/// Registry metadata for a published dataset (no rows).
#[derive(Clone, Debug)]
pub struct DatasetMeta {
    pub id: String,
    pub name: String,
    pub source_plugin: String,
    /// Producer instance id, used to drop datasets when the producer closes.
    pub source_instance: String,
    pub kind: String,
    pub tags: Vec<String>,
    pub row_count: u64,
    pub columns: Vec<DatasetColumn>,
}

/// A contiguous page of rows.
#[derive(Clone, Debug)]
pub struct Page {
    pub columns: Vec<DatasetColumn>,
    pub rows: Vec<Vec<String>>,
    pub offset: u64,
    pub total: u64,
}

struct Stored {
    meta: DatasetMeta,
    rows: Vec<Vec<String>>,
    last_access: Instant,
    /// Estimated heap footprint of this dataset, tracked so the registry can
    /// enforce [`MAX_BYTES`] without re-summing every entry.
    size: usize,
}

#[derive(Default)]
struct Registry {
    map: HashMap<String, Stored>,
    /// Publish order for stable listing.
    order: Vec<String>,
    /// Running sum of every `Stored::size`, kept in step via [`Registry::drop_dataset`].
    bytes: usize,
    seq: u64,
}

impl Registry {
    /// Remove a dataset by id, keeping `order` and the `bytes` total in step.
    fn drop_dataset(&mut self, id: &str) {
        if let Some(s) = self.map.remove(id) {
            self.bytes = self.bytes.saturating_sub(s.size);
        }
        self.order.retain(|o| o != id);
    }
}

/// Estimated heap footprint of a dataset's rows + metadata strings.
fn dataset_bytes(meta: &DatasetMeta, rows: &[Vec<String>]) -> usize {
    let cells: usize = rows
        .iter()
        .map(|r| {
            std::mem::size_of::<Vec<String>>()
                + r.iter()
                    .map(|c| std::mem::size_of::<String>() + c.len())
                    .sum::<usize>()
        })
        .sum();
    let cols: usize = meta
        .columns
        .iter()
        .map(|c| c.name.len() + c.type_hint.len())
        .sum();
    let tags: usize = meta
        .tags
        .iter()
        .map(|t| std::mem::size_of::<String>() + t.len())
        .sum();
    cells + cols + tags + meta.name.len() + meta.source_plugin.len() + meta.source_instance.len()
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));

/// Store a dataset published by `source_plugin` (instance `source_instance`),
/// returning its assigned id. Evicts the least-recently-accessed dataset when
/// the registry is full.
#[allow(clippy::too_many_arguments)]
pub fn publish(
    source_plugin: &str,
    source_instance: &str,
    name: String,
    kind: String,
    tags: Vec<String>,
    columns: Vec<DatasetColumn>,
    rows: Vec<Vec<String>>,
) -> String {
    let Ok(mut reg) = REGISTRY.lock() else {
        return String::new();
    };
    reg.seq += 1;
    let id = format!("ds-{}", reg.seq);
    let meta = DatasetMeta {
        id: id.clone(),
        name,
        source_plugin: source_plugin.to_string(),
        source_instance: source_instance.to_string(),
        kind,
        tags,
        row_count: rows.len() as u64,
        columns,
    };
    let size = dataset_bytes(&meta, &rows);
    reg.bytes += size;
    reg.map.insert(
        id.clone(),
        Stored {
            meta,
            rows,
            last_access: Instant::now(),
            size,
        },
    );
    reg.order.push(id.clone());

    // LRU eviction while over either the count or the byte budget. Keep at
    // least the just-published dataset (guard on len > 1) so a single
    // oversized dataset doesn't loop forever.
    while reg.order.len() > 1 && (reg.order.len() > MAX_DATASETS || reg.bytes > MAX_BYTES) {
        // Least-recently-accessed still-present id (never the new one — it's
        // the most recently accessed).
        let Some(victim) = reg
            .order
            .iter()
            .min_by_key(|id| reg.map.get(*id).map(|s| s.last_access))
            .cloned()
        else {
            break;
        };
        reg.drop_dataset(&victim);
    }
    id
}

/// Metadata for all published datasets, in publish order.
pub fn list() -> Vec<DatasetMeta> {
    let Ok(reg) = REGISTRY.lock() else {
        return Vec::new();
    };
    reg.order
        .iter()
        .filter_map(|id| reg.map.get(id).map(|s| s.meta.clone()))
        .collect()
}

/// Read rows `[offset, offset + limit)` of dataset `id`; `limit` is capped by
/// [`MAX_READ_LIMIT`]. Returns `None` if the id is unknown.
pub fn read(id: &str, offset: u64, limit: u32) -> Option<Page> {
    let Ok(mut reg) = REGISTRY.lock() else {
        return None;
    };
    let stored = reg.map.get_mut(id)?;
    stored.last_access = Instant::now();
    let total = stored.rows.len() as u64;
    let start = offset.min(total) as usize;
    let capped = limit.min(MAX_READ_LIMIT) as u64;
    let end = (offset.saturating_add(capped)).min(total) as usize;
    Some(Page {
        columns: stored.meta.columns.clone(),
        rows: stored.rows[start..end].to_vec(),
        offset: start as u64,
        total,
    })
}

/// Remove a dataset (idempotent).
pub fn release(id: &str) {
    if let Ok(mut reg) = REGISTRY.lock() {
        reg.drop_dataset(id);
    }
}

/// Drop datasets whose producing instance is no longer open. Called each frame
/// with the set of live plugin-instance ids (same set signals uses).
pub fn retain_instances(open: &std::collections::HashSet<String>) {
    if let Ok(mut reg) = REGISTRY.lock() {
        let dropped: Vec<String> = reg
            .map
            .values()
            .filter(|s| !open.contains(&s.meta.source_instance))
            .map(|s| s.meta.id.clone())
            .collect();
        for id in dropped {
            reg.drop_dataset(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(mut reg) = REGISTRY.lock() {
            reg.map.clear();
            reg.order.clear();
            reg.bytes = 0;
            reg.seq = 0;
        }
        guard
    }

    fn col(name: &str) -> DatasetColumn {
        DatasetColumn {
            name: name.to_string(),
            type_hint: "text".to_string(),
        }
    }

    #[test]
    fn publish_list_read_paged() {
        let _g = reset();
        let rows: Vec<Vec<String>> = (0..10)
            .map(|i| vec![i.to_string(), format!("n{i}")])
            .collect();
        let id = publish(
            "com.thoth.seshat",
            "seshat#1",
            "orders".into(),
            "sql-result".into(),
            vec!["db".into()],
            vec![col("id"), col("name")],
            rows,
        );
        let metas = list();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].name, "orders");
        assert_eq!(metas[0].row_count, 10);

        let page = read(&id, 3, 4).unwrap();
        assert_eq!(page.total, 10);
        assert_eq!(page.offset, 3);
        assert_eq!(page.rows.len(), 4);
        assert_eq!(page.rows[0][0], "3");
    }

    #[test]
    fn retain_drops_closed_producers() {
        let _g = reset();
        publish(
            "p",
            "p#1",
            "a".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![],
        );
        publish(
            "p",
            "p#2",
            "b".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![],
        );
        let open = std::collections::HashSet::from(["p#2".to_string()]);
        retain_instances(&open);
        let metas = list();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].name, "b");
    }

    #[test]
    fn byte_budget_evicts_lru() {
        let _g = reset();
        // Each row alone is the whole budget, so publishing a second one forces
        // eviction of the older (least-recently-accessed) dataset.
        let big_row = || vec![vec!["x".repeat(MAX_BYTES)]];

        publish(
            "p",
            "p#1",
            "first".into(),
            "k".into(),
            vec![],
            vec![col("v")],
            big_row(),
        );
        publish(
            "p",
            "p#2",
            "second".into(),
            "k".into(),
            vec![],
            vec![col("v")],
            big_row(),
        );

        let metas = list();
        assert_eq!(metas.len(), 1, "over budget → only the survivor remains");
        assert_eq!(metas[0].name, "second");
    }
}
