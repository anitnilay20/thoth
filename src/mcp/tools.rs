//! MCP tool definitions for the Thoth server.
//!
//! Uses rmcp's `#[tool_router]` macro to expose Thoth's file and search
//! capabilities as MCP tools.

use std::path::PathBuf;

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};

use super::state::ServerState;

// ─── The MCP Server struct ───────────────────────────────────────────────────

/// The Thoth MCP server — holds shared state and exposes tools.
#[derive(Clone)]
pub struct ThothMcpServer {
    state: ServerState,
}

impl ThothMcpServer {
    pub fn new(state: ServerState) -> Self {
        Self { state }
    }
}

// ─── Tool parameter / output types ───────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct OpenFileParams {
    /// Absolute or relative path to the JSON/NDJSON file to open.
    pub path: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct OpenFileResult {
    /// Opaque handle used to reference this file in subsequent tool calls.
    pub handle: String,
    /// The resolved file path.
    pub path: String,
    /// Detected format: "ndjson", "json_array", or "json_object".
    pub file_type: String,
    /// Number of top-level records in the file.
    pub record_count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct CloseFileParams {
    /// Handle of the file to close (returned by open_file).
    pub handle: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CloseFileResult {
    /// Whether the file was successfully closed.
    pub closed: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct GetFileInfoParams {
    /// Handle of the file to inspect.
    pub handle: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetFileInfoResult {
    pub handle: String,
    pub path: String,
    pub file_type: String,
    pub record_count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct GetRecordParams {
    /// Handle of the open file.
    pub handle: String,
    /// Zero-based index of the record to retrieve.
    pub index: usize,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetRecordResult {
    /// The record index that was requested.
    pub index: usize,
    /// The JSON record as a string.
    pub record: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct GetRecordCountParams {
    /// Handle of the open file.
    pub handle: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetRecordCountResult {
    pub record_count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SearchParams {
    /// Handle of the open file to search.
    pub handle: String,
    /// The search query string. For text search, this is a substring.
    /// For JSONPath, prefix with `$` (e.g. `$.user.name`).
    pub query: String,
    /// Search mode: "text" or "jsonpath". Defaults to "text".
    /// If the query starts with "$", jsonpath mode is used automatically.
    pub mode: Option<String>,
    /// Whether to match case-sensitively. Defaults to false.
    pub match_case: Option<bool>,
    /// Maximum number of results to return. Defaults to 50.
    pub max_results: Option<usize>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SearchResult {
    /// Total number of matching records.
    pub total_matches: usize,
    /// The matches returned (up to max_results).
    pub matches: Vec<SearchMatch>,
    /// The query that was executed.
    pub query: String,
    /// The mode used: "text" or "jsonpath".
    pub mode: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SearchMatch {
    /// Zero-based record index.
    pub record_index: usize,
    /// Short preview snippet of the match.
    pub preview: Option<String>,
    /// JSONPath or field path where the match occurred (if available).
    pub match_path: Option<String>,
}

// ─── Phase 2: Data tool parameter / output types ─────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct GetValueAtPathParams {
    /// Handle of the open file.
    pub handle: String,
    /// Zero-based record index.
    pub index: usize,
    /// Dot-notation path to the value, e.g. "user.address.city" or "items[2].name".
    pub path: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetValueAtPathResult {
    /// The extracted value as a JSON string.
    pub value: String,
    /// The path that was queried.
    pub path: String,
    /// The JSON type of the value: "string", "number", "boolean", "null", "object", "array".
    pub value_type: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct ExtractKeysParams {
    /// Handle of the open file.
    pub handle: String,
    /// Dot-notation path prefix to inspect keys under, e.g. "user" to get keys of the "user" object.
    /// Use empty string or omit for top-level keys.
    pub path: Option<String>,
    /// Maximum number of records to sample for key extraction. Defaults to 100.
    pub sample_size: Option<usize>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ExtractKeysResult {
    /// Sorted list of unique keys found.
    pub keys: Vec<String>,
    /// Number of records sampled.
    pub records_sampled: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SampleRecordsParams {
    /// Handle of the open file.
    pub handle: String,
    /// Number of records to return. Defaults to 5.
    pub count: Option<usize>,
    /// Sampling strategy: "first" (default), "last", or "even" (evenly spaced).
    pub strategy: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SampleRecordsResult {
    /// The sampled records.
    pub records: Vec<SampledRecord>,
    /// Total number of records in the file.
    pub total_records: usize,
    /// The strategy used.
    pub strategy: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SampledRecord {
    /// Zero-based index of this record in the file.
    pub index: usize,
    /// The JSON record as a string.
    pub record: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct GetSchemaParams {
    /// Handle of the open file.
    pub handle: String,
    /// Maximum number of records to sample for schema inference. Defaults to 50.
    pub sample_size: Option<usize>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetSchemaResult {
    /// Inferred JSON schema for the records.
    pub schema: serde_json::Value,
    /// Number of records sampled for inference.
    pub records_sampled: usize,
}

// ─── Tool implementations ────────────────────────────────────────────────────

#[tool_router]
impl ThothMcpServer {
    #[tool(
        name = "open_file",
        description = "Open a JSON, NDJSON, or GeoJSON file for inspection. Returns a handle for use with other tools."
    )]
    fn open_file(&self, Parameters(params): Parameters<OpenFileParams>) -> Json<OpenFileResult> {
        let path = PathBuf::from(&params.path);

        // Canonicalize path for consistent display
        let resolved = path.canonicalize().unwrap_or_else(|_| path.clone());

        match self.state.open_file(&resolved) {
            Ok((_handle, info)) => Json(OpenFileResult {
                handle: info.handle,
                path: info.path,
                file_type: info.file_type,
                record_count: info.record_count,
            }),
            Err(e) => {
                // Return error as a result with empty handle so the LLM sees the message
                Json(OpenFileResult {
                    handle: String::new(),
                    path: resolved.display().to_string(),
                    file_type: format!("error: {}", e),
                    record_count: 0,
                })
            }
        }
    }

    #[tool(
        name = "close_file",
        description = "Close a previously opened file, freeing its resources. Use the handle from open_file."
    )]
    fn close_file(&self, Parameters(params): Parameters<CloseFileParams>) -> Json<CloseFileResult> {
        let closed = self.state.close_file(&params.handle);
        Json(CloseFileResult { closed })
    }

    #[tool(
        name = "get_file_info",
        description = "Get metadata about an open file: path, detected type, and record count."
    )]
    fn get_file_info(
        &self,
        Parameters(params): Parameters<GetFileInfoParams>,
    ) -> Json<GetFileInfoResult> {
        match self.state.file_info(&params.handle) {
            Some(info) => Json(GetFileInfoResult {
                handle: info.handle,
                path: info.path,
                file_type: info.file_type,
                record_count: info.record_count,
            }),
            None => Json(GetFileInfoResult {
                handle: params.handle.clone(),
                path: String::new(),
                file_type: format!("error: no file with handle '{}'", params.handle),
                record_count: 0,
            }),
        }
    }

    #[tool(
        name = "get_record",
        description = "Retrieve a single JSON record by its zero-based index from an open file."
    )]
    fn get_record(&self, Parameters(params): Parameters<GetRecordParams>) -> Json<GetRecordResult> {
        let result = self.state.with_file(&params.handle, |file| {
            match file.file_type.get(params.index) {
                Ok(value) => {
                    let record =
                        serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                    Ok(record)
                }
                Err(e) => Err(format!("{}", e)),
            }
        });

        match result {
            Some(Ok(record)) => Json(GetRecordResult {
                index: params.index,
                record,
            }),
            Some(Err(e)) => Json(GetRecordResult {
                index: params.index,
                record: format!("error: {}", e),
            }),
            None => Json(GetRecordResult {
                index: params.index,
                record: format!("error: no file with handle '{}'", params.handle),
            }),
        }
    }

    #[tool(
        name = "get_record_count",
        description = "Get the total number of top-level records in an open file."
    )]
    fn get_record_count(
        &self,
        Parameters(params): Parameters<GetRecordCountParams>,
    ) -> Json<GetRecordCountResult> {
        let count = self
            .state
            .with_file(&params.handle, |file| file.record_count());

        Json(GetRecordCountResult {
            record_count: count.unwrap_or(0),
        })
    }

    #[tool(
        name = "search",
        description = "Search records in an open file using text substring match or JSONPath query. Returns matching record indices with preview snippets."
    )]
    fn search(&self, Parameters(params): Parameters<SearchParams>) -> Json<SearchResult> {
        use crate::search::{QueryMode, Search};

        let max_results = params.max_results.unwrap_or(50);
        let match_case = params.match_case.unwrap_or(false);

        // Auto-detect mode from query prefix if not explicitly specified
        let mode = match params.mode.as_deref() {
            Some("jsonpath") => QueryMode::JsonPath,
            Some("text") => QueryMode::Text,
            _ => {
                if params.query.starts_with('$') {
                    QueryMode::JsonPath
                } else {
                    QueryMode::Text
                }
            }
        };

        let mode_str = match mode {
            QueryMode::Text => "text",
            QueryMode::JsonPath => "jsonpath",
        };

        // Get the file path from state
        let file_path = self
            .state
            .with_file(&params.handle, |file| file.path.clone());

        let file_path = match file_path {
            Some(p) => p,
            None => {
                return Json(SearchResult {
                    total_matches: 0,
                    matches: vec![],
                    query: params.query,
                    mode: mode_str.to_string(),
                });
            }
        };

        // Get the file kind from state
        let file_kind = self.state.with_file(&params.handle, |file| file.file_kind);
        let file_kind = file_kind.unwrap_or_default();

        // Use Search engine — it reopens the file internally for thread-safe parallel scanning
        let mut search = Search {
            query: params.query.clone(),
            match_case,
            query_mode: mode,
            ..Search::default()
        };

        let path_opt = Some(file_path);
        search.start_scanning_internal(&path_opt, &file_kind);

        if let Some(err) = &search.error {
            return Json(SearchResult {
                total_matches: 0,
                matches: vec![SearchMatch {
                    record_index: 0,
                    preview: Some(format!("Search error: {}", err)),
                    match_path: None,
                }],
                query: params.query,
                mode: mode_str.to_string(),
            });
        }

        let hits = search.results.hits();
        let total = hits.len();
        let capped = &hits[..total.min(max_results)];

        let matches: Vec<SearchMatch> = capped
            .iter()
            .map(|hit| {
                let preview = hit
                    .preview
                    .as_ref()
                    .map(|p| format!("{}«{}»{}", p.before, p.highlight, p.after));

                let match_path = hit
                    .fragments
                    .first()
                    .and_then(|f| f.path.as_ref())
                    .map(|p| p.to_string());

                SearchMatch {
                    record_index: hit.record_index,
                    preview,
                    match_path,
                }
            })
            .collect();

        Json(SearchResult {
            total_matches: total,
            matches,
            query: params.query,
            mode: mode_str.to_string(),
        })
    }

    // ─── Phase 2: Data tools ─────────────────────────────────────────────

    #[tool(
        name = "get_value_at_path",
        description = "Extract a nested value from a record using dot-notation path (e.g. 'user.address.city' or 'items[2].name')."
    )]
    fn get_value_at_path(
        &self,
        Parameters(params): Parameters<GetValueAtPathParams>,
    ) -> Json<GetValueAtPathResult> {
        use crate::helpers::walk_rel;

        let result = self.state.with_file(&params.handle, |file| {
            let record = file.file_type.get(params.index)?;
            if params.path.is_empty() {
                Ok(record)
            } else {
                walk_rel(record, &params.path)
            }
        });

        match result {
            Some(Ok(value)) => {
                let value_type = json_type_name(&value).to_string();
                let value_str =
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                Json(GetValueAtPathResult {
                    value: value_str,
                    path: params.path,
                    value_type,
                })
            }
            Some(Err(e)) => Json(GetValueAtPathResult {
                value: format!("error: {}", e),
                path: params.path,
                value_type: "error".to_string(),
            }),
            None => Json(GetValueAtPathResult {
                value: format!("error: no file with handle '{}'", params.handle),
                path: params.path,
                value_type: "error".to_string(),
            }),
        }
    }

    #[tool(
        name = "extract_keys",
        description = "List all unique keys found across records, optionally at a specific nested path. Useful for understanding the structure of JSON data."
    )]
    fn extract_keys(
        &self,
        Parameters(params): Parameters<ExtractKeysParams>,
    ) -> Json<ExtractKeysResult> {
        use crate::helpers::walk_rel;
        use std::collections::BTreeSet;

        let sample_size = params.sample_size.unwrap_or(100);
        let path = params.path.as_deref().unwrap_or("");

        let result = self.state.with_file(&params.handle, |file| {
            let total = file.record_count();
            let count = total.min(sample_size);
            let mut keys = BTreeSet::new();

            for i in 0..count {
                if let Ok(record) = file.file_type.get(i) {
                    let target = if path.is_empty() {
                        record
                    } else {
                        match walk_rel(record, path) {
                            Ok(v) => v,
                            Err(_) => continue,
                        }
                    };

                    if let Some(obj) = target.as_object() {
                        for key in obj.keys() {
                            keys.insert(key.clone());
                        }
                    }
                }
            }

            (keys.into_iter().collect::<Vec<_>>(), count)
        });

        match result {
            Some((keys, sampled)) => Json(ExtractKeysResult {
                keys,
                records_sampled: sampled,
            }),
            None => Json(ExtractKeysResult {
                keys: vec![format!("error: no file with handle '{}'", params.handle)],
                records_sampled: 0,
            }),
        }
    }

    #[tool(
        name = "sample_records",
        description = "Return a sample of records from an open file. Strategies: 'first' (default), 'last', or 'even' (evenly spaced across the file)."
    )]
    fn sample_records(
        &self,
        Parameters(params): Parameters<SampleRecordsParams>,
    ) -> Json<SampleRecordsResult> {
        let count = params.count.unwrap_or(5);
        let strategy = params.strategy.as_deref().unwrap_or("first");

        let result = self.state.with_file(&params.handle, |file| {
            let total = file.record_count();
            let n = count.min(total);

            let indices: Vec<usize> = match strategy {
                "last" => {
                    if total == 0 {
                        vec![]
                    } else {
                        ((total.saturating_sub(n))..total).collect()
                    }
                }
                "even" => {
                    if n == 0 || total == 0 {
                        vec![]
                    } else if n >= total {
                        (0..total).collect()
                    } else {
                        (0..n).map(|i| i * (total - 1) / (n - 1).max(1)).collect()
                    }
                }
                _ => (0..n).collect(), // "first"
            };

            let mut records = Vec::with_capacity(indices.len());
            for idx in &indices {
                if let Ok(value) = file.file_type.get(*idx) {
                    let record_str =
                        serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                    records.push(SampledRecord {
                        index: *idx,
                        record: record_str,
                    });
                }
            }

            (records, total)
        });

        match result {
            Some((records, total)) => Json(SampleRecordsResult {
                records,
                total_records: total,
                strategy: strategy.to_string(),
            }),
            None => Json(SampleRecordsResult {
                records: vec![],
                total_records: 0,
                strategy: strategy.to_string(),
            }),
        }
    }

    #[tool(
        name = "get_schema",
        description = "Infer a JSON schema from the records in an open file by sampling records and analyzing their structure, types, and keys."
    )]
    fn get_schema(&self, Parameters(params): Parameters<GetSchemaParams>) -> Json<GetSchemaResult> {
        let sample_size = params.sample_size.unwrap_or(50);

        let result = self.state.with_file(&params.handle, |file| {
            let total = file.record_count();
            let count = total.min(sample_size);
            let mut sampled_values = Vec::with_capacity(count);

            for i in 0..count {
                if let Ok(value) = file.file_type.get(i) {
                    sampled_values.push(value);
                }
            }

            let schema = infer_schema(&sampled_values);
            (schema, count)
        });

        match result {
            Some((schema, sampled)) => Json(GetSchemaResult {
                schema,
                records_sampled: sampled,
            }),
            None => Json(GetSchemaResult {
                schema: serde_json::json!({"error": format!("no file with handle '{}'", params.handle)}),
                records_sampled: 0,
            }),
        }
    }
}

// ─── ServerHandler impl ──────────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for ThothMcpServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        let mut info = rmcp::model::ServerInfo::default();
        info.server_info = rmcp::model::Implementation::new("thoth", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Thoth is a high-performance JSON/NDJSON file inspector. \
             Use open_file to load a file, then use tools like get_record, search, \
             extract_keys, sample_records, and get_schema to explore the data."
                .to_string(),
        );
        info
    }
}

// ─── Helper functions ────────────────────────────────────────────────────────

/// Return a human-readable type name for a JSON value.
fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Infer a JSON schema from a collection of sample values.
///
/// Produces a schema object with "type", "properties" (for objects),
/// "items" (for arrays), and tracks which fields are required (present in all samples).
pub fn infer_schema(samples: &[serde_json::Value]) -> serde_json::Value {
    use serde_json::{Map, Value, json};
    use std::collections::{BTreeMap, BTreeSet};

    if samples.is_empty() {
        return json!({"type": "unknown", "description": "No samples available"});
    }

    // Collect types seen across all samples
    let mut types_seen: BTreeSet<&str> = BTreeSet::new();
    for sample in samples {
        types_seen.insert(json_type_name(sample));
    }

    // If all samples are objects, build a property schema
    if types_seen.len() == 1 && types_seen.contains("object") {
        let mut field_types: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut field_counts: BTreeMap<String, usize> = BTreeMap::new();

        for sample in samples {
            if let Some(obj) = sample.as_object() {
                for (key, val) in obj {
                    field_types
                        .entry(key.clone())
                        .or_default()
                        .insert(json_type_name(val).to_string());
                    *field_counts.entry(key.clone()).or_insert(0) += 1;
                }
            }
        }

        let total = samples.len();
        let mut properties = Map::new();
        let mut required = Vec::new();

        for (key, types) in &field_types {
            let type_list: Vec<&String> = types.iter().collect();
            let prop = if type_list.len() == 1 {
                json!({"type": type_list[0]})
            } else {
                json!({"type": type_list})
            };
            properties.insert(key.clone(), prop);

            if field_counts.get(key).copied().unwrap_or(0) == total {
                required.push(Value::String(key.clone()));
            }
        }

        let mut schema = json!({
            "type": "object",
            "properties": properties,
        });

        if !required.is_empty() {
            schema["required"] = Value::Array(required);
        }

        schema
    } else if types_seen.len() == 1 && types_seen.contains("array") {
        // If all samples are arrays, infer item type
        let mut item_types: BTreeSet<String> = BTreeSet::new();
        for sample in samples {
            if let Some(arr) = sample.as_array() {
                for item in arr {
                    item_types.insert(json_type_name(item).to_string());
                }
            }
        }

        let items = if item_types.len() == 1 {
            json!({"type": item_types.iter().next().unwrap()})
        } else {
            let types: Vec<&String> = item_types.iter().collect();
            json!({"type": types})
        };

        json!({
            "type": "array",
            "items": items,
        })
    } else if types_seen.len() == 1 {
        // Uniform primitive type
        json!({"type": types_seen.iter().next().unwrap()})
    } else {
        // Mixed types
        let types: Vec<&&str> = types_seen.iter().collect();
        json!({"type": types})
    }
}
