# Search Result Highlighting & Matcher Abstraction

Issue: [anitnilay20/thoth#8](https://github.com/anitnilay20/thoth/issues/8)  
Status: Draft (MVP scope)  
Last Updated: 2025-02-14

---

## Motivation

The current search engine only returns a `Vec<usize>` of root record indices. The UI can scroll to those roots but it cannot:

- Highlight the exact text spans that matched the query.
- Render different highlight colors per query fragment.
- Support richer query syntaxes (JSONPath, logical expressions) that need to surface which field satisfied which clause.

To ship precise highlighting without painting ourselves into a corner, we need a reusable model that captures *where* and *why* a match occurred. The same machinery must work for the existing substring search (powered by `memmem`) and future matchers like `$.store.book[*].author = "anit"`.

---

## Goals

- Capture every match as a structured `SearchHit` that includes the record index and highlightable fragments.
- Allow upcoming matcher implementations (free-text, JSONPath, regex, etc.) to share a common API.
- Provide enough metadata for the UI to paint highlights in both the sidebar list and the main viewer without re-running the query.
- Keep the `LazyJsonFile` fast path (zero extra parsing for non-matching records) and avoid blocking the UI thread.

### Non-Goals

- Changing ranking or filtering semantics (search still surfaces root indices; the viewer remains unfiltered).
- Building a full JSONPath engine in this iteration—the doc only reserves the abstractions required for it.
- Persisting highlight metadata across sessions (recompute per search).

---

## Proposed Architecture

### 1. Query & Matcher Model

Introduce two new enums that travel with `Search`:

```rust
pub enum QueryKind {
    FreeText,
    JsonPath,
    // future: Regex, Combination, etc.
}

pub enum QueryFragment {
    RawText(String),
    JsonPathExpr { id: u32, expr: Arc<str> },
}
```

`Search` stores `query_kind` plus a parsed `Vec<QueryFragment>`. Each matcher implementation knows how to evaluate a fragment and emit highlight metadata. This keeps `SearchMessage::StartSearch(Search)` backward compatible while giving the engine enough context to select the correct matcher.

### 2. Result Data Structures

Replace `Vec<usize>` with a richer container:

```rust
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub stats: SearchStats,
}

pub struct SearchHit {
    pub record_index: usize,
    pub fragments: Vec<MatchFragment>,
}

pub struct MatchFragment {
    pub fragment_id: u32, // ties back to QueryFragment
    pub target: MatchTarget,
    pub byte_range: std::ops::Range<u32>,
    pub path: Option<Arc<str>>, // JSON pointer style for structured matches
    pub confidence: f32,        // reserved for ranking tweaks
}

pub enum MatchTarget {
    RawRecord,
    JsonField { component: FieldComponent },
}

pub enum FieldComponent {
    Key,
    Value,
    EntireRow,
}
```

Key properties:

- `byte_range` always references offsets inside `LazyJsonFile::raw_slice(record_index)` so we can recolor the raw view without storing the string itself.
- `path` is optional; substring matches can omit it, while JSONPath matches fill it with `/store/book/3/author`.
- `fragment_id` connects the UI to the fragment metadata (for legend chips, tooltips, etc.).

`SearchHit` stays light enough to clone when broadcasting through `SearchMessage`, but if cloning becomes an issue we can wrap the `Vec<SearchHit>` in an `Arc`.

### 3. Engine Changes

1. **Matcher Trait**

   ```rust
   pub trait SearchMatcher: Send + Sync {
       fn scan_record(&self, record_index: usize, bytes: &[u8]) -> Option<SearchHit>;
   }
   ```

   - `FreeTextMatcher` keeps the current `memmem` fast path. It uses `Finder::new(&needle).find_iter(bytes)` to collect every offset and returns a `SearchHit` with `MatchTarget::RawRecord`.
   - Future matchers (JSONPath, etc.) can parse the record into `serde_json::Value`, evaluate the fragment, and emit `MatchTarget::JsonField`.

2. **parallel_scan**

   - Select a matcher from `QueryKind`.
   - Keep the rayon-based `into_par_iter()`; each worker clones an `Arc<dyn SearchMatcher>` so there is no shared mutable state.
   - Collect `SearchHit`s, sort by `record_index`, and stash them in `SearchResults`.

3. **Search Struct**

   ```rust
   pub struct Search {
       pub results: SearchResults,
       pub highlights: HashMap<usize, Arc<Vec<MatchFragment>>>, // derived cache for O(1) lookup
       // existing fields...
   }
   ```

   - `highlights` makes it cheap for the viewer to fetch matches for a root without scanning the `Vec`.

### 4. UI Integration

**Sidebar (Search Component)**

- `SearchProps` already receive `search_state`. After the change, the result list reads `search_state.results.hits`.
- The button label can show the number of fragments or render a short snippet (first fragment from the hit) by slicing `LazyJsonFile::raw_slice(record_index)` using the stored `byte_range`.

**Central Panel & FileViewer**

- Extend `FileViewer::ui` to accept `Option<&HighlightMap>` (a map from `record_index` to fragments).
- When a root is expanded or rendered, pass the relevant fragments to the viewer implementation (`JsonTreeViewer::render`).

**JsonTreeViewer / DataRow**

- Add optional highlight spans to `DataRowProps`. The row renderer will build an `egui::text::LayoutJob` that alternates between normal and highlighted text segments (`job.add_text(...)` with custom background colors).
- Map fragments to rows:
  - If `MatchTarget::RawRecord`, highlight inside a collapsible *raw* preview (simple multiline `TextEdit` with background spans). This is the immediate win for free-text search.
  - If `MatchTarget::JsonField`, match the `path` against the row's `path` (e.g., `0.user.name`). We can precompute a `HashMap<String, Vec<MatchFragment>>` per hit so each row performs an `O(1)` lookup.

### 5. Rendering Highlight Spans

1. When a record is selected/navigated to, fetch `raw_slice(record_index)` and build a cached `Vec<DisplaySpan>`:

   ```rust
   pub struct DisplaySpan {
       pub range: std::ops::Range<usize>,
       pub color: egui::Color32,
   }
   ```

2. Use `egui::text::LayoutJob` for both the raw preview and the tree rows. Example:

```rust
let mut job = LayoutJob::default();
for (range, color) in spans {
    job.append(&text[last..range.start], 0.0, default_format.clone());
    job.append(&text[range.clone()], 0.0, HighlightFormat::with_bg(color));
    last = range.end;
}
```

3. Store the computed layout in `ViewerState` so repeated renders do not rebuild the job unless either the selection changes or a new search runs.

### 6. Performance Notes

- `FreeTextMatcher` keeps the existing no-allocation scan path (copies once per record to lowercase when `match_case` is off). The only new overhead is storing a few `Range<u32>` values per match.
- JSONPath/structured matchers will necessarily parse the JSON; by isolating that logic in the matcher we can experiment with caching or partial traversal without touching the UI.
- Highlight maps are keyed by `usize` and wrapped in `Arc`s so they can be read from both the sidebar and the viewer without cloning the fragment vectors.

---

## Rollout Plan

1. **Phase 0 – Data model prep**
   - Introduce `SearchResults`, `SearchHit`, and `MatchFragment`.
   - Sidebar continues to only display record indices, but unit tests ensure we can serialize/clone the new structs.

2. **Phase 1 – Free-text highlighting**
   - Implement `FreeTextMatcher` and span rendering inside a raw preview panel.
   - Update the sidebar list to show short snippets cut from stored `byte_range`s.

3. **Phase 2 – Structured match metadata**
   - Extend the matcher trait with optional JSON parsing helpers.
   - Populate `path` for object/array values and highlight the correct rows in `JsonTreeViewer`.

4. **Phase 3 – JSONPath queries**
   - Plug in a JSONPath matcher that walks the parsed `Value` and emits `MatchTarget::JsonField`.
   - Update the sidebar to show the JSONPath fragment that matched (via `fragment_id`).

Each phase is shippable; MVP highlighting provides immediate UX value while the later phases enable the advanced query syntax requested in issue #8.

---

## Open Questions

1. **Raw preview location** – Should we embed the raw record preview under the tree view, or open it inside a popover when the user hovers a search result?
2. **Highlight persistence** – Do we want to keep highlights when the user edits the file (once editing exists), or should we invalidate them immediately?
3. **Memory budget** – Large result sets could store thousands of fragments. We might cap the total fragments per hit and expose “+N more” in the sidebar.
4. **Theme integration** – Need to confirm highlight colors for light/dark themes and contrast ratios for accessibility.

Feedback on these questions will help refine the final implementation before coding begins.
