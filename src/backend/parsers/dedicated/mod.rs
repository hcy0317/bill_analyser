// 中文导读：dedicated parser 门面，负责聚合来源解析器、自动检测和选择结果类型。
// 维护重点：本文件只保留模块边界和公开导出，具体检测、证据和选择逻辑下沉到功能子文件。
// 不变式：每个上传文件仍必须且只能命中一个 dedicated parser，冲突和未命中只返回决策证据。

mod common;
mod evidence;
mod hints;
mod registry;
mod selection;
mod types;

#[path = "ABC.rs"]
mod abc;
#[path = "Alipay.rs"]
mod alipay;
#[path = "CCB.rs"]
mod ccb;
#[path = "CMBC.rs"]
mod cmbc;
#[path = "ICBC.rs"]
mod icbc;
#[path = "WeChat.rs"]
mod wechat;

pub use selection::{
    detect_dedicated_import_bytes, parse_dedicated_import_bytes,
    parse_dedicated_import_bytes_with_decision,
};
pub use types::{
    DedicatedParseResult, DedicatedParseSelectionResult, DedicatedParserCandidate,
    DedicatedParserDecision,
};

/// 读取 HTML spreadsheet，并在返回前执行预览资源预算。
pub fn parse_html_spreadsheet_preview_rows(bytes: &[u8]) -> Option<(Vec<Vec<String>>, usize)> {
    common::html_spreadsheet_preview_rows(bytes)
}
