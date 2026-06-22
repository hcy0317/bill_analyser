use std::path::Path;

use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::SyncConfigContract;
use chrono::Utc;
use reqwest::{header, Client};
use serde_json::Value;

use super::body_hash::{hmac_sha256, streaming_body};
use super::config::require_non_empty;
use super::endpoint::{append_url_segments, append_url_segments_from_key, endpoint_url};
use super::types::{CloudBackupUploadError, AZURE_BLOB_VERSION, CONTENT_TYPE};

/// 上传 Azure Blob 对象，校验并解码账户 key 后按 SharedKey 规则签名 BlockBlob PUT。
pub(super) async fn upload_azure(
    client: &Client,
    config: &Value,
    contract: &SyncConfigContract,
    file_path: &Path,
) -> Result<reqwest::Response, CloudBackupUploadError> {
    let container = require_non_empty(config, "bucket")?;
    let account_name = require_non_empty(config, "access_key")?;
    let account_key = require_non_empty(config, "secret_key")?;
    let decoded_key = general_purpose::STANDARD
        .decode(account_key)
        .map_err(|_| CloudBackupUploadError::config("azure secret_key must be base64"))?;
    let url = append_url_segments(endpoint_url(config, &contract.provider)?, &[container])?;
    let url = append_url_segments_from_key(url, &contract.object_key)?;
    let stat = tokio::fs::metadata(file_path)
        .await
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))?;
    let content_length = stat.len().to_string();
    let x_ms_date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let canonicalized_headers = format!(
        "x-ms-blob-type:BlockBlob\nx-ms-date:{x_ms_date}\nx-ms-version:{AZURE_BLOB_VERSION}\n"
    );
    let canonicalized_resource = format!("/{account_name}/{container}/{}", contract.object_key);
    let string_to_sign = format!(
        "PUT\n\n\n{content_length}\n\n{CONTENT_TYPE}\n\n\n\n\n\n\n{canonicalized_headers}{canonicalized_resource}"
    );
    let signature =
        general_purpose::STANDARD.encode(hmac_sha256(&decoded_key, string_to_sign.as_bytes()));
    client
        .put(url)
        .header(header::CONTENT_LENGTH, content_length)
        .header(header::CONTENT_TYPE, CONTENT_TYPE)
        .header("x-ms-blob-type", "BlockBlob")
        .header("x-ms-date", x_ms_date)
        .header("x-ms-version", AZURE_BLOB_VERSION)
        .header(
            header::AUTHORIZATION,
            format!("SharedKey {account_name}:{signature}"),
        )
        .body(streaming_body(file_path).await?)
        .send()
        .await
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))
}
