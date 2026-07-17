// 中文导读：未命中 dedicated parser 时，为列映射界面提供当前用户临时文件的表格预览。
// 维护重点：HTTP 只负责认证、session/path 边界、文本预览与错误映射；表格格式安全策略归 parser 层。
// 不变式：预览不写数据库、不改变 import session，返回结果保持前端现有 result envelope。

mod payload;
mod request;
mod text;

#[cfg(test)]
mod tests;

pub(super) use request::{
    import_file_preview_runtime_handler, read_bounded_preview_file,
    IMPORT_FILE_PREVIEW_HARD_MAX_BYTES,
};
pub(super) use text::{decode_preview_text, normalize_preview_encoding};
