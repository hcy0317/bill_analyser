use sha2::{Digest, Sha256};

#[tracing::instrument(level = "debug", skip_all)]
/// 判断备份加密密钥是否真实配置，避免空白字符串被误认为可解密密钥。
pub fn backup_encryption_secret_configured(secret: Option<&str>) -> bool {
    secret.is_some_and(|value| !value.trim().is_empty())
}

#[tracing::instrument(level = "debug", skip_all)]
/// 将用户配置的备份密钥稳定派生为 Fernet 所需的 urlsafe base64 密钥。
pub fn derive_backup_fernet_key(secret: &str) -> Option<String> {
    let secret = secret.trim();
    if secret.is_empty() {
        return None;
    }
    let digest = Sha256::digest(secret.as_bytes());
    Some(base64_urlsafe_padded(&digest))
}

/// 使用 Fernet 兼容的 urlsafe alphabet 生成带 padding 的 base64 字符串，避免引入额外运行时依赖。
fn base64_urlsafe_padded(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut encoded = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        let triple = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);

        encoded.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        encoded.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() >= 2 {
            encoded.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() == 3 {
            encoded.push(ALPHABET[(triple & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}
