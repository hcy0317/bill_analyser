use std::path::Path;

use bill_analyser_core::SyncConfigContract;
use chrono::Utc;
use reqwest::{header, Client};
use serde_json::Value;

use super::body_hash::{hex_lower, hmac_sha1, sha1_hex, streaming_body};
use super::config::require_non_empty;
use super::endpoint::{cos_upload_url, endpoint_url, host_header_value};
use super::types::{CloudBackupUploadError, CONTENT_TYPE};

/// 上传腾讯云 COS 对象，按 q-sign SHA1 合同构造签名并复用 endpoint 的 bucket URL 策略。
pub(super) async fn upload_cos(
    client: &Client,
    config: &Value,
    contract: &SyncConfigContract,
    file_path: &Path,
) -> Result<reqwest::Response, CloudBackupUploadError> {
    let bucket = require_non_empty(config, "bucket")?;
    let access_key = require_non_empty(config, "access_key")?;
    let secret_key = require_non_empty(config, "secret_key")?;
    let url = cos_upload_url(
        endpoint_url(config, &contract.provider)?,
        bucket,
        &contract.object_key,
    )?;
    let host = host_header_value(&url)?;
    let now = Utc::now().timestamp();
    let sign_time = format!("{now};{}", now + 600);
    let header_list = "host";
    let canonical_request = format!("put\n{}\n\nhost={host}\n", url.path());
    let string_to_sign = format!(
        "sha1\n{sign_time}\n{}\n",
        sha1_hex(canonical_request.as_bytes())
    );
    let sign_key = hmac_sha1(secret_key.as_bytes(), sign_time.as_bytes());
    let signature = hex_lower(&hmac_sha1(&sign_key, string_to_sign.as_bytes()));
    let authorization = format!(
        "q-sign-algorithm=sha1&q-ak={access_key}&q-sign-time={sign_time}&q-key-time={sign_time}&q-header-list={header_list}&q-url-param-list=&q-signature={signature}"
    );
    client
        .put(url)
        .header(header::CONTENT_TYPE, CONTENT_TYPE)
        .header(header::HOST, host)
        .header(header::AUTHORIZATION, authorization)
        .body(streaming_body(file_path).await?)
        .send()
        .await
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))
}
