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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn an_installed_picker_receives_open_and_save_requests() {
        // This is the only test that installs the process-wide picker, so the
        // `OnceLock` is set exactly once for the core test binary.
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        set_picker(std::sync::Arc::new(|request| {
            assert_eq!(request.title, "Pick");
            assert_eq!(request.extensions, vec!["json".to_string()]);
            CALLS.fetch_add(1, Ordering::Relaxed);
            Some(PathBuf::from("/tmp/chosen.json"))
        }));

        assert_eq!(
            pick_open("Pick", &["json".to_string()]),
            Some(PathBuf::from("/tmp/chosen.json"))
        );
        // Empty default name -> `default_name` is `None`.
        assert_eq!(
            pick_save("Pick", "", &["json".to_string()]),
            Some(PathBuf::from("/tmp/chosen.json"))
        );
        // Non-empty default name -> `default_name` is `Some`.
        assert_eq!(
            pick_save("Pick", "out.json", &["json".to_string()]),
            Some(PathBuf::from("/tmp/chosen.json"))
        );
        assert_eq!(CALLS.load(Ordering::Relaxed), 3);
    }
}
