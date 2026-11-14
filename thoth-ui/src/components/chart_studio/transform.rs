//! Data shaping for Chart Studio: aggregation (group by X), sorting, and
//! top-N with an "Other" bucket. Produces render-ready columns + string rows.

use super::{Aggregation, SortMode};

/// Render-ready shaped data.
pub struct Shaped {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub x_col: usize,
    pub y_cols: Vec<usize>,
}

fn cell(rows: &[Vec<String>], r: usize, c: usize) -> Option<f64> {
    rows.get(r)?.get(c)?.trim().parse::<f64>().ok()
}

/// Aggregate one source Y column over a set of member rows.
fn aggregate(agg: Aggregation, rows: &[Vec<String>], members: &[usize], y_col: usize) -> f64 {
    let vals: Vec<f64> = members
        .iter()
        .filter_map(|&r| cell(rows, r, y_col))
        .collect();
    match agg {
        Aggregation::Count => members.len() as f64,
        Aggregation::Sum => vals.iter().sum(),
        Aggregation::Average => {
            if vals.is_empty() {
                0.0
            } else {
                vals.iter().sum::<f64>() / vals.len() as f64
            }
        }
        Aggregation::Min => vals.iter().cloned().fold(f64::INFINITY, f64::min),
        Aggregation::Max => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        Aggregation::None => vals.first().copied().unwrap_or(0.0),
    }
}

/// Compact number → string that still round-trips through `parse::<f64>()`.
fn num_str(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 {
        format!("{}", v as i64)
    } else if v.is_finite() {
        format!("{v:.2}")
    } else {
        "0".to_string()
    }
}

/// Order rows by the given value/label columns.
fn sort_rows(rows: &mut [Vec<String>], value_col: usize, label_col: usize, sort: SortMode) {
    match sort {
        SortMode::None => {}
        SortMode::ValueDesc | SortMode::ValueAsc => {
            rows.sort_by(|a, b| {
                let va = a.get(value_col).and_then(|s| s.trim().parse::<f64>().ok());
                let vb = b.get(value_col).and_then(|s| s.trim().parse::<f64>().ok());
                let ord = va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal);
                if matches!(sort, SortMode::ValueDesc) {
                    ord.reverse()
                } else {
                    ord
                }
            });
        }
        SortMode::LabelAsc | SortMode::LabelDesc => {
            rows.sort_by(|a, b| {
                let ord = a
                    .get(label_col)
                    .map(String::as_str)
                    .unwrap_or("")
                    .cmp(b.get(label_col).map(String::as_str).unwrap_or(""));
                if matches!(sort, SortMode::LabelDesc) {
                    ord.reverse()
                } else {
                    ord
                }
            });
        }
    }
}

/// Shape raw source data into render-ready form.
pub fn shape(
    columns: &[String],
    rows: &[Vec<String>],
    x_col: usize,
    y_cols: &[usize],
    agg: Aggregation,
    top_n: usize,
    sort: SortMode,
) -> Shaped {
    if agg == Aggregation::None {
        // Pass-through: keep every column; just sort + truncate.
        let mut out: Vec<Vec<String>> = rows.to_vec();
        let value_col = y_cols.first().copied().unwrap_or(x_col);
        sort_rows(&mut out, value_col, x_col, sort);
        if top_n > 0 {
            out.truncate(top_n);
        }
        return Shaped {
            columns: columns.to_vec(),
            rows: out,
            x_col,
            y_cols: y_cols.to_vec(),
        };
    }

    // Group member-row indices by the X value, first-seen order.
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (r, row) in rows.iter().enumerate() {
        let key = row.get(x_col).cloned().unwrap_or_default();
        groups.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Vec::new()
        });
        if let Some(m) = groups.get_mut(&key) {
            m.push(r);
        }
    }

    // Output columns: X, then one column per aggregated series (Count → a
    // single "count" column).
    let x_name = columns.get(x_col).cloned().unwrap_or_default();
    let series: Vec<usize> = if agg == Aggregation::Count {
        vec![]
    } else {
        y_cols.to_vec()
    };
    let mut out_columns = vec![x_name];
    if agg == Aggregation::Count {
        out_columns.push("count".to_string());
    } else {
        for &c in &series {
            out_columns.push(columns.get(c).cloned().unwrap_or_default());
        }
    }

    let value_count = if agg == Aggregation::Count {
        1
    } else {
        series.len()
    };
    let out_y_cols: Vec<usize> = (1..=value_count).collect();

    // One row per group.
    let group_row = |members: &[usize], label: String| -> Vec<String> {
        let mut row = vec![label];
        if agg == Aggregation::Count {
            row.push(num_str(members.len() as f64));
        } else {
            for &c in &series {
                row.push(num_str(aggregate(agg, rows, members, c)));
            }
        }
        row
    };

    let mut out: Vec<Vec<String>> = order
        .iter()
        .map(|k| group_row(&groups[k], k.clone()))
        .collect();

    // Sort by the first value column (index 1) or the label (index 0).
    sort_rows(&mut out, 1, 0, sort);

    // Top-N with an "Other" bucket that re-aggregates the remaining groups'
    // member rows (correct for every aggregation, unlike summing partials).
    if top_n > 0 && out.len() > top_n {
        let kept_labels: std::collections::HashSet<String> =
            out.iter().take(top_n).map(|r| r[0].clone()).collect();
        let mut other_members: Vec<usize> = Vec::new();
        for (k, members) in &groups {
            if !kept_labels.contains(k) {
                other_members.extend(members.iter().copied());
            }
        }
        out.truncate(top_n);
        if !other_members.is_empty() {
            out.push(group_row(&other_members, "Other".to_string()));
        }
    }

    Shaped {
        columns: out_columns,
        rows: out,
        x_col: 0,
        y_cols: out_y_cols,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Vec<String>, Vec<Vec<String>>) {
        let cols = vec!["provider".to_string(), "count".to_string()];
        let rows = vec![
            vec!["github".into(), "5".into()],
            vec!["gitlab".into(), "2".into()],
            vec!["github".into(), "3".into()],
            vec!["bitbucket".into(), "1".into()],
        ];
        (cols, rows)
    }

    fn val(rows: &[Vec<String>], label: &str) -> Option<f64> {
        rows.iter()
            .find(|r| r[0] == label)
            .and_then(|r| r[1].parse().ok())
    }

    #[test]
    fn sum_groups_by_x() {
        let (c, r) = sample();
        let s = shape(&c, &r, 0, &[1], Aggregation::Sum, 0, SortMode::None);
        assert_eq!(s.x_col, 0);
        assert_eq!(s.y_cols, vec![1]);
        assert_eq!(s.rows.len(), 3); // github, gitlab, bitbucket
        assert_eq!(val(&s.rows, "github"), Some(8.0));
        assert_eq!(val(&s.rows, "gitlab"), Some(2.0));
    }

    #[test]
    fn count_ignores_value_column() {
        let (c, r) = sample();
        let s = shape(&c, &r, 0, &[1], Aggregation::Count, 0, SortMode::None);
        assert_eq!(s.columns[1], "count");
        assert_eq!(val(&s.rows, "github"), Some(2.0)); // two github rows
        assert_eq!(val(&s.rows, "bitbucket"), Some(1.0));
    }

    #[test]
    fn top_n_buckets_remainder_into_other() {
        let (c, r) = sample();
        let s = shape(&c, &r, 0, &[1], Aggregation::Sum, 2, SortMode::ValueDesc);
        // github(8), gitlab(2), then Other = bitbucket(1).
        assert_eq!(s.rows.len(), 3);
        assert_eq!(s.rows[0][0], "github");
        assert_eq!(val(&s.rows, "Other"), Some(1.0));
    }

    #[test]
    fn none_passthrough_sorts_and_truncates() {
        let (c, r) = sample();
        let s = shape(&c, &r, 0, &[1], Aggregation::None, 2, SortMode::ValueDesc);
        assert_eq!(s.rows.len(), 2);
        assert_eq!(s.rows[0][1], "5"); // highest raw value first
        assert_eq!(s.columns, c); // all columns kept
    }
}
