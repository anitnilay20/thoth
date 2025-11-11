# JsonViewer Refactoring Plan

## Problem Statement

Currently `JsonViewer` handles too many responsibilities:

1. File loading (`loader`, `open()`)
2. Search filtering (`visible_roots`, `set_root_filter()`)
3. UI state management (`expanded`, `selected`, `rows`)
4. Caching (`cache`)
5. JSON-specific rendering logic
6. Tree structure building

This makes it hard to:

- Support other file types (CSV, XML, YAML, etc.)
- Test components in isolation
- Reuse common viewer functionality
- Maintain and extend features

## Proposed Architecture

### New Component Hierarchy

```
FileViewer (Generic Parent)
├── File loading & management
├── Search filtering (visible_roots)
├── Selection state (selected)
├── Cache management (LruCache)
└── Delegates to format-specific viewers
    ├── JsonTreeViewer (JSON/NDJSON)
    │   ├── Tree expansion state
    │   ├── Row building
    │   └── JSON rendering
    ├── CsvTableViewer (future)
    ├── XmlTreeViewer (future)
    └── TextViewer (future - plain text fallback)
```

## Detailed Design

### 1. FileViewer (Generic Parent Component)

**Location:** `src/components/file_viewer/mod.rs`

**Responsibilities:**

- File loading via lazy loader
- Search result filtering
- Selection state management
- Cache management (generic over Value types)
- Dispatching to format-specific viewers

```rust
pub struct FileViewer {
    // Data source (file-agnostic)
    loader: Option<Box<dyn FileLoader>>,  // Trait for different loaders
    file_type: FileType,

    // Common viewer state
    visible_roots: Option<Vec<usize>>,  // Search filtering
    selected: Option<String>,           // Current selection
    cache: LruCache<usize, CachedValue>, // Generic cache

    // Format-specific viewers
    json_viewer: Option<JsonTreeViewer>,
    // csv_viewer: Option<CsvTableViewer>,  // Future
    // xml_viewer: Option<XmlTreeViewer>,   // Future
}

impl FileViewer {
    pub fn open(&mut self, path: &Path, file_type: &FileType) -> Result<()> {
        // Load file with appropriate loader
        // Initialize appropriate viewer based on file_type
    }

    pub fn set_root_filter(&mut self, filter: Option<Vec<usize>>) {
        // Common search filtering
        self.visible_roots = filter;
        // Notify active viewer to rebuild
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        // Dispatch to appropriate viewer
        match self.file_type {
            FileType::Json | FileType::Ndjson => {
                if let Some(viewer) = &mut self.json_viewer {
                    viewer.render(ui, &self.visible_roots, &self.cache, &mut self.selected);
                }
            }
            // FileType::Csv => { ... }  // Future
            // FileType::Xml => { ... }  // Future
        }
    }
}
```

### 2. JsonTreeViewer (JSON-Specific)

**Location:** `src/components/file_viewer/json_tree_viewer.rs`

**Responsibilities:**

- Tree expansion state (`expanded`)
- Row building (`rows`, `build_rows_from_value()`)
- JSON-specific rendering logic
- Syntax highlighting with TextTokens

```rust
pub struct JsonTreeViewer {
    // JSON-specific UI state
    expanded: HashSet<String>,  // Tree expansion state
    rows: Vec<JsonRow>,         // Flattened render list
}

impl JsonTreeViewer {
    pub fn new() -> Self { ... }

    pub fn render(
        &mut self,
        ui: &mut Ui,
        visible_roots: &Option<Vec<usize>>,
        cache: &LruCache<usize, Value>,
        selected: &mut Option<String>,
    ) {
        // Build rows from visible roots
        // Render tree with expand/collapse
        // Handle selection
    }

    fn rebuild_rows(&mut self, visible_roots: &Option<Vec<usize>>, cache: &LruCache) {
        // Build row list from expanded state
    }

    fn build_rows_from_value(&mut self, value: &Value, path: &str, indent: usize) {
        // Current logic for building nested rows
    }
}

struct JsonRow {
    path: String,
    indent: usize,
    is_expandable: bool,
    is_expanded: bool,
    display_text: String,
    text_token: (TextToken, Option<TextToken>),
}
```

### 3. FileLoader Trait (for Extensibility)

**Location:** `src/file/loader_trait.rs`

```rust
pub trait FileLoader: Send + Sync {
    fn len(&self) -> usize;
    fn get(&mut self, index: usize) -> Result<CachedValue>;
    fn raw_slice(&self, index: usize) -> Result<Vec<u8>>;
}

// Implement for LazyJsonFile
impl FileLoader for LazyJsonFile {
    fn len(&self) -> usize { ... }
    fn get(&mut self, index: usize) -> Result<CachedValue> { ... }
    fn raw_slice(&self, index: usize) -> Result<Vec<u8>> { ... }
}

// Future: CsvLoader, XmlLoader, etc.
```

### 4. Shared Types

**Location:** `src/components/file_viewer/types.rs`

```rust
// Generic cached value (can hold different formats)
pub enum CachedValue {
    Json(serde_json::Value),
    // Csv(CsvRecord),      // Future
    // Xml(XmlNode),        // Future
    // Text(String),        // Future
}

// Common viewer state
pub struct ViewerState {
    pub visible_roots: Option<Vec<usize>>,
    pub selected: Option<String>,
}
```

## Migration Strategy

### Phase 1: Extract Generic Components (No Breaking Changes) ✅ COMPLETE

1. ✅ Create `src/components/file_viewer/` directory
2. ✅ Create `FileViewer` struct with current JsonViewer logic
3. ✅ Create `JsonTreeViewer` with JSON-specific logic
4. ✅ Create `types.rs` with shared ViewerState
5. ✅ All tests pass, backward compatible API maintained

### Phase 2: Update References ✅ COMPLETE

1. ✅ Update `CentralPanel` to use new `FileViewer`
2. ✅ Remove old `json_viewer.rs` facade
3. ✅ Build successful with clean architecture

### Phase 3: Add New File Types (Future) 📋 See Issue #35

1. Implement `CsvTableViewer`
2. Implement `XmlTreeViewer`
3. Implement `TextViewer` (fallback)
4. Add new `FileType` variants

**Status:** Phases 1 & 2 complete! See [Issue #35](https://github.com/anitnilay20/thoth/issues/35) for Phase 3 implementation.

## Benefits of This Approach

### ✅ Separation of Concerns

- **FileViewer**: File/data management
- **JsonTreeViewer**: JSON-specific rendering
- Each component has a single, clear responsibility

### ✅ Extensibility

- Easy to add new file formats
- Common functionality (search, selection, cache) is reused
- Format-specific logic is isolated

### ✅ Testability

- Test file loading separately from rendering
- Test JSON rendering separately from filtering
- Mock different file types easily

### ✅ Maintainability

- Smaller, focused files
- Clear boundaries between components
- Easier to understand and modify

### ✅ Future-Proof

- Adding CSV support doesn't touch JSON code
- Adding XML support doesn't touch CSV code
- Common features benefit all formats

## File Structure After Refactoring

```
src/components/
├── file_viewer/
│   ├── mod.rs                    # FileViewer (generic parent)
│   ├── types.rs                  # Shared types (CachedValue, ViewerState)
│   ├── json_tree_viewer.rs       # JSON-specific tree viewer
│   ├── csv_table_viewer.rs       # Future: CSV viewer
│   ├── xml_tree_viewer.rs        # Future: XML viewer
│   └── text_viewer.rs            # Future: Plain text viewer
├── central_panel.rs              # Uses FileViewer
├── toolbar.rs
├── settings_panel.rs
└── drag_and_drop.rs

src/file/
├── lazy_loader/
│   ├── mod.rs
│   ├── json_loader.rs            # LazyJsonFile
│   └── loader_trait.rs           # FileLoader trait
└── detect_file_type.rs
```

## Breaking Down the Work

### Step 1: Create Directory Structure

```bash
mkdir -p src/components/file_viewer
touch src/components/file_viewer/mod.rs
touch src/components/file_viewer/types.rs
touch src/components/file_viewer/json_tree_viewer.rs
```

### Step 2: Extract Common Types (15 min)

- Move `CachedValue` enum to `types.rs`
- Move `ViewerState` struct to `types.rs`
- Create re-exports

### Step 3: Create JsonTreeViewer (30 min)

- Copy JSON-specific logic from `JsonViewer`
- Extract `expanded`, `rows`, tree building
- Keep API surface minimal

### Step 4: Create FileViewer (30 min)

- Move file loading logic
- Move cache, selection, filtering
- Delegate rendering to JsonTreeViewer

### Step 5: Update Central Panel (15 min)

- Update imports
- Change `JsonViewer` to `FileViewer`
- Update method calls if needed

### Step 6: Test & Cleanup (20 min)

- Run all tests
- Update documentation
- Remove old code if using facade pattern

**Total Estimated Time: ~2 hours**

## Risks & Mitigation

### Risk 1: Breaking Existing Functionality

**Mitigation:** Use facade pattern in Phase 1, keep old API working

### Risk 2: Performance Regression

**Mitigation:** Keep hot paths identical, measure before/after

### Risk 3: Over-Engineering

**Mitigation:** Only extract what's needed now, design for future but don't implement it

## Success Criteria

- ✅ All existing functionality works (open, search, expand, select)
- ✅ Code is more modular and easier to understand
- ✅ Adding a new file type wouldn't require changing JSON code
- ✅ Tests pass
- ✅ No performance degradation

## Alternative Considered: Component Composition

Instead of a parent `FileViewer`, use composition:

```rust
struct CentralPanel {
    file_state: FileState,        // Loading, cache, filtering
    json_viewer: JsonTreeViewer,  // JSON rendering
    // csv_viewer: CsvTableViewer,
}
```

**Pros:** More flexible, no "god object"
**Cons:** `CentralPanel` becomes more complex, state management harder

**Decision:** Go with FileViewer approach for now, can switch later if needed.

## Open Questions

1. **Should cache be generic or format-specific?**
   - Recommendation: Generic `LruCache<usize, CachedValue>` for now

2. **How to handle format-specific settings?**
   - Recommendation: Each viewer has its own config struct

3. **Should viewers be created lazily or upfront?**
   - Recommendation: Create on-demand when file is opened

4. **How to share selection state between different viewers?**
   - Recommendation: Keep selection in FileViewer, pass as mutable reference

## Next Steps

1. Review this plan with the team
2. Create GitHub issue for tracking
3. Start with Phase 1 (extraction with facade)
4. Iterate based on feedback

---

**This refactoring sets up Thoth for future extensibility while keeping the current codebase clean and maintainable.**
