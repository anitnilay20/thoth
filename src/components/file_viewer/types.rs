/// Common types shared across file viewers
/// Viewer state that's common to all file types
#[derive(Default)]
pub struct ViewerState {
    /// Filtered root indices (e.g., from search results)
    pub visible_roots: Option<Vec<usize>>,

    /// Currently selected item path
    pub selected: Option<String>,
}
