/// Common types shared across file viewers
/// Viewer state that's common to all file types
#[derive(Default)]
pub struct ViewerState {
    /// Filtered root indices (e.g., from search results)
    pub visible_roots: Option<Vec<usize>>,

    /// Currently selected item path
    pub selected: Option<String>,

    /// Flag to indicate if we should scroll to the selected item on next render
    pub should_scroll_to_selection: bool,

    /// Flag to indicate if this is a large jump (search navigation) vs keyboard navigation
    pub is_search_navigation: bool,
}
