// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::{env, net::IpAddr, path::Path, time::Duration};

use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::SyncConfigContract;
use chrono::Utc;
use reqwest::{header, Client, RequestBuilder, StatusCode};
use ring::hmac;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;
use url::Url;

const CONTENT_TYPE: &str = "application/octet-stream";
const AZURE_BLOB_VERSION: &str = "2023-11-03";
const BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV: &str = "BILL_ANALYSER_BACKUP_SYNC_ENDPOINT_ALLOWLIST";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudBackupUploadResult {
    pub provider: String,
    pub object_key: String,
    pub status_code: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudBackupUploadError {
    pub status_code: u16,
    pub message: String,
    pub response_status: Option<u16>,
}

impl CloudBackupUploadError {
    fn config(message: impl ToString) -> Self {
        Self {
            status_code: 400,
            message: message.to_string(),
            response_status: None,
        }
    }

    fn provider(message: impl ToString) -> Self {
        Self {
            status_code: 502,
            message: message.to_string(),
            response_status: None,
        }
    }

    fn provider_status(provider: &str, status: StatusCode) -> Self {
        Self {
            status_code: 502,
            message: format!("{provider} upload failed with status {}", status.as_u16()),
            response_status: Some(status.as_u16()),
        }
    }
}

pub fn validate_sync_upload_config(
    config: &Value,
    contract: &SyncConfigContract,
) -> Result<(), CloudBackupUploadError> {
    if contract.provider.trim().is_empty() {
        return Err(CloudBackupUploadError::config("provider is required"));
    }
    if !contract.supported {
        return Err(CloudBackupUploadError::config(format!(
            "Unsupported backup sync provider: {}",
            contract.provider
        )));
    }
    let endpoint = endpoint_url(config, &contract.provider)?;
    match contract.provider.as_str() {
        "webdav" => {
            let _ = endpoint;
        }
        "oss" | "s3" | "cos" => {
            require_non_empty(config, "bucket")?;
            require_non_empty(config, "access_key")?;
            require_non_empty(config, "secret_key")?;
        }
        "azure" => {
            require_non_empty(config, "bucket")?;
            require_non_empty(config, "access_key")?;
            let secret_key = require_non_empty(config, "secret_key")?;
            general_purpose::STANDARD
                .decode(secret_key)
                .map_err(|_| CloudBackupUploadError::config("azure secret_key must be base64"))?;
        }
        _ => {
            return Err(CloudBackupUploadError::config(format!(
                "Unsupported backup sync provider: {}",
                contract.provider
            )));
        }
    }
    Ok(())
}

pub async fn upload_backup_to_cloud(
    config: &Value,
    contract: &SyncConfigContract,
    file_path: &Path,
    timeout: Duration,
) -> Result<CloudBackupUploadResult, CloudBackupUploadError> {
    validate_sync_upload_config(config, contract)?;
    let client = Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))?;
    let response = match contract.provider.as_str() {
        "webdav" => upload_webdav(&client, config, contract, file_path).await?,
        "oss" => upload_oss(&client, config, contract, file_path).await?,
        "s3" => upload_s3(&client, config, contract, file_path).await?,
        "cos" => upload_cos(&client, config, contract, file_path).await?,
        "azure" => upload_azure(&client, config, contract, file_path).await?,
        _ => {
            return Err(CloudBackupUploadError::config(format!(
                "Unsupported backup sync provider: {}",
                contract.provider
            )));
        }
    };
    let status = response.status();
    if !status.is_success() {
        return Err(CloudBackupUploadError::provider_status(
            &contract.provider,
            status,
        ));
    }
    Ok(CloudBackupUploadResult {
        provider: contract.provider.clone(),
        object_key: contract.object_key.clone(),
        status_code: status.as_u16(),
    })
}

async fn upload_webdav(
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

async fn upload_oss(
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

async fn upload_s3(
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

async fn upload_cos(
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

async fn upload_azure(
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

async fn streaming_body(file_path: &Path) -> Result<reqwest::Body, CloudBackupUploadError> {
    let file = tokio::fs::File::open(file_path)
        .await
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))?;
    Ok(reqwest::Body::wrap_stream(ReaderStream::new(file)))
}

async fn sha256_file_hex(file_path: &Path) -> Result<String, CloudBackupUploadError> {
    let mut file = tokio::fs::File::open(file_path)
        .await
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| CloudBackupUploadError::provider(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn endpoint_url(config: &Value, provider: &str) -> Result<Url, CloudBackupUploadError> {
    let endpoint = require_non_empty(config, "endpoint")?;
    let url = Url::parse(endpoint)
        .map_err(|_| CloudBackupUploadError::config("endpoint must be a valid URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(CloudBackupUploadError::config(
            "endpoint scheme must be http or https",
        ));
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(CloudBackupUploadError::config(
            "endpoint must not include credentials, query, or fragment",
        ));
    }
    let Some(host) = url.host_str() else {
        return Err(CloudBackupUploadError::config("endpoint host is required"));
    };
    if backup_sync_endpoint_host_is_never_allowed(host) {
        return Err(CloudBackupUploadError::config(
            "backup sync endpoint host is not allowed",
        ));
    }
    if backup_sync_endpoint_is_allowlisted(&url) {
        if url.scheme() == "https" || backup_sync_endpoint_is_self_hosted_plain_http(&url) {
            return Ok(url);
        }
        return Err(CloudBackupUploadError::config(
            "backup sync endpoint must use https unless allowlisting a local or private endpoint",
        ));
    }
    if backup_sync_endpoint_host_is_restricted(host) {
        return Err(CloudBackupUploadError::config(
            "backup sync endpoint host is not allowed",
        ));
    }
    if url.scheme() != "https" {
        return Err(CloudBackupUploadError::config(
            "backup sync endpoint must use https",
        ));
    }
    if backup_sync_endpoint_matches_provider(&url, provider) {
        return Ok(url);
    }
    Err(CloudBackupUploadError::config(format!(
        "backup sync endpoint is not allowed; configure {BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV}"
    )))
}

fn append_url_segments(mut url: Url, segments: &[&str]) -> Result<Url, CloudBackupUploadError> {
    let existing_segments = url
        .path_segments()
        .map(|items| {
            items
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    url.set_path("");
    {
        let mut path_segments = url
            .path_segments_mut()
            .map_err(|_| CloudBackupUploadError::config("endpoint cannot be used as base URL"))?;
        for segment in existing_segments {
            path_segments.push(&segment);
        }
        for segment in segments {
            if !segment.is_empty() {
                path_segments.push(segment);
            }
        }
    }
    Ok(url)
}

fn append_url_segments_from_key(url: Url, object_key: &str) -> Result<Url, CloudBackupUploadError> {
    let segments = object_key
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    append_url_segments(url, &segments)
}

fn cos_upload_url(
    mut endpoint: Url,
    bucket: &str,
    object_key: &str,
) -> Result<Url, CloudBackupUploadError> {
    let Some(host) = endpoint.host_str().map(str::to_string) else {
        return Err(CloudBackupUploadError::config("endpoint host is required"));
    };
    if is_loopback_or_local_test_host(&host) || endpoint.port().is_some() {
        let url = append_url_segments(endpoint, &[bucket])?;
        return append_url_segments_from_key(url, object_key);
    }
    if !host.starts_with(&format!("{bucket}.")) {
        endpoint
            .set_host(Some(&format!("{bucket}.{host}")))
            .map_err(|_| CloudBackupUploadError::config("invalid COS bucket host"))?;
    }
    append_url_segments_from_key(endpoint, object_key)
}

fn is_loopback_or_local_test_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1") || host.ends_with(".localhost")
}

fn backup_sync_endpoint_matches_provider(url: &Url, provider: &str) -> bool {
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    match provider {
        "oss" => host.ends_with(".aliyuncs.com") || host == "aliyuncs.com",
        "s3" => host.ends_with(".amazonaws.com") || host == "amazonaws.com",
        "cos" => host.ends_with(".myqcloud.com") || host == "myqcloud.com",
        "azure" => {
            host.ends_with(".blob.core.windows.net")
                || host == "blob.core.windows.net"
                || host.ends_with(".blob.core.chinacloudapi.cn")
                || host == "blob.core.chinacloudapi.cn"
        }
        "webdav" => false,
        _ => false,
    }
}

fn backup_sync_endpoint_is_allowlisted(url: &Url) -> bool {
    let origin = backup_sync_endpoint_origin(url);
    let full = url.as_str().trim_end_matches('/').to_ascii_lowercase();
    env::var(BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV)
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.trim_end_matches('/').to_ascii_lowercase())
        .any(|entry| entry == full || entry == origin)
}

fn backup_sync_endpoint_origin(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    match url.port() {
        Some(port) => format!("{}://{}:{port}", url.scheme(), host),
        None => format!("{}://{}", url.scheme(), host),
    }
}

fn backup_sync_endpoint_is_self_hosted_plain_http(url: &Url) -> bool {
    if url.scheme() != "http" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return true;
    }
    let Ok(address) = host.parse::<IpAddr>() else {
        return false;
    };
    match address {
        IpAddr::V4(address) => address.is_loopback() || address.is_private(),
        IpAddr::V6(address) => address.is_loopback() || address.is_unique_local(),
    }
}

fn backup_sync_endpoint_host_is_restricted(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return true;
    }
    let Ok(address) = host.parse::<IpAddr>() else {
        return false;
    };
    if address.is_unspecified() || address.is_loopback() || address.is_multicast() {
        return true;
    }
    match address {
        IpAddr::V4(address) => address.is_private() || address.is_link_local(),
        IpAddr::V6(address) => address.is_unique_local() || address.is_unicast_link_local(),
    }
}

fn backup_sync_endpoint_host_is_never_allowed(host: &str) -> bool {
    if host.eq_ignore_ascii_case("metadata.google.internal") {
        return true;
    }
    let Ok(address) = host.parse::<IpAddr>() else {
        return false;
    };
    matches!(
        address,
        IpAddr::V4(address)
            if address.octets() == [169, 254, 169, 254]
                || address.octets() == [100, 100, 100, 200]
                || address.is_link_local()
    ) || matches!(
        address,
        IpAddr::V6(address) if address.is_unicast_link_local()
    )
}

fn host_header_value(url: &Url) -> Result<String, CloudBackupUploadError> {
    let Some(host) = url.host_str() else {
        return Err(CloudBackupUploadError::config("endpoint host is required"));
    };
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

fn string_config(config: &Value, key: &str) -> String {
    config
        .get(key)
        .and_then(value_to_string)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn non_empty_config(config: &Value, key: &str) -> Option<String> {
    let value = string_config(config, key);
    (!value.is_empty()).then_some(value)
}

fn require_non_empty<'a>(config: &'a Value, key: &str) -> Result<&'a str, CloudBackupUploadError> {
    config
        .get(key)
        .and_then(value_to_string_ref)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CloudBackupUploadError::config(format!("{key} is required")))
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_to_string_ref(value: &Value) -> Option<&str> {
    match value {
        Value::String(text) => Some(text.as_str()),
        _ => None,
    }
}

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

fn region_from_s3_endpoint(config: &Value) -> Option<String> {
    let endpoint = endpoint_url(config, "s3").ok()?;
    endpoint
        .host_str()?
        .split('.')
        .collect::<Vec<_>>()
        .windows(3)
        .find_map(|parts| {
            if parts[0] == "s3" && parts[2].starts_with("amazonaws") {
                Some(parts[1].to_string())
            } else {
                None
            }
        })
}

fn aws_signing_key(secret_key: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let date_key = hmac_sha256(format!("AWS4{secret_key}").as_bytes(), date.as_bytes());
    let region_key = hmac_sha256(&date_key, region.as_bytes());
    let service_key = hmac_sha256(&region_key, service.as_bytes());
    hmac_sha256(&service_key, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::sign(&key, data).as_ref().to_vec()
}

fn hmac_sha1(key: &[u8], data: &[u8]) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, key);
    hmac::sign(&key, data).as_ref().to_vec()
}

fn sha256_hex(value: &[u8]) -> String {
    hex_lower(&Sha256::digest(value))
}

fn sha1_hex(value: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, value);
    hex_lower(digest.as_ref())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn s3_region_is_derived_from_endpoint_host() {
        assert_eq!(
            region_from_s3_endpoint(&json!({
                "endpoint": "https://s3.us-west-2.amazonaws.com",
            }))
            .as_deref(),
            Some("us-west-2")
        );
    }

    #[test]
    fn s3_region_ignores_non_s3_endpoint_hosts() {
        assert_eq!(
            region_from_s3_endpoint(&json!({
                "endpoint": "https://storage.example.com",
            })),
            None
        );
    }
}
