# Search Result Highlighting & JSONPath Queries

Issue: [anitnilay20/thoth#8](https://github.com/anitnilay20/thoth/issues/8)  
Status: Implemented (Phases 0–3 shipped)  
Last Updated: 2025-12-02

---

## 1. Motivation & Vision

The original search only returned a `Vec<usize>` of record indices. That worked for jumping to rows, but the experience broke down immediately:

- Nothing told the user _why_ a record matched or which field satisfied a query.
- Highlighting was crude—entire rows were painted even if only a single token matched.
- Extending the engine to richer syntaxes (e.g., JSONPath) would have required rewiring both the backend and the UI.

Our goal was to introduce precise highlighting and advanced queries without regressing the “buttery” feel of the UI. The guiding principles:

1. **Single Source of Truth** – Search results must contain everything the UI needs (paths, ranges, previews). The UI never re-runs queries.
2. **Streaming Friendly** – The engine should still scan file chunks independently and only parse JSON when absolutely necessary.
3. **UI Fluidity** – Highlight spans are precomputed once and reused; egui widgets do zero substring or regex work at paint time.
4. **Pluggable Matchers** – Adding JSONPath (and future matchers) should not require new plumbing—just emit more structured fragments.
5. **Graceful Scaling** – Enforce fragment caps and reuse allocations so multi-GB NDJSON logs remain responsive.

---

## 2. Architecture Overview

```
┌─────────┐  query + mode   ┌──────────────┐  SearchHit Vec   ┌──────────────┐
│ Sidebar │ ──────────────► │ SearchEngine │ ────────────────► │ FileViewer   │
└─────────┘  history store  └──────────────┘  highlight map   └──────┬───────┘
      ▲                                                        Arc   │
      │ serialized entries                                     map   ▼
┌─────────────┐                                        ┌──────────────────────┐
│ Persistent  │◄────────────── per-file history ────── │ JsonTreeViewer/DataRow│
│ Search Store│                                        │ (RowHighlights)       │
└─────────────┘                                        └──────────────────────┘
```

Key building blocks:

- **Search State** – `Search` carries `query`, `match_case`, `query_mode`, and `SearchResults`.
- **Query Modes** – `QueryMode` is a serialized enum (`text` / `json_path`). History entries are stored as JSON so reopening a saved query restores its mode.
- **Search Hits** – Each record produces a `SearchHit { record_index, fragments, preview }`. Every `MatchFragment` includes `target`, `path`, `matched_text`, and `text_range`.
- **Highlight Cache** – `HashMap<usize, Arc<Vec<MatchFragment>>>` lets the viewer access per-record fragments in O(1) without cloning big vectors.
- **Row Rendering** – `JsonTreeViewer` maps fragments to `RowHighlights`, and `DataRow` paints them via `egui::text::LayoutJob` (no runtime searching).

---

## 3. Query Modes & Algorithms

### 3.1 Text Mode (Phases 0–2 Recap)

1. **Fast ASCII Scan**
   - Each rayon worker copies the record slice if case-folding is required and calls `memmem::Finder::find_iter`.
   - `MAX_FRAGMENTS_PER_RECORD` bounds work per record (default: 64).

2. **Structured Matches**
   - `collect_field_matches` parses the record into `serde_json::Value` _only when the record already matches the substring query_.
   - It walks the JSON hierarchy once, using `find_match_ranges` (byte-level comparisons) to capture key/value offsets.

3. **Range Translation**
   - Fragments carry `text_range: Range<u32>`.
   - `JsonTreeViewer::compute_row_highlights` adjusts the range to account for string formatting, quotes, and whitespace before storing them in `RowHighlights`.

4. **Rendering**
   - `DataRow` splits `"key: value"` strings only once (`splitn(2, ':')`) and feeds the range list into a `LayoutJob`:

     ```rust
     for range in ranges {
         job.append(&text[cursor..range.start], 0.0, base_format.clone());
         job.append(&text[range.clone()], 0.0, highlight_format.clone());
         cursor = range.end;
     }
     ```

   - Hover/selection overlays are blended separately, so highlights stay vivid in both Catppuccin Latte (light) and Mocha (dark) themes.

### 3.2 JSONPath Mode (Phase 3)

#### Parsing

- `JsonPathQuery::parse(&str)` tokenizes expressions like:
  - `$.store.book[0].author`
  - `$.user["first-name"] = "Anit"`
  - `$.items[*].price = 42`
- Supported syntax today:
  - Dot notation + bracket notation (including quoted identifiers).
  - Array indexes (`[3]`) and wildcards (`[*]`).
  - Single equality filter at the end (`= <value>`). Values can be JSON literals or single-quoted strings (we normalize them to valid JSON).
- Output: `JsonPathQuery { original, segments: Vec<PathSegment>, filter: Option<FilterValue> }`. The original string is retained for previews.

#### Evaluation

1. Start with a frontier `Vec<(String, &Value)>` containing the root path (record index) and the parsed JSON value.
2. For every `PathSegment`, produce the next frontier:
   - **Field** – If `value` is an object, look up the property and append `"{path}.{field}"`.
   - **FieldWildcard** – Push all properties.
   - **ArrayIndex** / **ArrayWildcard** – Similar but for arrays, using `"path[idx]"` notation.
3. After all segments, optionally evaluate the equality filter. String filters honor `match_case`; other value types are compared structurally.
4. Convert each surviving `(path, value)` into a `JsonPathMatch`:
   - Strings/numbers/bools/null store both `matched_text` and `highlight_range: 0..len`.
   - Complex values (objects/arrays) mark `component = FieldComponent::EntireRow` with `display_value = preview_value`.

#### MatchFragment Conversion

`jsonpath_scan` maps each `JsonPathMatch` to a `MatchFragment`:

- `target: MatchTarget::JsonField { component }`
- `path: Arc<str>` inserted into the highlight map.
- `matched_text` + `text_range` enable the same `RowHighlights` code path as text mode.
- A compact `MatchPreview { before: "<query> -> <path>", highlight: "<value>" }` improves sidebar readability.

Because all fragments look identical to the UI, the viewer/rendering layer is oblivious to how the match was produced.

---

## 4. UI & Persistence

### 4.1 Sidebar UX

- **Automatic Mode Detection** – Query mode is automatically detected based on the query prefix:
  - Queries starting with `$` are treated as JSONPath queries
  - All other queries use text search mode
  - This eliminates the need for a manual mode selector
- **Smart Placeholder** – The search input displays hint text: `"Search... (use $ prefix for JSONPath, e.g. $.user.name = \"alice\")"`, guiding users to use the `$` prefix for JSONPath queries.
- **Clear/Search Buttons** – Create `SearchMessage::StartSearch` with auto-detected mode based on query content.
- **History Persistence** – History entries are stored per file in `~/Library/Application Support/thoth/search_history.json` (JSON format). Each entry contains `{ "mode": "json_path", "query": "$.user.name='anit'" }`.
- **History Display** – Recent searches show only the query text without mode prefixes (e.g., no `[JSONPath]` label), maintaining a clean interface. The mode is preserved internally and restored when clicking a history entry.
### 4.2 Persistent State

- We cap history at `MAX_SEARCH_HISTORY_PER_FILE` per file and `MAX_FILES_WITH_HISTORY` overall, trimming by recency.
- Entries are deduplicated (most recent first) exactly like recent files.

### 4.3 Viewer Integration

- `CentralPanel` passes highlight maps to `FileViewer`, which in turn calls `JsonTreeViewer::set_highlights`.
- Navigation (clicking a sidebar entry) still only sets `selected = Some(path)`; highlights are rendered automatically on the next repaint.
- Context menus, indent guides, and other UI affordances remain untouched because highlight metadata is orthogonal.

---

## 5. Performance Considerations

| Concern             | Strategy                                                                                                      |
| ------------------- | ------------------------------------------------------------------------------------------------------------- |
| Text Search Speed   | `memmem::Finder` (SIMD-accelerated) with per-worker lowercase buffers; zero JSON parsing for non-matches.     |
| JSONPath Cost       | Parse `serde_json::Value` once per record per query (still parallelized). `MAX_FRAGMENTS_PER_RECORD` applies. |
| Highlight Rendering | `RowHighlights` contains raw byte ranges; `DataRow` simply stitches strings and never does substring search.  |
| Memory Usage        | Highlight maps store `Arc<Vec<MatchFragment>>` so the sidebar and viewer share data without cloning.          |
| UI Smoothness       | Rendering does not block on search threads; `search.scanning` toggles a lightweight spinner.                  |

Even worst-case scenarios (searching for a single character) stay smooth because highlight ranges are precomputed. Layout work per row scales with the number of fragments in that row, not the size of the tree or the query complexity.

---

## 6. Rollout Recap

| Phase                          | Status | Highlights                                                                                                               |
| ------------------------------ | ------ | ------------------------------------------------------------------------------------------------------------------------ |
| **0 – Data Model Prep**        | ✅     | Introduced `SearchResults`, `SearchHit`, `MatchFragment`, preview snippets, and highlight maps.                          |
| **1 – free-text highlighting** | ✅     | Implemented `RowHighlights`, `DataRow` layout jobs, and sidebar snippets.                                                |
| **2 – structured metadata**    | ✅     | Populated per-field ranges, swapped full-row backgrounds for substring highlights, and ensured hover/selection blending. |
| **3 – JSONPath queries**       | ✅     | Added `QueryMode`, JSONPath parser/evaluator, mode-aware sidebar UX, and rich previews (`query -> path`).                |

Every phase was shippable; each built upon the same data flow, enabling incremental delivery without regressions.

---

## 7. Future Work

1. **Additional Matchers** – Regex, logical combinations, or field-specific filters can slot into `QueryMode` and emit `MatchFragment`s with `fragment_id`s for color-coding.
2. **Raw Preview Highlighting** – Currently only the tree view paints spans. Extending highlights to a raw JSON preview panel would reuse the same `ByteRange` metadata.
3. **Legend / Explanation Chips** – `fragment_id` is reserved for mapping fragments to legend entries (e.g., “Matches `$.user.name = 'Anit'`”).
4. **Result Ranking** – With structured metadata we can experiment with scoring (depth, key names, filter confidence) without touching the UI.

The current architecture already answers the hard questions (where and why a record matched). Future enhancements simply interpret the existing metadata in richer ways.
