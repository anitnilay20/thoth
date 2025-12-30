# Navigation Features Implementation Plan

## Issue: #9 - Implement advanced navigation features

## Current State Analysis

### Existing Components:
1. **Recent Files** - Already implemented in `src/components/recent_files.rs`
   - Displays last N files
   - Persisted in `PersistentState`
   - Already shown in sidebar

2. **Path Selection** - `ViewerState.selected: Option<String>`
   - Tracks currently selected JSON path (e.g., "0.user.name")
   - Used for keyboard navigation and search results
   - Supports scroll-to-selection

3. **Keyboard Shortcuts** - `src/shortcuts.rs`
   - Existing: focus_search, next_match, prev_match, expand/collapse
   - Need to add: go_to_path, bookmark_toggle, nav_back, nav_forward

4. **Persistent State** - `src/app/persistent_state.rs`
   - Already stores: recent_files, sidebar_width, sidebar_expanded
   - Can be extended for: bookmarks, navigation_history

### Architecture Patterns Observed:
- **Component Architecture**: Props/Events pattern with StatelessComponent/ContextComponent traits
- **State Management**: WindowState (per-window) + PersistentState (global, serialized)
- **Event Flow**: Components emit events → App handles events → Updates state
- **Persistence**: JSON serialization to `~/.config/thoth/`

## Implementation Strategy

### Phase 1: Navigation History (Back/Forward)

**Goal**: Track viewed paths and allow back/forward navigation

**Data Structures**:
```rust
// In WindowState (per-window, not persisted)
pub struct NavigationHistory {
    history: Vec<String>,      // Stack of visited paths
    current_index: usize,       // Current position in history
    max_history: usize,         // Limit (e.g., 100)
}
```

**Implementation Steps**:
1. Add `NavigationHistory` to `WindowState`
2. Track path changes in `FileViewer::navigate_to(path)`
3. Add `nav_back` and `nav_forward` methods to `FileViewer`
4. Add shortcuts: `Ctrl+[` (back), `Ctrl+]` (forward)
5. Handle in `shortcut_handler.rs`

**Files to Modify**:
- `src/state.rs` - Add NavigationHistory to WindowState
- `src/components/file_viewer/mod.rs` - Add back/forward methods
- `src/shortcuts.rs` - Add nav_back, nav_forward shortcuts
- `src/app/shortcut_handler.rs` - Handle new shortcuts

---

### Phase 2: Bookmarks System

**Goal**: Let users bookmark specific JSON paths for quick access

**Data Structures**:
```rust
// In PersistentState (persisted to disk)
#[derive(Serialize, Deserialize)]
pub struct Bookmark {
    pub path: String,           // JSON path (e.g., "0.user.email")
    pub file_path: String,      // File path
    pub label: Option<String>,  // Optional custom label
    pub created_at: u64,        // Timestamp
}

pub bookmarks: Vec<Bookmark>,   // In PersistentState
```

**Component**: Create `src/components/bookmarks.rs`
- Display bookmarks in sidebar (new SidebarSection)
- Events: BookmarkClicked(path), RemoveBookmark(index), AddBookmark

**Implementation Steps**:
1. Add `bookmarks` Vec to `PersistentState`
2. Create `Bookmarks` component following component architecture
3. Add `SidebarSection::Bookmarks` variant
4. Add bookmark methods to `FileViewer`
5. Add shortcut: `Ctrl+D` (toggle bookmark)
6. Handle in `shortcut_handler.rs`

**Files to Create**:
- `src/components/bookmarks.rs` - Bookmarks component

**Files to Modify**:
- `src/app/persistent_state.rs` - Add bookmarks field
- `src/components/sidebar.rs` - Add Bookmarks section
- `src/components/file_viewer/mod.rs` - Add bookmark methods
- `src/shortcuts.rs` - Add toggle_bookmark shortcut
- `src/app/shortcut_handler.rs` - Handle bookmark shortcut

---

### Phase 3: Go-to-Path Dialog

**Goal**: Ctrl+G dialog to jump to any JSON path

**Component**: Create `src/components/go_to_path_dialog.rs`
- Modal dialog with text input
- Path validation as user types
- Autocomplete suggestions based on visible paths
- Enter to navigate, Esc to cancel

**Data Structures**:
```rust
pub struct GoToPathDialog {
    open: bool,
    input: String,
    suggestions: Vec<String>,
    selected_suggestion: usize,
}

pub enum GoToPathEvent {
    NavigateToPath(String),
    Close,
}
```

**Implementation Steps**:
1. Create `GoToPathDialog` component
2. Add to `WindowState`
3. Add shortcut: `Ctrl+G` (open dialog)
4. Implement path validation (check if path exists in current file)
5. Handle events in `thoth_app.rs`

**Files to Create**:
- `src/components/go_to_path_dialog.rs` - Dialog component

**Files to Modify**:
- `src/state.rs` - Add go_to_path_dialog to WindowState
- `src/shortcuts.rs` - Add go_to_path shortcut
- `src/app/thoth_app.rs` - Render dialog, handle events
- `src/app/shortcut_handler.rs` - Open dialog on Ctrl+G

---

### Phase 4: Breadcrumb Navigation

**Goal**: Show current path at top of viewer, clickable to navigate

**Component**: Create `src/components/breadcrumbs.rs`
- Displays path segments (e.g., "Root > users > [0] > name")
- Each segment clickable to navigate to that level
- Compact display with ellipsis for long paths

**Data Structures**:
```rust
pub struct BreadcrumbsProps<'a> {
    pub current_path: Option<&'a str>,
}

pub enum BreadcrumbsEvent {
    NavigateToPath(String),
}
```

**Implementation Steps**:
1. Create `Breadcrumbs` component
2. Parse current path into segments
3. Add to `CentralPanel` above file viewer
4. Handle click events to navigate

**Files to Create**:
- `src/components/breadcrumbs.rs` - Breadcrumbs component

**Files to Modify**:
- `src/components/central_panel.rs` - Add breadcrumbs above viewer
- `src/app/thoth_app.rs` - Handle breadcrumb events

---

## Implementation Order

**Priority 1 (Core Features)**:
1. Navigation History (back/forward) - Enables basic navigation
2. Bookmarks System - Most requested feature

**Priority 2 (Enhanced UX)**:
3. Go-to-Path Dialog - Power user feature
4. Breadcrumbs - Visual navigation aid

## Testing Strategy

For each feature:
1. Unit tests for data structures (history stack, bookmarks)
2. Component tests (render without panic, event emission)
3. Integration tests (end-to-end navigation flows)
4. Manual testing with large JSON files

## Acceptance Criteria Mapping

- [x] Recent files menu - Already implemented ✓
- [ ] Bookmark system with persistence - Phase 2
- [ ] Go-to-path dialog with autocomplete - Phase 3
- [ ] Breadcrumb navigation bar - Phase 4
- [ ] Back/forward navigation history - Phase 1
- [ ] Keyboard shortcuts for all features - All phases

## Questions for User

1. **Navigation History Scope**: Should history be per-file or global across all files?
   - Recommended: Per-file (tracks positions within current file only)
   - Alternative: Global (tracks file + path combinations)

2. **Bookmarks Organization**: Should bookmarks be grouped by file or flat list?
   - Recommended: Show file path with each bookmark, searchable
   - Alternative: Group by file in collapsible sections

3. **Breadcrumb Location**: Should breadcrumbs be:
   - Option A: Above viewer in central panel (recommended)
   - Option B: In status bar at bottom
   - Option C: In toolbar at top

4. **Go-to-Path Format**: What path format should users enter?
   - Current format: "0.user.items[2].name"
   - Alternative: JSONPath syntax "$.users[0].items[2].name"
   - Recommended: Support both formats

## Risks & Mitigation

**Risk 1**: Navigation history consuming too much memory
- **Mitigation**: Limit to 100 entries, store only paths (strings are small)

**Risk 2**: Bookmarks file growing large
- **Mitigation**: Limit to 100 bookmarks per file, show warning when approaching limit

**Risk 3**: Path validation performance on large files
- **Mitigation**: Use cached expanded paths from viewer, validate asynchronously

**Risk 4**: Breadcrumb overflow with deeply nested paths
- **Mitigation**: Truncate middle segments with "...", show full path on hover

## Next Steps

1. Get user approval on plan
2. Start with Phase 1 (Navigation History) as it's foundational
3. Implement incrementally with tests
4. Get feedback after each phase before proceeding
