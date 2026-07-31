//! Host-side registry for the plugin **datasets** channel — the host's single
//! owned copy of tabular data (part of the plugin data ecosystem, #118).
//!
//! A producer publishes a dataset (typed columns + string cells for v1) via the
//! `dataset-bus` WIT import and gets back a handle; it embeds that handle in a
//! `data-view` render node, and the host draws the data itself (#114) — reading
//! rows here by handle so they never re-enter the plugin. The registry
//! LRU-evicts old datasets, replaces an instance's dataset on re-`publish`, and
//! drops a producer's datasets when its instance closes (reconciled each frame,
//! like signals).
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

    /// Evict the least-recently-accessed datasets while over either the count or
    /// the byte budget. Keeps at least one dataset (guard on `len > 1`) so a
    /// single oversized dataset can't loop forever.
    fn enforce_budget(&mut self) {
        while self.order.len() > 1 && (self.order.len() > MAX_DATASETS || self.bytes > MAX_BYTES) {
            let Some(victim) = self
                .order
                .iter()
                .min_by_key(|id| self.map.get(*id).map(|s| s.last_access))
                .cloned()
            else {
                break;
            };
            self.drop_dataset(&victim);
        }
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
    // A fresh publish from an instance replaces that instance's previous
    // dataset — dropping the old rows immediately rather than waiting for LRU
    // or tab close. A producer that wants to keep the same dataset live should
    // call `update` (same handle) instead of re-publishing.
    let stale: Vec<String> = reg
        .map
        .values()
        .filter(|s| s.meta.source_instance == source_instance)
        .map(|s| s.meta.id.clone())
        .collect();
    for id in stale {
        reg.drop_dataset(&id);
    }
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

    // Evict down to the count/byte budget (never the just-published dataset —
    // it's the most recently accessed).
    reg.enforce_budget();
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

/// Replace the columns + rows behind an existing handle in place (keeping its
/// id, source, and byte-budget accounting current). No-op unless the handle is
/// known and owned by `instance` — a producer can only mutate its own datasets.
pub fn update(instance: &str, id: &str, columns: Vec<DatasetColumn>, rows: Vec<Vec<String>>) {
    if let Ok(mut reg) = REGISTRY.lock() {
        let Some(stored) = reg.map.get(id) else {
            return;
        };
        if stored.meta.source_instance != instance {
            return;
        }
        let meta = DatasetMeta {
            row_count: rows.len() as u64,
            columns,
            ..stored.meta.clone()
        };
        let size = dataset_bytes(&meta, &rows);
        let old_size = stored.size;
        if let Some(stored) = reg.map.get_mut(id) {
            stored.meta = meta;
            stored.rows = rows;
            stored.size = size;
            stored.last_access = Instant::now();
        }
        reg.bytes = reg.bytes.saturating_add(size).saturating_sub(old_size);
        // A larger dataset may push us over budget — evict to fit.
        reg.enforce_budget();
    }
}

/// Remove a dataset (idempotent). No-op unless the handle is owned by
/// `instance`, so a producer can only release its own datasets.
pub fn release(instance: &str, id: &str) {
    if let Ok(mut reg) = REGISTRY.lock()
        && reg
            .map
            .get(id)
            .is_some_and(|s| s.meta.source_instance == instance)
    {
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
    fn publish_replaces_same_instance() {
        let _g = reset();
        let first = publish(
            "p",
            "p#1",
            "a".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![],
        );
        let second = publish(
            "p",
            "p#1",
            "b".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![],
        );
        // The fresh publish from p#1 dropped its previous dataset.
        let metas = list();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].name, "b");
        assert!(read(&first, 0, 10).is_none());
        assert!(read(&second, 0, 10).is_some());
    }

    #[test]
    fn update_replaces_rows_in_place() {
        let _g = reset();
        let id = publish(
            "p",
            "p#1",
            "a".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![vec!["1".into()]],
        );
        update(
            "p#1",
            &id,
            vec![col("x")],
            vec![vec!["1".into()], vec!["2".into()], vec!["3".into()]],
        );
        let page = read(&id, 0, 10).unwrap();
        assert_eq!(page.total, 3); // same handle, new rows
        assert_eq!(list().len(), 1);

        // A different instance can't mutate this dataset.
        update("other#1", &id, vec![col("x")], vec![]);
        assert_eq!(read(&id, 0, 10).unwrap().total, 3);
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

    fn publish_small(instance: &str, name: &str) -> String {
        publish(
            "p",
            instance,
            name.into(),
            "k".into(),
            vec![],
            vec![col("v")],
            vec![vec!["1".into()]],
        )
    }

    #[test]
    fn release_removes_dataset() {
        let _g = reset();
        let id = publish_small("p#1", "a");
        assert_eq!(list().len(), 1);
        // A different instance can't release it.
        release("other#1", &id);
        assert_eq!(list().len(), 1);
        release("p#1", &id);
        assert!(list().is_empty());
        assert!(read(&id, 0, 1).is_none());
        // Idempotent.
        release("p#1", &id);
    }

    #[test]
    fn count_cap_holds_at_max() {
        let _g = reset();
        for i in 0..(MAX_DATASETS + 5) {
            publish_small(&format!("p#{i}"), &format!("d{i}"));
        }
        assert_eq!(list().len(), MAX_DATASETS, "count cap enforced");
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
