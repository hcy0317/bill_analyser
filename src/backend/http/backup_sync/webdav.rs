use std::path::Path;

use bill_analyser_core::SyncConfigContract;
use reqwest::{header, Client, RequestBuilder};
use serde_json::Value;

use super::body_hash::streaming_body;
use super::config::string_config;
use super::endpoint::{append_url_segments, endpoint_url};
use super::types::{CloudBackupUploadError, CONTENT_TYPE};

/// 上传 WebDAV 备份对象，先按 prefix 逐级 MKCOL，再用 PUT 写入最终对象。
pub(super) async fn upload_webdav(
    client: &Client,
    config: &Value,
    contract: &SyncConfigContract,
    file_path: &Path,
) -> Result<reqwest::Response, CloudBackupUploadError> {
    let endpoint = endpoint_url(config, &contract.provider)?;
    let access_key = string_config(config, "access_key");
    let secret_key = string_config(config, "secret_key");
    let prefix_segments = contract
        .prefix
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    for index in 0..prefix_segments.len() {
        let url = append_url_segments(endpoint.clone(), &prefix_segments[..=index])?;
        let method = reqwest::Method::from_bytes(b"MKCOL")
            .map_err(|error| CloudBackupUploadError::provider(error.to_string()))?;
        let request = client.request(method, url);
        let response = apply_optional_basic_auth(request, &access_key, &secret_key)
            .send()
            .await
            .map_err(|error| CloudBackupUploadError::provider(error.to_string()))?;
        if !matches!(response.status().as_u16(), 200 | 201 | 204 | 405 | 409) {
            return Err(CloudBackupUploadError::provider_status(
                "webdav",
                response.status(),
            ));
        }
    }

    let url = append_url_segments(
        endpoint,
        &contract
            .object_key
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>(),
    )?;
    let body = streaming_body(file_path).await?;
    let request = client
        .put(url)
        .header(header::CONTENT_TYPE, CONTENT_TYPE)
        .body(body);
    apply_optional_basic_auth(request, &access_key, &secret_key)
        .send()
        .await
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))
}

/// 只有用户配置 access_key/secret_key 时才附加 Basic Auth，保持匿名 WebDAV endpoint 可用。
fn apply_optional_basic_auth(
    request: RequestBuilder,
    access_key: &str,
    secret_key: &str,
) -> RequestBuilder {
    if access_key.is_empty() && secret_key.is_empty() {
        request
    } else {
        request.basic_auth(access_key.to_string(), Some(secret_key.to_string()))
    }
}
