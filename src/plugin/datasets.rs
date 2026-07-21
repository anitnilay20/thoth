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
}

#[derive(Default)]
struct Registry {
    map: HashMap<String, Stored>,
    /// Publish order for stable listing.
    order: Vec<String>,
    seq: u64,
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
    reg.map.insert(
        id.clone(),
        Stored {
            meta,
            rows,
            last_access: Instant::now(),
        },
    );
    reg.order.push(id.clone());

    // LRU eviction when over the cap.
    while reg.order.len() > MAX_DATASETS {
        // Find the least-recently-accessed still-present id.
        if let Some(victim) = reg
            .order
            .iter()
            .min_by_key(|id| reg.map.get(*id).map(|s| s.last_access))
            .cloned()
        {
            reg.map.remove(&victim);
            reg.order.retain(|o| o != &victim);
        } else {
            break;
        }
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
        reg.map.remove(id);
        reg.order.retain(|o| o != id);
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
            reg.map.remove(&id);
            reg.order.retain(|o| o != &id);
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
}
