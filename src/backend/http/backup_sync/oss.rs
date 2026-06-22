use std::path::Path;

use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::SyncConfigContract;
use chrono::Utc;
use reqwest::{header, Client};
use ring::hmac;
use serde_json::Value;

use super::body_hash::streaming_body;
use super::config::require_non_empty;
use super::endpoint::{append_url_segments, append_url_segments_from_key, endpoint_url};
use super::types::{CloudBackupUploadError, CONTENT_TYPE};

/// 上传阿里云 OSS 对象，保持当前 Date、CanonicalResource 和 OSS HMAC-SHA1 签名合同。
pub(super) async fn upload_oss(
    client: &Client,
    config: &Value,
    contract: &SyncConfigContract,
    file_path: &Path,
) -> Result<reqwest::Response, CloudBackupUploadError> {
    let bucket = require_non_empty(config, "bucket")?;
    let access_key = require_non_empty(config, "access_key")?;
    let secret_key = require_non_empty(config, "secret_key")?;
    let url = append_url_segments(endpoint_url(config, &contract.provider)?, &[bucket])?;
    let url = append_url_segments_from_key(url, &contract.object_key)?;
    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let canonical_resource = format!("/{bucket}/{}", contract.object_key);
    let string_to_sign = format!("PUT\n\n{CONTENT_TYPE}\n{date}\n{canonical_resource}");
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret_key.as_bytes());
    let signature = general_purpose::STANDARD.encode(hmac::sign(&key, string_to_sign.as_bytes()));
    client
        .put(url)
        .header(header::CONTENT_TYPE, CONTENT_TYPE)
        .header(header::DATE, date)
        .header(
            header::AUTHORIZATION,
            format!("OSS {access_key}:{signature}"),
        )
        .body(streaming_body(file_path).await?)
        .send()
        .await
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))
}
