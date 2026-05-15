use std::{collections::VecDeque, sync::Arc, time::SystemTime};

// ── Data types ────────────────────────────────────────────────────────────────

/// A single permission the plugin is requesting.
#[derive(Clone)]
pub struct PermissionEntry {
    /// Phosphor glyph for the permission type (e.g. `egui_phosphor::regular::GLOBE`).
    pub icon: &'static str,
    pub label: String,
    /// Short monospace scope hint shown next to the label.
    pub scope: String,
    /// Whether this permission is considered high-sensitivity.
    pub sensitive: bool,
}

/// Structured description of one consent request, passed to the modal for rendering.
#[derive(Clone)]
pub struct ConsentRequest {
    /// Queue key — passed to `ConsentManager::resolve` after the user decides.
    pub id: String,
    pub title: String,
    pub message: String,
    pub permissions: Vec<PermissionEntry>,
    /// The domain being requested (set for HTTP consent requests).
    pub domain: Option<String>,
    /// The plugin that raised this request (set for HTTP consent requests).
    pub plugin_id: Option<String>,
}

/// A pending consent request: display data + allow/deny callbacks.
/// `bool` is `true` when the user checked "Remember this choice".
pub type ConsentCallback = Arc<dyn Fn(bool) + Send + Sync + 'static>;

pub struct PendingConsent {
    pub request: ConsentRequest,
    pub on_allow: ConsentCallback,
    pub on_deny: ConsentCallback,
}

// ── Manager ───────────────────────────────────────────────────────────────────

/// Queue for permission consent requests. Completely independent of
/// `NotificationManager` — separate storage, separate global, separate UI.
#[derive(Default)]
pub struct ConsentManager {
    pub(super) queue: VecDeque<PendingConsent>,
}

impl ConsentManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue a network-access consent request built from a domain name.
    pub fn push_http_consent(
        domain: &str,
        plugin_id: &str,
        on_allow: ConsentCallback,
        on_deny: ConsentCallback,
    ) {
        let id = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        Self::push(PendingConsent {
            request: ConsentRequest {
                id,
                title: "Network Request Consent".to_string(),
                message: format!("A plugin is requesting access to '{domain}'."),
                permissions: vec![PermissionEntry {
                    icon: egui_phosphor::regular::GLOBE,
                    label: "Make network requests".to_string(),
                    scope: format!("to {domain}"),
                    sensitive: false,
                }],
                domain: Some(domain.to_string()),
                plugin_id: Some(plugin_id.to_string()),
            },
            on_allow,
            on_deny,
        });
    }

    /// Enqueue any consent request.
    pub fn push(consent: PendingConsent) {
        if let Some(mutex) = crate::CONSENT_MANAGER.get() {
            if let Ok(mut cm) = mutex.lock() {
                cm.queue.push_back(consent);
            }
        }
    }

    /// Remove the consent with the given id (call after the user decides).
    pub fn resolve(id: &str) {
        if let Some(mutex) = crate::CONSENT_MANAGER.get() {
            if let Ok(mut cm) = mutex.lock() {
                cm.queue.retain(|c| c.request.id != id);
            }
        }
    }

    /// Clone the first pending consent's data so the caller can render without
    /// holding the lock across frames.
    pub fn take_first() -> Option<(ConsentRequest, ConsentCallback, ConsentCallback)> {
        crate::CONSENT_MANAGER
            .get()
            .and_then(|m| m.lock().ok())
            .and_then(|cm| {
                cm.queue.front().map(|c| {
                    (
                        c.request.clone(),
                        Arc::clone(&c.on_allow),
                        Arc::clone(&c.on_deny),
                    )
                })
            })
    }
}
