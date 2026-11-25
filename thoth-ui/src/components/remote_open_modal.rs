//! Modal for opening remote files via OpenDAL (`s3://`, `gs://`, `az://`,
//! `https://`, `file://`) with OS-keychain credential persistence and
//! streaming download progress (#150).

use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use thoth_core::storage::{
    DownloadProgress, NamedTempFile, RemoteScheme, RemoteUri, StorageCredentials,
    stream_to_temp_file,
};
use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonType, Input, Modal, Progress, Spinner, Typography,
    TypographyVariant,
};

struct ActiveDownload {
    progress: DownloadProgress,
    rx: Receiver<thoth_core::error::Result<NamedTempFile>>,
}

#[derive(Default)]
pub struct RemoteOpenModal {
    pub open: bool,
    uri_input: String,
    loaded_scheme: Option<RemoteScheme>,
    creds: StorageCredentials,
    error: Option<String>,
    download: Option<ActiveDownload>,
}

fn text_field(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    placeholder: &str,
    password: bool,
) {
    let mut input = Input::builder()
        .id(id)
        .value(value.clone())
        .placeholder(placeholder)
        .password(password)
        .rows(1)
        .build();
    if input.show(ui).inner {
        *value = input.value;
    }
}

impl RemoteOpenModal {
    pub fn show_dialog(&mut self) {
        self.open = true;
        self.error = None;
    }

    /// Render the modal and return `Some(NamedTempFile)` when a remote download
    /// completes.
    pub fn render(&mut self, ui: &mut egui::Ui) -> Option<NamedTempFile> {
        if let Some(dl) = &self.download {
            match dl.rx.try_recv() {
                Ok(Ok(temp)) => {
                    self.download = None;
                    self.open = false;
                    self.error = None;
                    return Some(temp);
                }
                Ok(Err(e)) => {
                    self.download = None;
                    self.error = Some(e.to_string());
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.download = None;
                    self.error = Some("remote download worker exited unexpectedly".to_string());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if !self.open {
            return None;
        }

        let detected_scheme = RemoteUri::parse(&self.uri_input).ok().map(|u| u.scheme);
        if detected_scheme != self.loaded_scheme {
            self.loaded_scheme = detected_scheme;
            if let Some(scheme) = detected_scheme
                && let Ok(Some(saved)) = StorageCredentials::load_for(scheme)
            {
                self.creds = saved;
            }
        }

        let mut start_download = false;
        let mut cancel_download = false;
        let mut close_modal = false;
        let is_downloading = self.download.is_some();

        let modal = Modal::builder()
            .id("remote_open_modal")
            .title("Open Remote File")
            .eyebrow("OPENDAL STORAGE")
            .glyph(egui_phosphor::regular::CLOUD_ARROW_DOWN)
            .glyph_tile(true)
            .dismissible(!is_downloading)
            .open(true)
            .width(500.0)
            .build();

        let _ = modal.show_with_footer(
            ui,
            |ui| {
                ui.add(
                    Typography::builder()
                        .text("Enter an s3://, gs://, az://, https://, or file:// URI")
                        .variant(TypographyVariant::BodyMuted)
                        .build(),
                );
                ui.add_space(6.0);
                text_field(
                    ui,
                    "remote_uri_input",
                    &mut self.uri_input,
                    "s3://bucket/path/data.parquet",
                    false,
                );

                match detected_scheme {
                    Some(RemoteScheme::S3) => {
                        ui.add_space(10.0);
                        ui.add(
                            Typography::builder()
                                .text("S3 / R2 / MinIO Credentials (saved in OS keychain)")
                                .variant(TypographyVariant::BodyMuted)
                                .build(),
                        );
                        ui.add_space(4.0);
                        text_field(
                            ui,
                            "s3_access_key",
                            &mut self.creds.access_key_id,
                            "Access Key ID",
                            false,
                        );
                        ui.add_space(4.0);
                        text_field(
                            ui,
                            "s3_secret_key",
                            &mut self.creds.secret_access_key,
                            "Secret Access Key",
                            true,
                        );
                        ui.add_space(4.0);
                        text_field(
                            ui,
                            "s3_region",
                            &mut self.creds.region,
                            "Region (e.g. us-east-1)",
                            false,
                        );
                        ui.add_space(4.0);
                        text_field(
                            ui,
                            "s3_endpoint",
                            &mut self.creds.endpoint,
                            "Custom Endpoint (optional, for R2/MinIO)",
                            false,
                        );
                    }
                    Some(RemoteScheme::Gcs) => {
                        ui.add_space(10.0);
                        ui.add(
                            Typography::builder()
                                .text("GCS Service Account JSON (saved in OS keychain)")
                                .variant(TypographyVariant::BodyMuted)
                                .build(),
                        );
                        ui.add_space(4.0);
                        text_field(
                            ui,
                            "gcs_sa_json",
                            &mut self.creds.service_account_json,
                            "Service Account JSON (optional for public buckets)",
                            true,
                        );
                    }
                    Some(RemoteScheme::Azure) => {
                        ui.add_space(10.0);
                        ui.add(
                            Typography::builder()
                                .text("Azure Blob Credentials (saved in OS keychain)")
                                .variant(TypographyVariant::BodyMuted)
                                .build(),
                        );
                        ui.add_space(4.0);
                        text_field(
                            ui,
                            "az_account",
                            &mut self.creds.account_name,
                            "Storage Account Name",
                            false,
                        );
                        ui.add_space(4.0);
                        text_field(
                            ui,
                            "az_key",
                            &mut self.creds.account_key,
                            "Account Key",
                            true,
                        );
                    }
                    _ => {}
                }

                if let Some(dl) = &self.download {
                    ui.add_space(12.0);
                    let downloaded = dl.progress.downloaded();
                    let total = dl.progress.total();
                    if total > 0 {
                        let frac = (downloaded as f64 / total as f64).clamp(0.0, 1.0);
                        ui.add(Progress::builder().value(frac).build());
                        ui.add_space(4.0);
                        ui.add(
                            Typography::builder()
                                .text(format!("Downloaded {downloaded} / {total} bytes"))
                                .variant(TypographyVariant::BodyMuted)
                                .build(),
                        );
                    } else {
                        ui.horizontal(|ui| {
                            ui.add(Spinner::builder().build());
                            ui.add(
                                Typography::builder()
                                    .text(format!("Downloading… {downloaded} bytes"))
                                    .variant(TypographyVariant::BodyMuted)
                                    .build(),
                            );
                        });
                    }
                }

                if let Some(err) = &self.error {
                    ui.add_space(8.0);
                    ui.add(
                        Typography::builder()
                            .text(err.as_str())
                            .color("error")
                            .build(),
                    );
                }
            },
            |ui| {
                ui.horizontal(|ui| {
                    if is_downloading {
                        if ui
                            .add(
                                Button::builder()
                                    .label("Cancel Download")
                                    .button_type(ButtonType::Elevated)
                                    .color(ButtonColor::Danger)
                                    .build(),
                            )
                            .clicked()
                        {
                            cancel_download = true;
                        }
                    } else {
                        if ui
                            .add(
                                Button::builder()
                                    .label("Cancel")
                                    .button_type(ButtonType::Text)
                                    .color(ButtonColor::Default)
                                    .build(),
                            )
                            .clicked()
                        {
                            close_modal = true;
                        }
                        if ui
                            .add(
                                Button::builder()
                                    .label("Open")
                                    .button_type(ButtonType::Elevated)
                                    .color(ButtonColor::Primary)
                                    .build(),
                            )
                            .clicked()
                        {
                            start_download = true;
                        }
                    }
                });
            },
        );

        if cancel_download {
            if let Some(dl) = &self.download {
                dl.progress.cancel();
            }
            self.download = None;
        } else if close_modal {
            self.open = false;
            self.error = None;
        } else if start_download {
            match RemoteUri::parse(&self.uri_input) {
                Ok(uri) => {
                    self.error = None;
                    let has_creds = match uri.scheme {
                        RemoteScheme::S3 => {
                            !self.creds.access_key_id.trim().is_empty()
                                || !self.creds.secret_access_key.trim().is_empty()
                                || !self.creds.endpoint.trim().is_empty()
                        }
                        RemoteScheme::Gcs => !self.creds.service_account_json.trim().is_empty(),
                        RemoteScheme::Azure => !self.creds.account_name.trim().is_empty(),
                        RemoteScheme::File | RemoteScheme::Http => false,
                    };
                    if has_creds {
                        let _ = self.creds.save_for(uri.scheme);
                    }
                    let creds = has_creds.then(|| self.creds.clone());
                    let progress = DownloadProgress::new();
                    let worker_progress = progress.clone();
                    let (tx, rx) = mpsc::channel();
                    std::thread::spawn(move || {
                        let res = stream_to_temp_file(&uri, creds.as_ref(), &worker_progress);
                        let _ = tx.send(res);
                        thoth_core::repaint::request();
                    });
                    self.download = Some(ActiveDownload { progress, rx });
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                }
            }
        }

        None
    }
}
