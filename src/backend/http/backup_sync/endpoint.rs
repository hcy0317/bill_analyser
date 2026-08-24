use std::env;

use bill_analyser_core::{OutboundHostClass, OutboundHttpUrl, OutboundHttpUrlParseError};
use serde_json::Value;
use url::Url;

use super::config::require_non_empty;
use super::types::{CloudBackupUploadError, BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV};

/// 解析并校验云备份 endpoint，集中执行 scheme、凭据、SSRF、allowlist 和 provider 域名规则。
pub(super) fn endpoint_url(config: &Value, provider: &str) -> Result<Url, CloudBackupUploadError> {
    let endpoint = require_non_empty(config, "endpoint")?;
    let endpoint = OutboundHttpUrl::parse(endpoint).map_err(|error| match error {
        OutboundHttpUrlParseError::UnsupportedScheme => {
            CloudBackupUploadError::config("endpoint scheme must be http or https")
        }
        OutboundHttpUrlParseError::Credentials => CloudBackupUploadError::config(
            "endpoint must not include credentials, query, or fragment",
        ),
        OutboundHttpUrlParseError::Empty
        | OutboundHttpUrlParseError::UnsafeText
        | OutboundHttpUrlParseError::Invalid
        | OutboundHttpUrlParseError::MissingHost => {
            CloudBackupUploadError::config("endpoint must be a valid URL")
        }
    })?;
    if endpoint.has_query_or_fragment() {
        return Err(CloudBackupUploadError::config(
            "endpoint must not include credentials, query, or fragment",
        ));
    }
    if backup_sync_endpoint_host_is_never_allowed(&endpoint) {
        return Err(CloudBackupUploadError::config(
            "backup sync endpoint host is not allowed",
        ));
    }
    if backup_sync_endpoint_is_allowlisted(&endpoint) {
        if endpoint.is_https() || backup_sync_endpoint_is_self_hosted_plain_http(&endpoint) {
            return Ok(endpoint.into_url());
        }
        return Err(CloudBackupUploadError::config(
            "backup sync endpoint must use https unless allowlisting a local or private endpoint",
        ));
    }
    if backup_sync_endpoint_host_is_restricted(&endpoint) {
        return Err(CloudBackupUploadError::config(
            "backup sync endpoint host is not allowed",
        ));
    }
    if !endpoint.is_https() {
        return Err(CloudBackupUploadError::config(
            "backup sync endpoint must use https",
        ));
    }
    if backup_sync_endpoint_matches_provider(endpoint.as_url(), provider) {
        return Ok(endpoint.into_url());
    }
    Err(CloudBackupUploadError::config(format!(
        "backup sync endpoint is not allowed; configure {BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV}"
    )))
}

/// 在保留 endpoint 原有 path 前缀的基础上安全追加 URL path 段，供各 provider 拼接对象路径。
pub(super) fn append_url_segments(
    mut url: Url,
    segments: &[&str],
) -> Result<Url, CloudBackupUploadError> {
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

/// 将对象 key 拆成 URL path 段追加，避免把带斜杠的 key 当成未编码的整段路径。
pub(super) fn append_url_segments_from_key(
    url: Url,
    object_key: &str,
) -> Result<Url, CloudBackupUploadError> {
    let segments = object_key
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    append_url_segments(url, &segments)
}

/// 构造 COS 上传 URL，本地测试 endpoint 使用 path-style，真实 endpoint 使用 bucket virtual-host。
pub(super) fn cos_upload_url(
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

/// 校验公网 HTTPS endpoint 是否匹配当前 provider 的官方域名后缀。
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

/// 检查 endpoint 是否命中显式 allowlist，支持 origin 或完整 URL 两种配置粒度。
fn backup_sync_endpoint_is_allowlisted(endpoint: &OutboundHttpUrl) -> bool {
    endpoint.matches_exact_or_origin_allowlist(
        &env::var(BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV).unwrap_or_default(),
    )
}

/// 判断 allowlist 命中的 HTTP endpoint 是否限定在本机或私有网段，避免公开明文上传。
fn backup_sync_endpoint_is_self_hosted_plain_http(endpoint: &OutboundHttpUrl) -> bool {
    !endpoint.is_https()
        && matches!(
            endpoint.host_class(),
            OutboundHostClass::Localhost | OutboundHostClass::Loopback | OutboundHostClass::Private
        )
}

/// 识别默认禁止访问的内网/本地地址，除非用户显式 allowlist 后再按 HTTP 例外收窄。
fn backup_sync_endpoint_host_is_restricted(endpoint: &OutboundHttpUrl) -> bool {
    !matches!(endpoint.host_class(), OutboundHostClass::Public)
}

/// 识别即使 allowlist 也不允许访问的云元数据和 link-local 地址，防止 SSRF 打到敏感元数据服务。
fn backup_sync_endpoint_host_is_never_allowed(endpoint: &OutboundHttpUrl) -> bool {
    matches!(
        endpoint.host_class(),
        OutboundHostClass::Metadata
            | OutboundHostClass::Unspecified
            | OutboundHostClass::LinkLocal
            | OutboundHostClass::Multicast
    )
}

/// 根据最终上传 URL 构造签名使用的 Host header，保留显式端口以匹配实际请求。
pub(super) fn host_header_value(url: &Url) -> Result<String, CloudBackupUploadError> {
    let Some(host) = url.host_str() else {
        return Err(CloudBackupUploadError::config("endpoint host is required"));
    };
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

/// 从标准 S3 endpoint host 推断 region，作为用户未配置 region 时的兼容默认值。
pub(super) fn region_from_s3_endpoint(config: &Value) -> Option<String> {
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
