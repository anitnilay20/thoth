pub mod notification_dropdown;

use eframe::egui::{self};
use egui_notify::Toasts;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};

use crate::NOTIFICATION_MANAGER;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationStatus {
    Running,
    Completed,
    Created,
    Viewed,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationKind {
    Success,
    Error,
    Warn,
    Update,
    Plugin,
    Tip,
    #[default]
    Info,
}

#[derive(Clone)]
pub struct Notification {
    pub id: String,
    /// Creation time as Unix epoch milliseconds. Independent of `id` so callers
    /// can override `id` via `with_id()` without breaking time-based display.
    pub created_at: i64,
    pub title: String,
    pub message: String,
    pub actions: Vec<(String, Arc<dyn Fn() + Send + Sync + 'static>)>,
    pub expire_after: Option<Duration>,
    pub show_toast: bool,
    pub show_in_status_bar: bool,
    pub status: NotificationStatus,
    pub kind: NotificationKind,
    pub unread: bool,
    pub pinned: bool,
}

pub struct NotificationManager {
    pub toasts: Toasts,
    pub notifications: HashMap<String, Notification>,
    pub tasks: HashMap<String, Notification>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            toasts: Toasts::default().with_anchor(egui_notify::Anchor::BottomRight),
            notifications: HashMap::new(),
            tasks: HashMap::new(),
        }
    }

    pub fn notify(notification: Notification) -> String {
        let id = notification.id.clone();

        if let Some(mutex) = NOTIFICATION_MANAGER.get()
            && let Ok(mut nm) = mutex.lock() {
                nm.add_notification(notification);
            }

        id
    }

    pub fn notify_error(notification: Notification) -> String {
        Self::notify(
            notification
                .with_status(NotificationStatus::Error)
                .with_kind(NotificationKind::Error)
                .with_toast(true),
        )
    }

    pub fn mark_notification_as_complete(id: &str) {
        if let Some(mutex) = NOTIFICATION_MANAGER.get()
            && let Ok(mut nm) = mutex.lock() {
                nm.move_to_notifications(id);
            }
    }

    pub fn remove_notification(id: &str) {
        if let Some(mutex) = NOTIFICATION_MANAGER.get()
            && let Ok(mut nm) = mutex.lock() {
                nm.notifications.remove(id);
            }
    }

    pub fn all_running_notifications_tasks() -> Vec<Notification> {
        NOTIFICATION_MANAGER
            .get()
            .and_then(|mutex| mutex.lock().ok())
            .map(|nm| nm.tasks.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn is_notification_empty() -> bool {
        NOTIFICATION_MANAGER
            .get()
            .and_then(|mutex| mutex.lock().ok())
            .map(|nm| nm.notifications.is_empty())
            .unwrap_or(true)
    }

    pub fn add_notification(&mut self, notification: Notification) {
        if notification.status == NotificationStatus::Running || notification.show_in_status_bar {
            self.tasks
                .insert(notification.id.clone(), notification.clone());
        } else {
            self.notifications
                .insert(notification.id.clone(), notification.clone());
        }

        if notification.show_toast {
            self.toasts
                .info(notification.title.clone())
                .closable(true)
                .duration(Duration::from_secs(5));
        }
    }

    pub fn show_notifications(&mut self, ctx: &egui::Context) {
        self.toasts.show(ctx);
    }

    pub fn clear_notifications(&mut self) {
        self.notifications.retain(|_, n| n.pinned);
        self.toasts.dismiss_all_toasts();
    }

    pub fn mark_all_read(&mut self) {
        for n in self.notifications.values_mut() {
            n.unread = false;
            if n.status == NotificationStatus::Created {
                n.status = NotificationStatus::Viewed;
            }
        }
    }

    pub fn unread_count(&self) -> usize {
        self.notifications.values().filter(|n| n.unread).count()
    }

    pub fn move_to_notifications(&mut self, id: &str) {
        if let Some(mut notification) = self.tasks.remove(id) {
            notification.status = NotificationStatus::Completed;
            self.notifications.insert(id.to_string(), notification);
        }
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Notification {
    pub fn new(title: &str, message: &str) -> Self {
        let now_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self {
            id: now_ms.to_string(),
            created_at: now_ms,
            title: title.to_string(),
            message: message.to_string(),
            actions: Vec::new(),
            expire_after: None,
            show_toast: true,
            show_in_status_bar: false,
            status: NotificationStatus::Created,
            kind: NotificationKind::default(),
            unread: true,
            pinned: false,
        }
    }

    pub fn with_kind(mut self, kind: NotificationKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_action(
        mut self,
        label: &str,
        action: Arc<dyn Fn() + Send + Sync + 'static>,
    ) -> Self {
        self.actions.push((label.to_string(), action));
        self
    }

    pub fn with_expiration(mut self, duration: Duration) -> Self {
        self.expire_after = Some(duration);
        self
    }

    pub fn with_toast(mut self, show: bool) -> Self {
        self.show_toast = show;
        self
    }

    pub fn with_status_bar(mut self, show: bool) -> Self {
        self.show_in_status_bar = show;
        self
    }

    pub fn with_status(mut self, status: NotificationStatus) -> Self {
        self.status = status;
        self
    }

    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }
}
