// 中文导读：HTTP 运行态日志初始化，统一 Rust-only 服务的默认日志等级和 span 输出策略。
// 维护重点：默认只输出 info 及以上；开发环境通过 RUST_LOG 打开 debug span。
// 不变式：日志字段只记录功能域、操作和不敏感的内部 ID，不写入用户内容、文件名、token 或密钥。

use std::{fmt, sync::Once};

use chrono::{Local, SecondsFormat};
use tracing_subscriber::fmt::{format::FmtSpan, time::FormatTime};
use tracing_subscriber::EnvFilter;

static INIT_TRACING: Once = Once::new();

struct LocalRfc3339Timer;

impl FormatTime for LocalRfc3339Timer {
    fn format_time(&self, writer: &mut tracing_subscriber::fmt::format::Writer<'_>) -> fmt::Result {
        write!(writer, "{}", local_rfc3339_timestamp())
    }
}

fn local_rfc3339_timestamp() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Millis, false)
}

pub fn runtime_log_filter_from_directives(directives: Option<&str>) -> EnvFilter {
    let builder = EnvFilter::builder().with_default_directive(tracing::Level::INFO.into());
    match directives {
        Some(value) => builder.parse_lossy(value),
        None => builder.from_env_lossy(),
    }
}

pub fn init_runtime_tracing() {
    INIT_TRACING.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(runtime_log_filter_from_directives(None))
            .with_timer(LocalRfc3339Timer)
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .try_init()
            .ok();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_log_timestamp_includes_timezone_offset() {
        let timestamp = local_rfc3339_timestamp();
        let has_numeric_offset = timestamp.len() >= 6 && {
            let offset_start = timestamp.len() - 6;
            let offset = &timestamp[offset_start..];
            matches!(offset.as_bytes().first(), Some(b'+') | Some(b'-'))
                && offset.as_bytes().get(3) == Some(&b':')
                && offset
                    .chars()
                    .enumerate()
                    .all(|(index, value)| index == 0 || index == 3 || value.is_ascii_digit())
        };

        assert!(timestamp.contains('T'), "timestamp={timestamp}");
        assert!(
            timestamp.ends_with('Z') || has_numeric_offset,
            "timestamp should include timezone offset, got {timestamp}"
        );
    }
}
