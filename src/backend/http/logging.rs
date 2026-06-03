// 中文导读：HTTP 运行态日志初始化，统一 Rust-only 服务的默认日志等级和 span 输出策略。
// 维护重点：默认只输出 info 及以上；开发环境通过 RUST_LOG 打开 debug span。
// 不变式：日志字段只记录功能域、操作和不敏感的内部 ID，不写入用户内容、文件名、token 或密钥。

use std::sync::Once;

use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

static INIT_TRACING: Once = Once::new();

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
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .try_init()
            .ok();
    });
}
