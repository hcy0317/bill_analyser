use super::*;
use super::{
    endpoint::{endpoint_url, region_from_s3_endpoint},
    types::BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV,
};
use serde_json::{json, Value};
use std::env;
use std::sync::{Mutex, MutexGuard, OnceLock};

static ENDPOINT_ALLOWLIST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct EndpointAllowlistGuard {
    previous: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for EndpointAllowlistGuard {
    fn drop(&mut self) {
        match self.previous.as_deref() {
            Some(value) => env::set_var(BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV, value),
            None => env::remove_var(BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV),
        }
    }
}

fn endpoint_allowlist(value: Option<&str>) -> EndpointAllowlistGuard {
    let lock = ENDPOINT_ALLOWLIST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("endpoint allowlist env lock should not be poisoned");
    let previous = env::var(BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV).ok();
    match value {
        Some(value) => env::set_var(BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV, value),
        None => env::remove_var(BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV),
    }
    EndpointAllowlistGuard {
        previous,
        _lock: lock,
    }
}

fn sync_contract(provider: &str, supported: bool) -> SyncConfigContract {
    SyncConfigContract {
        provider: provider.to_string(),
        supported,
        endpoint: String::new(),
        bucket: String::new(),
        prefix: "daily".to_string(),
        object_key: "daily/backup.zip".to_string(),
        safe_config: json!({}),
    }
}

fn config_with_endpoint(endpoint: &str) -> Value {
    json!({ "endpoint": endpoint })
}

fn endpoint_error(provider: &str, endpoint: &str) -> CloudBackupUploadError {
    endpoint_url(&config_with_endpoint(endpoint), provider)
        .expect_err("endpoint should be rejected")
}

#[test]
fn s3_region_is_derived_from_endpoint_host() {
    let _guard = endpoint_allowlist(None);

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
    let _guard = endpoint_allowlist(None);

    assert_eq!(
        region_from_s3_endpoint(&json!({
            "endpoint": "https://storage.example.com",
        })),
        None
    );
}

#[test]
fn provider_https_endpoints_are_accepted_only_for_matching_hosts() {
    let _guard = endpoint_allowlist(None);
    let accepted = [
        ("oss", "https://oss-cn-hangzhou.aliyuncs.com"),
        ("s3", "https://s3.us-west-2.amazonaws.com"),
        ("cos", "https://cos.ap-guangzhou.myqcloud.com"),
        ("azure", "https://account.blob.core.windows.net"),
        ("azure", "https://account.blob.core.chinacloudapi.cn"),
    ];

    for (provider, endpoint) in accepted {
        assert!(
            endpoint_url(&config_with_endpoint(endpoint), provider).is_ok(),
            "{provider} should accept {endpoint}"
        );
    }

    let error = endpoint_error("s3", "https://storage.example.com");
    assert_eq!(error.status_code, 400);
    assert!(error.message.contains(BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV));
}

#[test]
fn endpoint_validation_rejects_plain_http_credentials_and_metadata_hosts() {
    let _guard = endpoint_allowlist(None);
    let rejected = [
        (
            "s3",
            "http://s3.us-west-2.amazonaws.com",
            "backup sync endpoint must use https",
        ),
        (
            "s3",
            "https://user:secret@s3.us-west-2.amazonaws.com",
            "endpoint must not include credentials, query, or fragment",
        ),
        (
            "s3",
            "https://s3.us-west-2.amazonaws.com?token=secret",
            "endpoint must not include credentials, query, or fragment",
        ),
        (
            "s3",
            "https://metadata.google.internal",
            "backup sync endpoint host is not allowed",
        ),
        (
            "s3",
            "http://169.254.169.254/latest/meta-data",
            "backup sync endpoint host is not allowed",
        ),
        (
            "s3",
            "https://127.0.0.1:9000",
            "backup sync endpoint host is not allowed",
        ),
    ];

    for (provider, endpoint, message) in rejected {
        let error = endpoint_error(provider, endpoint);
        assert_eq!(error.status_code, 400);
        assert_eq!(error.message, message);
    }
}

#[test]
fn endpoint_allowlist_accepts_local_http_but_not_public_plain_http() {
    let _guard = endpoint_allowlist(Some("http://127.0.0.1:9000;http://example.com"));

    assert!(endpoint_url(&config_with_endpoint("http://127.0.0.1:9000"), "webdav").is_ok());

    let error = endpoint_error("webdav", "http://example.com");
    assert_eq!(error.status_code, 400);
    assert_eq!(
        error.message,
        "backup sync endpoint must use https unless allowlisting a local or private endpoint"
    );
}

#[test]
fn endpoint_allowlist_accepts_full_or_origin_https_entries() {
    let _guard = endpoint_allowlist(Some(
        "https://storage.example.com;https://storage-with-path.example.com/base",
    ));

    assert!(endpoint_url(
        &config_with_endpoint("https://storage.example.com/backups"),
        "webdav"
    )
    .is_ok());
    assert!(endpoint_url(
        &config_with_endpoint("https://storage-with-path.example.com/base"),
        "webdav",
    )
    .is_ok());
    assert!(endpoint_url(
        &config_with_endpoint("https://storage-with-path.example.com/base/child"),
        "webdav",
    )
    .is_err());
}

#[test]
fn validate_sync_upload_config_keeps_provider_specific_required_fields() {
    let _guard = endpoint_allowlist(None);
    let s3 = sync_contract("s3", true);

    let missing_bucket = validate_sync_upload_config(
        &json!({ "endpoint": "https://s3.us-west-2.amazonaws.com" }),
        &s3,
    )
    .expect_err("bucket should be required");
    assert_eq!(missing_bucket.message, "bucket is required");

    let missing_access_key = validate_sync_upload_config(
        &json!({
            "endpoint": "https://s3.us-west-2.amazonaws.com",
            "bucket": "daily"
        }),
        &s3,
    )
    .expect_err("access_key should be required");
    assert_eq!(missing_access_key.message, "access_key is required");

    let missing_secret_key = validate_sync_upload_config(
        &json!({
            "endpoint": "https://s3.us-west-2.amazonaws.com",
            "bucket": "daily",
            "access_key": "ak"
        }),
        &s3,
    )
    .expect_err("secret_key should be required");
    assert_eq!(missing_secret_key.message, "secret_key is required");
}

#[test]
fn validate_sync_upload_config_rejects_unsupported_and_invalid_azure_secret() {
    let _guard = endpoint_allowlist(None);

    let unsupported = validate_sync_upload_config(
        &json!({ "endpoint": "https://backup.example.com" }),
        &sync_contract("dropbox", false),
    )
    .expect_err("unsupported provider should fail before endpoint validation");
    assert_eq!(
        unsupported.message,
        "Unsupported backup sync provider: dropbox"
    );

    let invalid_azure_secret = validate_sync_upload_config(
        &json!({
            "endpoint": "https://account.blob.core.windows.net",
            "bucket": "daily",
            "access_key": "account",
            "secret_key": "not-base64"
        }),
        &sync_contract("azure", true),
    )
    .expect_err("azure secret_key must be base64");
    assert_eq!(
        invalid_azure_secret.message,
        "azure secret_key must be base64"
    );
}

#[test]
fn validate_sync_upload_config_allows_webdav_with_allowlisted_local_endpoint() {
    let _guard = endpoint_allowlist(Some("http://127.0.0.1:9000"));

    assert!(validate_sync_upload_config(
        &json!({ "endpoint": "http://127.0.0.1:9000" }),
        &sync_contract("webdav", true),
    )
    .is_ok());
}
