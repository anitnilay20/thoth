//! Host-mediated native file dialogs.
//!
//! Plugins can ask the host to open or save a file (`file-dialog` WIT import).
//! The desktop UI installs a picker backed by `rfd`; the headless CLI leaves it
//! unset, in which case dialog requests simply resolve to "cancelled".

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

type Picker = Arc<dyn Fn(FileDialogRequest) -> Option<PathBuf> + Send + Sync + 'static>;

/// A native open/save request raised on behalf of a plugin.
#[derive(Debug, Clone)]
pub struct FileDialogRequest {
    pub title: String,
    pub extensions: Vec<String>,
    pub save: bool,
    pub default_name: Option<String>,
}

static PICKER: OnceLock<Picker> = OnceLock::new();

/// Install the process-wide dialog picker. Ignored if already set.
pub fn set_picker(picker: Picker) {
    let _ = PICKER.set(picker);
}

/// Show an open dialog. Returns `None` when no picker is installed (headless).
pub fn pick_open(title: &str, extensions: &[String]) -> Option<PathBuf> {
    pick(FileDialogRequest {
        title: title.to_string(),
        extensions: extensions.to_vec(),
        save: false,
        default_name: None,
    })
}

/// Show a save dialog. Returns `None` when no picker is installed (headless).
pub fn pick_save(title: &str, default_name: &str, extensions: &[String]) -> Option<PathBuf> {
    pick(FileDialogRequest {
        title: title.to_string(),
        extensions: extensions.to_vec(),
        save: true,
        default_name: (!default_name.is_empty()).then(|| default_name.to_string()),
    })
}

fn pick(request: FileDialogRequest) -> Option<PathBuf> {
    PICKER.get().and_then(|picker| picker(request))
}
