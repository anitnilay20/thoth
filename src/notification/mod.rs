pub mod notification_dropdown;

use eframe::egui::{self};
use egui_notify::Toasts;
use std::{
    collections::HashMap,
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

#[derive(Clone)]
pub struct Notification {
    id: String,
    pub title: String,
    pub message: String,
    // pub actions: Vec<(String, Box<dyn NewTrait>)>,
    pub expire_after: Option<Duration>,
    pub show_toast: bool,
    pub show_in_status_bar: bool,
    pub status: NotificationStatus,
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

        if let Some(mutex) = NOTIFICATION_MANAGER.get() {
            if let Ok(mut nm) = mutex.lock() {
                nm.add_notification(notification);
            }
        }

        id
    }

    pub fn mark_notification_as_complete(id: &str) {
        if let Some(mutex) = NOTIFICATION_MANAGER.get() {
            if let Ok(mut nm) = mutex.lock() {
                nm.move_to_notifications(id);
            }
        }
    }

    pub fn remove_notification(id: &str) {
        if let Some(mutex) = NOTIFICATION_MANAGER.get() {
            if let Ok(mut nm) = mutex.lock() {
                nm.notifications.remove(id);
            }
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
        if notification.status == NotificationStatus::Running {
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
        self.notifications.clear();
        self.toasts.dismiss_all_toasts();
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
        Self {
            id: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string(),
            title: title.to_string(),
            message: message.to_string(),
            // actions: Vec::new(),
            expire_after: None,
            show_toast: true,
            show_in_status_bar: false,
            status: NotificationStatus::Created,
        }
    }

    // pub fn with_action(mut self, label: &str, action: impl Fn() + Send + Sync + 'static) -> Self {
    //     self.actions.push((label.to_string(), Box::new(action)));
    //     self
    // }

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
}
