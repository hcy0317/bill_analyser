use std::path::Path;

use ring::hmac;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;

use super::types::CloudBackupUploadError;

/// 以流式 body 读取备份文件，避免大备份上传时一次性载入内存。
pub(super) async fn streaming_body(
    file_path: &Path,
) -> Result<reqwest::Body, CloudBackupUploadError> {
    let file = tokio::fs::File::open(file_path)
        .await
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))?;
    Ok(reqwest::Body::wrap_stream(ReaderStream::new(file)))
}

/// 流式计算文件 SHA256 hex，供 S3 签名锁定 payload hash。
pub(super) async fn sha256_file_hex(file_path: &Path) -> Result<String, CloudBackupUploadError> {
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

/// 按 AWS Signature V4 规则派生服务签名密钥，供 S3 兼容上传签名复用。
pub(super) fn aws_signing_key(
    secret_key: &str,
    date: &str,
    region: &str,
    service: &str,
) -> Vec<u8> {
    let date_key = hmac_sha256(format!("AWS4{secret_key}").as_bytes(), date.as_bytes());
    let region_key = hmac_sha256(&date_key, region.as_bytes());
    let service_key = hmac_sha256(&region_key, service.as_bytes());
    hmac_sha256(&service_key, b"aws4_request")
}

/// 计算 HMAC-SHA256 原始字节，用于 S3 和 Azure 签名。
pub(super) fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::sign(&key, data).as_ref().to_vec()
}

/// 计算 HMAC-SHA1 原始字节，仅用于 OSS/COS 当前历史签名算法。
pub(super) fn hmac_sha1(key: &[u8], data: &[u8]) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, key);
    hmac::sign(&key, data).as_ref().to_vec()
}

/// 计算内存片段 SHA256 hex，供 canonical request 摘要使用。
pub(super) fn sha256_hex(value: &[u8]) -> String {
    hex_lower(&Sha256::digest(value))
}

/// 计算内存片段 SHA1 hex，仅用于 COS canonical request 摘要。
pub(super) fn sha1_hex(value: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, value);
    hex_lower(digest.as_ref())
}

/// 将原始字节编码为小写 hex，确保不同 provider 的签名摘要格式一致。
pub(super) fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
