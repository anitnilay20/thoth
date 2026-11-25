//! Unified remote & local storage access via Apache OpenDAL (#150).
//!
//! Supports `file://`, `s3://`, `gs://`, `az://`, `http://`, and `https://`
//! URIs. Credentials are stored per backend in the OS keychain via
//! `secret_store` (in-memory under `cfg(test)`). Remote files are streamed into
//! a `NamedTempFile` (preserving the extension) so DuckDB and the existing
//! file-loading pipeline work identically to local files.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use opendal::{Operator, services};
use serde::{Deserialize, Serialize};
pub use tempfile::NamedTempFile;
use tempfile::Builder;

use crate::error::{Result, ThothError};
use crate::plugin::wasm_data_source::secret_store;

/// Supported URI schemes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteScheme {
    File,
    S3,
    Gcs,
    Azure,
    Http,
}

impl RemoteScheme {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "Local File",
            Self::S3 => "Amazon S3 / R2 / MinIO",
            Self::Gcs => "Google Cloud Storage",
            Self::Azure => "Azure Blob Storage",
            Self::Http => "HTTP / HTTPS",
        }
    }

    pub fn keyring_account(self) -> &'static str {
        match self {
            Self::File => "storage:file",
            Self::S3 => "storage:s3",
            Self::Gcs => "storage:gcs",
            Self::Azure => "storage:azure",
            Self::Http => "storage:http",
        }
    }
}

/// Parsed remote or local storage URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUri {
    pub scheme: RemoteScheme,
    /// Bucket, container, or `scheme://host` endpoint for HTTP.
    pub authority: String,
    /// Object key or path within `authority`.
    pub path: String,
    /// Original URI string.
    pub raw: String,
}

impl RemoteUri {
    /// Parse `file://...`, `s3://bucket/key`, `gs://bucket/key`,
    /// `az://container/blob`, `https://host/path`, or `http://host/path`.
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        let (scheme_str, rest) = trimmed.split_once("://").ok_or_else(|| {
            storage_err("URI must include a scheme (s3://, gs://, az://, https://, file://)")
        })?;

        let scheme = match scheme_str.to_ascii_lowercase().as_str() {
            "file" => RemoteScheme::File,
            "s3" => RemoteScheme::S3,
            "gs" => RemoteScheme::Gcs,
            "az" | "azblob" => RemoteScheme::Azure,
            "http" | "https" => RemoteScheme::Http,
            other => {
                return Err(storage_err(format!("unsupported storage scheme '{other}'")));
            }
        };

        if scheme == RemoteScheme::File {
            let path = rest.to_string();
            if path.is_empty() {
                return Err(storage_err("file:// URI is missing a path"));
            }
            return Ok(Self {
                scheme,
                authority: "/".to_string(),
                path,
                raw: trimmed.to_string(),
            });
        }

        let (authority, path) = rest
            .split_once('/')
            .ok_or_else(|| storage_err("URI must include both bucket/host and object path"))?;

        if authority.is_empty() || path.is_empty() {
            return Err(storage_err(
                "URI must include both bucket/host and object path",
            ));
        }

        let authority = if scheme == RemoteScheme::Http {
            format!("{}://{authority}", scheme_str.to_ascii_lowercase())
        } else {
            authority.to_string()
        };

        Ok(Self {
            scheme,
            authority,
            path: path.to_string(),
            raw: trimmed.to_string(),
        })
    }

    /// Extract the file extension (without leading dot), if any, so temp files
    /// keep their format hint for DuckDB and `FileType` sniffing.
    pub fn extension(&self) -> Option<&str> {
        let path_part = self.path.split(['?', '#']).next().unwrap_or(&self.path);
        Path::new(path_part).extension().and_then(|e| e.to_str())
    }
}

/// Per-backend credentials stored in the OS keychain (never plaintext config).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageCredentials {
    // S3 / R2 / MinIO
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
    pub endpoint: String,
    // GCS
    pub service_account_json: String,
    // Azure Blob
    pub account_name: String,
    pub account_key: String,
}

impl StorageCredentials {
    pub fn save_for(&self, scheme: RemoteScheme) -> Result<()> {
        let json = serde_json::to_string(self)?;
        secret_store::write(scheme.keyring_account(), &json).map_err(|reason| {
            ThothError::StateError {
                reason: format!("failed to store credentials in keychain: {reason}"),
            }
        })
    }

    pub fn load_for(scheme: RemoteScheme) -> Result<Option<Self>> {
        let raw = secret_store::read(scheme.keyring_account()).map_err(|reason| {
            ThothError::StateError {
                reason: format!("failed to read credentials from keychain: {reason}"),
            }
        })?;
        match raw {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }
}

/// Shared progress handle for background remote downloads.
#[derive(Clone, Default)]
pub struct DownloadProgress {
    pub downloaded_bytes: Arc<AtomicU64>,
    /// `0` when the backend did not report content-length.
    pub total_bytes: Arc<AtomicU64>,
    pub cancelled: Arc<AtomicBool>,
}

impl DownloadProgress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn downloaded(&self) -> u64 {
        self.downloaded_bytes.load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        self.total_bytes.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

fn storage_err(msg: impl Into<String>) -> ThothError {
    ThothError::Unknown {
        message: msg.into(),
    }
}

/// Build an OpenDAL `Operator` configured for `uri` and optional `creds`.
pub fn build_operator(uri: &RemoteUri, creds: Option<&StorageCredentials>) -> Result<Operator> {
    match uri.scheme {
        RemoteScheme::File => {
            let builder = services::Fs::default().root("/");
            Operator::new(builder).map_err(|e| storage_err(e.to_string()))
        }
        RemoteScheme::Http => {
            let builder = services::Http::default().endpoint(&uri.authority);
            Operator::new(builder).map_err(|e| storage_err(e.to_string()))
        }
        RemoteScheme::S3 => {
            let mut builder = services::S3::default().bucket(&uri.authority);
            if let Some(c) = creds {
                if !c.region.trim().is_empty() {
                    builder = builder.region(c.region.trim());
                } else {
                    builder = builder.region("us-east-1");
                }
                if !c.endpoint.trim().is_empty() {
                    builder = builder.endpoint(c.endpoint.trim());
                }
                if !c.access_key_id.trim().is_empty() {
                    builder = builder.access_key_id(c.access_key_id.trim());
                }
                if !c.secret_access_key.trim().is_empty() {
                    builder = builder.secret_access_key(c.secret_access_key.trim());
                }
            } else {
                builder = builder.region("us-east-1");
            }
            Operator::new(builder).map_err(|e| storage_err(e.to_string()))
        }
        RemoteScheme::Gcs => {
            let mut builder = services::Gcs::default().bucket(&uri.authority);
            if let Some(c) = creds
                && !c.service_account_json.trim().is_empty()
            {
                builder = builder.credential(c.service_account_json.trim());
            }
            Operator::new(builder).map_err(|e| storage_err(e.to_string()))
        }
        RemoteScheme::Azure => {
            let mut builder = services::Azblob::default().container(&uri.authority);
            if let Some(c) = creds {
                if !c.account_name.trim().is_empty() {
                    builder = builder.account_name(c.account_name.trim());
                }
                if !c.account_key.trim().is_empty() {
                    builder = builder.account_key(c.account_key.trim());
                }
            }
            Operator::new(builder).map_err(|e| storage_err(e.to_string()))
        }
    }
}

/// Stream a remote (or `file://`) object into a `NamedTempFile`, updating
/// `progress` in 64 KB chunks so large files never load into memory at once.
pub fn stream_to_temp_file(
    uri: &RemoteUri,
    creds: Option<&StorageCredentials>,
    progress: &DownloadProgress,
) -> Result<NamedTempFile> {
    let op = build_operator(uri, creds)?;
    let suffix = uri
        .extension()
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();
    let mut temp = Builder::new()
        .prefix("thoth-remote-")
        .suffix(&suffix)
        .tempfile()
        .map_err(|e| storage_err(format!("failed to create temp file: {e}")))?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| storage_err(format!("failed to start async runtime: {e}")))?;

    rt.block_on(async {
        let rel_path = uri.path.trim_start_matches('/');
        let total = match op.stat(rel_path).await {
            Ok(meta) => {
                let len = meta.content_length();
                progress.total_bytes.store(len, Ordering::Relaxed);
                len
            }
            Err(_) => 0,
        };

        let reader = op
            .reader(rel_path)
            .await
            .map_err(|e| storage_err(format!("failed to open remote object: {e}")))?;

        let mut offset = 0u64;
        const CHUNK: u64 = 64 * 1024;
        loop {
            if progress.cancelled.load(Ordering::Relaxed) {
                return Err(storage_err("remote download cancelled"));
            }
            if total > 0 && offset >= total {
                break;
            }
            let end = if total > 0 {
                (offset + CHUNK).min(total)
            } else {
                offset + CHUNK
            };
            let buf = reader
                .read(offset..end)
                .await
                .map_err(|e| storage_err(format!("failed reading remote object: {e}")))?;
            if buf.is_empty() {
                break;
            }
            let n = buf.len() as u64;
            temp.write_all(&buf.to_bytes())
                .map_err(|e| storage_err(format!("failed writing temp file: {e}")))?;
            offset += n;
            progress.downloaded_bytes.store(offset, Ordering::Relaxed);
        }
        temp.flush()
            .map_err(|e| storage_err(format!("failed flushing temp file: {e}")))?;
        Ok::<(), ThothError>(())
    })?;

    Ok(temp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_remote_uris() {
        let s3 = RemoteUri::parse("s3://my-bucket/dir/events.parquet").unwrap();
        assert_eq!(s3.scheme, RemoteScheme::S3);
        assert_eq!(s3.authority, "my-bucket");
        assert_eq!(s3.path, "dir/events.parquet");
        assert_eq!(s3.extension(), Some("parquet"));

        let gs = RemoteUri::parse("gs://data-lake/logs.ndjson").unwrap();
        assert_eq!(gs.scheme, RemoteScheme::Gcs);
        assert_eq!(gs.authority, "data-lake");
        assert_eq!(gs.path, "logs.ndjson");

        let az = RemoteUri::parse("az://container/blob.json").unwrap();
        assert_eq!(az.scheme, RemoteScheme::Azure);
        assert_eq!(az.authority, "container");
        assert_eq!(az.path, "blob.json");

        let http = RemoteUri::parse("https://example.com/data/users.csv?v=1").unwrap();
        assert_eq!(http.scheme, RemoteScheme::Http);
        assert_eq!(http.authority, "https://example.com");
        assert_eq!(http.path, "data/users.csv?v=1");
        assert_eq!(http.extension(), Some("csv"));

        assert!(RemoteUri::parse("ftp://host/file").is_err());
        assert!(RemoteUri::parse("s3://bucket-only").is_err());
    }

    #[test]
    fn credentials_round_trip_through_secret_store() {
        let creds = StorageCredentials {
            access_key_id: "AKIA_TEST".to_string(),
            secret_access_key: "SECRET_TEST".to_string(),
            region: "us-west-2".to_string(),
            ..Default::default()
        };
        creds.save_for(RemoteScheme::S3).unwrap();
        let loaded = StorageCredentials::load_for(RemoteScheme::S3)
            .unwrap()
            .expect("saved credentials");
        assert_eq!(loaded, creds);
    }

    #[test]
    fn streams_file_uri_to_temp_file_with_progress() {
        let mut src = Builder::new().suffix(".json").tempfile().unwrap();
        let payload = br#"[{"id":1,"name":"thoth"}]"#;
        src.write_all(payload).unwrap();
        src.flush().unwrap();

        let uri_str = format!("file://{}", src.path().display());
        let uri = RemoteUri::parse(&uri_str).unwrap();
        let progress = DownloadProgress::new();
        let temp = stream_to_temp_file(&uri, None, &progress).unwrap();

        assert_eq!(progress.downloaded(), payload.len() as u64);
        assert_eq!(progress.total(), payload.len() as u64);
        assert_eq!(std::fs::read(temp.path()).unwrap(), payload);
        assert_eq!(temp.path().extension().and_then(|e| e.to_str()), Some("json"));
    }
}
