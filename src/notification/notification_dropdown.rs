use eframe::egui::{self, Align2, CornerRadius, Layout};

use crate::{
    components::{
        button::{Button, ButtonColor, ButtonProps},
        icon_button::{IconButton, IconButtonProps},
        list::{List, ListItem, ListProps},
        traits::{ContextComponent, StatelessComponent},
    },
    notification::{Notification, NotificationManager, NotificationStatus},
};

#[derive(Default)]
pub struct NotificationDropdown {
    state: NotificationDropdownState,
}

#[derive(Default)]
pub struct NotificationDropdownState {
    is_open: bool,
}

pub struct NotificationDropdownProps {
    // Define any properties needed for the dropdown
}

impl ContextComponent for NotificationDropdown {
    type Props<'a> = NotificationDropdownProps;
    type Output = ();

    fn render(&mut self, ui: &mut egui::Ui, _props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let notification_manager = crate::NOTIFICATION_MANAGER.get();
        let notification_empty = notification_manager
            .and_then(|mutex| mutex.lock().ok())
            .map(|nm| nm.notifications.is_empty())
            .unwrap_or(true);

        let badge_color = if !notification_empty {
            Some(colors.error)
        } else {
            None
        };

        let button_output = IconButton::render(
            ui,
            IconButtonProps {
                icon: egui_phosphor::regular::BELL,
                tooltip: Some("Notifications"),
                frame: false,
                badge_color,
                size: None,
                disabled: false,
                icon_size: None,
                selected: false,
            },
        );

        if button_output.clicked {
            self.state.is_open = !self.state.is_open;
        }

        if self.state.is_open {
            // Render the dropdown menu
            egui::Window::new("Notifications")
                .frame(egui::Frame::window(&ui.ctx().global_style()).inner_margin(0))
                .title_bar(false)
                .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -28.0))
                .resizable(false)
                .collapsible(false)
                .movable(false)
                .min_height(25.0)
                .scroll([false, true])
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .inner_margin(egui::Margin::same(8))
                        .corner_radius(CornerRadius::same(4))
                        .fill(colors.surface_raised)
                        .stroke(ui.style().visuals.window_stroke())
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());

                            ui.horizontal(|ui| {
                                ui.heading("Notifications");
                                ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                                    let close_button = IconButton::render(
                                        ui,
                                        IconButtonProps {
                                            icon: egui_phosphor::regular::CARET_DOWN,
                                            tooltip: Some("Close notifications"),
                                            frame: false,
                                            badge_color: None,
                                            size: None,
                                            disabled: false,
                                            icon_size: None,
                                            selected: false,
                                        },
                                    );
                                    if let Some(mutex) = notification_manager {
                                        if let Ok(mut nm) = mutex.lock() {
                                            let clear_button = IconButton::render(
                                                ui,
                                                IconButtonProps {
                                                    icon: egui_phosphor::regular::X,
                                                    tooltip: Some("Clear notifications"),
                                                    frame: false,
                                                    badge_color: None,
                                                    size: None,
                                                    disabled: nm.notifications.is_empty(),
                                                    selected: false,
                                                    icon_size: None,
                                                },
                                            );

                                            if clear_button.clicked {
                                                nm.clear_notifications();
                                            }
                                        }
                                    }

                                    if close_button.clicked {
                                        self.state.is_open = false;
                                    }
                                });
                            });
                        });

                    // Collect notification data while holding the lock briefly
                    let notifications: Vec<(String, String, String, NotificationStatus)> =
                        notification_manager
                            .and_then(|m| m.lock().ok())
                            .map(|nm| {
                                nm.notifications
                                    .iter()
                                    .map(|(id, n)| {
                                        (id.clone(), n.title.clone(), n.message.clone(), n.status)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();

                    let items: Vec<ListItem> = notifications
                        .iter()
                        .map(|(_, title, message, status)| {
                            let (icon, color) = match status {
                                NotificationStatus::Error => {
                                    (egui_phosphor::regular::WARNING, colors.error)
                                }
                                NotificationStatus::Completed => {
                                    (egui_phosphor::regular::CHECK, colors.success)
                                }
                                NotificationStatus::Running => {
                                    (egui_phosphor::regular::CLOCK, colors.warning)
                                }
                                _ => (egui_phosphor::regular::INFO, colors.info),
                            };
                            ListItem {
                                title,
                                description: if message.is_empty() {
                                    None
                                } else {
                                    Some(message.as_str())
                                },
                                prefix: Some(
                                    crate::components::common::list::ListItemPrefix::Icon {
                                        glyph: icon,
                                        color: Some(color),
                                    },
                                ),
                                badge: None,
                                postfix: None,
                                selected: false,
                                tags: &[],
                            }
                        })
                        .collect();

                    ui.set_min_width(280.0);
                    let _list_out = List::render(
                        ui,
                        ListProps {
                            items: &items,
                            empty_label: Some("No notifications"),
                            shrink_to_fit: false,
                            show_separators: true,
                            compact: false,
                        },
                    );
                });
        }

        let running_tasks: Vec<Notification> =
            NotificationManager::all_running_notifications_tasks();

        for task in running_tasks {
            ui.add(egui::Spinner::new().size(16.0).color(colors.warning));
            ui.label(&task.title);
        }

        let open_consent_notifications: Vec<Notification> =
            NotificationManager::all_consent_notifications();

        if !open_consent_notifications.is_empty() {
            egui::Window::new("Notifications - Consent")
                .frame(egui::Frame::window(&ui.ctx().global_style()).inner_margin(0))
                .title_bar(false)
                .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -28.0))
                .resizable(false)
                .collapsible(false)
                .movable(false)
                .min_height(25.0)
                .scroll([false, true])
                .show(ui.ctx(), |ui| {
                    ui.set_min_width(300.0);
                    ui.heading("Action Required");

                    let mut clicked: Option<(
                        String,
                        std::sync::Arc<dyn Fn() + Send + Sync + 'static>,
                    )> = None;

                    for n in &open_consent_notifications {
                        ui.horizontal(|ui| {
                            ui.colored_label(colors.warning, egui_phosphor::regular::WARNING);
                            ui.vertical(|ui| {
                                ui.label(&n.title);
                                if !n.message.is_empty() {
                                    ui.label(egui::RichText::new(&n.message).weak().small());
                                }
                            });
                        });
                        ui.horizontal(|ui| {
                            for (i, (label, callback)) in n.actions.iter().enumerate() {
                                let color = if i == 0 {
                                    ButtonColor::Primary
                                } else {
                                    ButtonColor::Default
                                };
                                let btn = Button::render(
                                    ui,
                                    ButtonProps {
                                        label: label.clone(),
                                        color,
                                        ..Default::default()
                                    },
                                );
                                if btn.clicked && clicked.is_none() {
                                    clicked = Some((n.id.clone(), callback.clone()));
                                }
                            }
                        });
                        ui.separator();
                    }

                    if let Some((id, callback)) = clicked {
                        callback();
                        NotificationManager::mark_notification_as_complete(&id);
                    }
                });
        }
    }
}
