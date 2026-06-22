use std::path::Path;

use bill_analyser_core::SyncConfigContract;
use chrono::Utc;
use reqwest::{header, Client};
use serde_json::Value;

use super::body_hash::{
    aws_signing_key, hex_lower, hmac_sha256, sha256_file_hex, sha256_hex, streaming_body,
};
use super::config::{non_empty_config, require_non_empty};
use super::endpoint::{
    append_url_segments, append_url_segments_from_key, endpoint_url, host_header_value,
    region_from_s3_endpoint,
};
use super::types::{CloudBackupUploadError, CONTENT_TYPE};

/// 上传 S3 对象，按 AWS Signature V4 构造 canonical request、payload hash 和 Authorization。
pub(super) async fn upload_s3(
    client: &Client,
    config: &Value,
    contract: &SyncConfigContract,
    file_path: &Path,
) -> Result<reqwest::Response, CloudBackupUploadError> {
    let bucket = require_non_empty(config, "bucket")?;
    let access_key = require_non_empty(config, "access_key")?;
    let secret_key = require_non_empty(config, "secret_key")?;
    let region = non_empty_config(config, "region").unwrap_or_else(|| {
        region_from_s3_endpoint(config).unwrap_or_else(|| "us-east-1".to_string())
    });
    let url = append_url_segments(endpoint_url(config, &contract.provider)?, &[bucket])?;
    let url = append_url_segments_from_key(url, &contract.object_key)?;
    let host = host_header_value(&url)?;
    let payload_hash = sha256_file_hex(file_path).await?;
    let now = Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let scope_date = now.format("%Y%m%d").to_string();
    let scope = format!("{scope_date}/{region}/s3/aws4_request");
    let signed_headers = "content-type;host;x-amz-content-sha256;x-amz-date";
    let canonical_headers = format!(
        "content-type:{CONTENT_TYPE}\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
    );
    let canonical_request = format!(
        "PUT\n{}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
        url.path()
    );
    let canonical_hash = sha256_hex(canonical_request.as_bytes());
    let string_to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{canonical_hash}");
    let signing_key = aws_signing_key(secret_key, &scope_date, &region, "s3");
    let signature = hex_lower(&hmac_sha256(&signing_key, string_to_sign.as_bytes()));
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );
    client
        .put(url)
        .header(header::CONTENT_TYPE, CONTENT_TYPE)
        .header(header::HOST, host)
        .header("x-amz-date", amz_date)
        .header("x-amz-content-sha256", payload_hash)
        .header(header::AUTHORIZATION, authorization)
        .body(streaming_body(file_path).await?)
        .send()
        .await
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))
}
