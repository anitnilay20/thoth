//! Intelligent JSON analysis, schema inference, anomaly detection,
//! relationship mapping, and natural-language to SQL translation (issue #15).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// Per-field statistical summary.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldStats {
    pub path: String,
    pub total: usize,
    pub non_null: usize,
    pub distinct_count: usize,
    pub observed_types: Vec<String>,
    pub min_num: Option<f64>,
    pub max_num: Option<f64>,
    pub mean_num: Option<f64>,
}

/// Detected anomaly in sampled JSON records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anomaly {
    pub field: String,
    pub kind: String,
    pub detail: String,
}

/// Inferred relationship (e.g. `*_id` foreign-key candidate or nested entity).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub field: String,
    pub target_entity: String,
    pub confidence: String,
}

/// Complete analysis report for a JSON array or NDJSON sample.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonInsightsReport {
    pub record_count: usize,
    pub schema: Value,
    pub fields: Vec<FieldStats>,
    pub anomalies: Vec<Anomaly>,
    pub relationships: Vec<Relationship>,
}

/// Analyze a slice of JSON records into a schema, field stats, anomalies, and relationships.
pub fn analyze_records(records: &[Value]) -> JsonInsightsReport {
    let schema = infer_json_schema(records);
    let mut acc: BTreeMap<String, FieldAccum> = BTreeMap::new();

    for rec in records {
        if let Some(obj) = rec.as_object() {
            for (k, v) in obj {
                let entry = acc.entry(k.clone()).or_default();
                entry.observe(v);
            }
        }
    }

    let total = records.len();
    let mut fields = Vec::with_capacity(acc.len());
    let mut anomalies = Vec::new();
    let mut relationships = Vec::new();

    for (path, a) in acc {
        let mean_num = if a.numbers.is_empty() {
            None
        } else {
            Some(a.numbers.iter().sum::<f64>() / a.numbers.len() as f64)
        };
        let min_num = a.numbers.iter().copied().reduce(f64::min);
        let max_num = a.numbers.iter().copied().reduce(f64::max);

        if a.types.len() > 1 {
            anomalies.push(Anomaly {
                field: path.clone(),
                kind: "mixed_types".to_string(),
                detail: format!(
                    "Observed {} types: {}",
                    a.types.len(),
                    a.types.iter().cloned().collect::<Vec<_>>().join(", ")
                ),
            });
        }

        if let Some(mean) = mean_num {
            if a.numbers.len() >= 4 {
                let var = a
                    .numbers
                    .iter()
                    .map(|x| (x - mean).powi(2))
                    .sum::<f64>()
                    / a.numbers.len() as f64;
                let std = var.sqrt();
                if std > 0.0 {
                    for &x in &a.numbers {
                        if (x - mean).abs() / std > 3.0 {
                            anomalies.push(Anomaly {
                                field: path.clone(),
                                kind: "numeric_outlier".to_string(),
                                detail: format!("Value {x} is >3σ from mean {mean:.2}"),
                            });
                            break;
                        }
                    }
                }
            }
        }

        if let Some(target) = path.strip_suffix("_id").filter(|s| !s.is_empty()) {
            relationships.push(Relationship {
                field: path.clone(),
                target_entity: target.to_string(),
                confidence: "high".to_string(),
            });
        } else if path.ends_with("Id") && path.len() > 2 {
            let target = &path[..path.len() - 2];
            relationships.push(Relationship {
                field: path.clone(),
                target_entity: target.to_string(),
                confidence: "medium".to_string(),
            });
        }

        fields.push(FieldStats {
            path,
            total,
            non_null: a.non_null,
            distinct_count: a.distinct.len(),
            observed_types: a.types.into_iter().collect(),
            min_num,
            max_num,
            mean_num,
        });
    }

    JsonInsightsReport {
        record_count: records.len(),
        schema,
        fields,
        anomalies,
        relationships,
    }
}

#[derive(Default)]
struct FieldAccum {
    non_null: usize,
    types: BTreeSet<String>,
    distinct: BTreeSet<String>,
    numbers: Vec<f64>,
}

impl FieldAccum {
    fn observe(&mut self, v: &Value) {
        if !v.is_null() {
            self.non_null += 1;
        }
        self.types.insert(json_type(v).to_string());
        if self.distinct.len() < 1000 {
            self.distinct.insert(v.to_string());
        }
        if let Some(n) = v.as_f64() {
            self.numbers.push(n);
        }
    }
}

fn json_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Infer a JSON Schema (Draft 2020-12) from a sample of JSON records.
pub fn infer_json_schema(records: &[Value]) -> Value {
    let mut properties = Map::new();
    let mut field_types: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut field_counts: BTreeMap<String, usize> = BTreeMap::new();

    for rec in records {
        if let Some(obj) = rec.as_object() {
            for (k, v) in obj {
                *field_counts.entry(k.clone()).or_insert(0) += 1;
                field_types
                    .entry(k.clone())
                    .or_default()
                    .insert(json_type(v).to_string());
            }
        }
    }

    let mut required = Vec::new();
    let total = records.len();
    for (k, types) in field_types {
        if total > 0 && field_counts.get(&k).copied().unwrap_or(0) == total && !types.contains("null")
        {
            required.push(Value::String(k.clone()));
        }
        let non_null: Vec<String> = types.into_iter().filter(|t| t != "null").collect();
        let schema_ty = if non_null.len() == 1 {
            Value::String(non_null[0].clone())
        } else if non_null.is_empty() {
            Value::String("null".to_string())
        } else {
            Value::Array(non_null.into_iter().map(Value::String).collect())
        };
        properties.insert(k, json!({ "type": schema_ty }));
    }

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

/// Translate a natural-language question (e.g. `"show me all users from California"`,
/// `"count where total > 100"`, `"top 10 by score"`) into an executable DuckDB SQL query.
pub fn nl_to_sql(table: &str, question: &str, known_columns: &[String]) -> String {
    let q = question.trim();
    let lower = q.to_lowercase();
    let quoted_table = format!("\"{}\"", table.replace('"', "\"\""));

    let mut limit = 100usize;
    let words: Vec<&str> = q.split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        if (w.eq_ignore_ascii_case("top") || w.eq_ignore_ascii_case("limit"))
            && i + 1 < words.len()
        {
            if let Ok(n) = words[i + 1].parse::<usize>() {
                limit = n;
            }
        }
    }

    // Count intent
    if lower.starts_with("count") || lower.contains("how many") {
        if let Some(cond) = extract_where_clause(q, known_columns) {
            return format!("SELECT COUNT(*) AS count FROM {quoted_table} WHERE {cond}");
        }
        return format!("SELECT COUNT(*) AS count FROM {quoted_table}");
    }

    // "from <Value>" pattern (e.g. "show me all users from California")
    if let Some(idx) = lower.find(" from ") {
        let val = q[idx + 6..]
            .trim()
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != ' ')
            .trim();
        if !val.is_empty() {
            let loc_col = known_columns
                .iter()
                .find(|c| {
                    let cl = c.to_lowercase();
                    cl == "state" || cl == "city" || cl == "country" || cl == "region" || cl == "location"
                })
                .map(|s| s.as_str());
            if let Some(col) = loc_col {
                let escaped = val.replace('\'', "''");
                return format!(
                    "SELECT * FROM {quoted_table} WHERE \"{}\" = '{escaped}' LIMIT {limit}",
                    col.replace('"', "\"\"")
                );
            }
        }
    }

    if let Some(cond) = extract_where_clause(q, known_columns) {
        return format!("SELECT * FROM {quoted_table} WHERE {cond} LIMIT {limit}");
    }

    format!("SELECT * FROM {quoted_table} LIMIT {limit}")
}

fn extract_where_clause(q: &str, known_columns: &[String]) -> Option<String> {
    let tokens: Vec<&str> = q.split_whitespace().collect();
    for i in 0..tokens.len() {
        let col = known_columns
            .iter()
            .find(|c| c.eq_ignore_ascii_case(tokens[i].trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_')));
        if let Some(col_name) = col {
            if i + 2 < tokens.len() {
                let op = match tokens[i + 1].to_lowercase().as_str() {
                    "=" | "is" | "equals" => Some("="),
                    ">" | "above" | "greater" => Some(">"),
                    "<" | "below" | "less" => Some("<"),
                    ">=" => Some(">="),
                    "<=" => Some("<="),
                    _ => None,
                };
                if let Some(sql_op) = op {
                    let raw_val = tokens[i + 2].trim_matches(|c: char| c == '\'' || c == '"' || c == ',');
                    let rhs = if raw_val.parse::<f64>().is_ok() {
                        raw_val.to_string()
                    } else {
                        format!("'{}'", raw_val.replace('\'', "''"))
                    };
                    return Some(format!("\"{}\" {sql_op} {rhs}", col_name.replace('"', "\"\"")));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_schema_anomalies_and_relationships() {
        let mut records: Vec<Value> = (0..15)
            .map(|i| json!({ "user_id": i, "state": "California", "amount": 10.0 + i as f64 }))
            .collect();
        records.push(json!({ "user_id": 99, "state": "Nevada", "amount": 10000.0 }));

        let report = analyze_records(&records);
        assert_eq!(report.record_count, 16);
        assert_eq!(report.schema["type"], json!("object"));
        assert!(report
            .relationships
            .iter()
            .any(|r| r.field == "user_id" && r.target_entity == "user"));
        assert!(report
            .anomalies
            .iter()
            .any(|a| a.field == "amount" && a.kind == "numeric_outlier"));
    }

    #[test]
    fn translates_natural_language_to_sql() {
        let cols = vec!["name".to_string(), "state".to_string(), "total".to_string()];
        let sql1 = nl_to_sql("users", "show me all users from California", &cols);
        assert_eq!(sql1, r#"SELECT * FROM "users" WHERE "state" = 'California' LIMIT 100"#);

        let sql2 = nl_to_sql("orders", "count orders where total > 100", &cols);
        assert_eq!(sql2, r#"SELECT COUNT(*) AS count FROM "orders" WHERE "total" > 100"#);
    }
}
