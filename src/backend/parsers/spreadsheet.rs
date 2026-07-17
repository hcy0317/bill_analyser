mod error;
mod html;
mod xlsx;

pub use error::{SpreadsheetValidationError, SpreadsheetValidationErrorKind};

pub const MAX_SPREADSHEET_INPUT_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_SPREADSHEET_EXPANDED_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_SPREADSHEET_WORKSHEET_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_SPREADSHEET_ARCHIVE_ENTRIES: usize = 512;
pub const MAX_SPREADSHEET_ROWS: usize = 50_000;
pub const MAX_SPREADSHEET_COLUMNS: usize = 128;
pub const MAX_SPREADSHEET_CELL_BYTES: usize = 16 * 1024;
pub const MAX_SPREADSHEET_CELLS: usize = 1_000_000;
pub const MAX_SPREADSHEET_TOTAL_CELL_BYTES: usize = 64 * 1024 * 1024;
pub const SPREADSHEET_PREVIEW_SAMPLE_ROWS: usize = 512;

const OLE_MAGIC: &[u8] = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpreadsheetPreviewRows {
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
}

/// 为通用列映射预览解析一个受统一资源预算约束的 HTML/XLSX 表格。
pub fn parse_spreadsheet_preview_rows(
    bytes: &[u8],
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    parse_spreadsheet_rows_with_limit(bytes, SPREADSHEET_PREVIEW_SAMPLE_ROWS)
}

/// 为通用列映射完整解析所有仍处于统一资源预算内的表格行。
pub fn parse_spreadsheet_rows(
    bytes: &[u8],
) -> Result<Vec<Vec<String>>, SpreadsheetValidationError> {
    let parsed = parse_spreadsheet_rows_with_limit(bytes, MAX_SPREADSHEET_ROWS)?;
    if parsed.rows.len() != parsed.total_rows {
        return Err(SpreadsheetValidationError::too_large(
            "Import spreadsheet rows could not be fully materialized",
        ));
    }
    Ok(parsed.rows)
}

fn parse_spreadsheet_rows_with_limit(
    bytes: &[u8],
    materialized_row_limit: usize,
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    validate_input_size(bytes)?;
    if looks_like_html_spreadsheet(bytes) {
        return html::parse_rows(bytes, materialized_row_limit);
    }
    if bytes.starts_with(OLE_MAGIC) {
        return Err(SpreadsheetValidationError::unsupported(
            "Legacy binary XLS preview is not supported; convert the file to XLSX or CSV",
        ));
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(SpreadsheetValidationError::invalid(
            "Import preview spreadsheet content is invalid",
        ));
    }
    xlsx::parse_rows(bytes, materialized_row_limit)
}

/// 在通用预览前验证上传内容；legacy OLE/BIFF XLS 保持不支持。
pub fn validate_spreadsheet_payload(bytes: &[u8]) -> Result<(), SpreadsheetValidationError> {
    parse_spreadsheet_preview_rows(bytes).map(|_| ())
}

/// 在 dedicated parser 选择前执行统一资源边界校验。
///
/// HTML/XLSX 继续使用受限解析；legacy binary XLS 暂时失败关闭，避免在第三方
/// 解析器无法证明完整资源边界时继续物化。HTML 表格形式的 `.xls` 导出不受影响。
pub fn validate_dedicated_spreadsheet_payload(
    bytes: &[u8],
) -> Result<(), SpreadsheetValidationError> {
    validate_input_size(bytes)?;
    if bytes.starts_with(OLE_MAGIC) {
        return Err(SpreadsheetValidationError::unsupported(
            "Legacy binary XLS is not supported; convert the file to XLSX or CSV",
        ));
    }
    if looks_like_html_spreadsheet(bytes) {
        return html::parse_preview_rows(bytes).map(|_| ());
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(SpreadsheetValidationError::invalid(
            "Import preview spreadsheet content is invalid",
        ));
    }
    xlsx::parse_preview_rows(bytes).map(|_| ())
}

pub(crate) fn parse_html_preview_rows(
    bytes: &[u8],
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    validate_input_size(bytes)?;
    html::parse_preview_rows(bytes)
}

fn validate_input_size(bytes: &[u8]) -> Result<(), SpreadsheetValidationError> {
    if bytes.len() > MAX_SPREADSHEET_INPUT_BYTES {
        return Err(SpreadsheetValidationError::too_large(
            "Import preview file exceeds the size limit",
        ));
    }
    Ok(())
}

fn looks_like_html_spreadsheet(bytes: &[u8]) -> bool {
    let mut probe = bytes;
    if probe.starts_with(&[0xef, 0xbb, 0xbf]) {
        probe = &probe[3..];
    }
    probe = probe
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|index| &probe[index..])
        .unwrap_or_default();
    [b"<!doctype html".as_slice(), b"<html", b"<table"]
        .into_iter()
        .any(|prefix| {
            probe
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_dispatch_rejects_size_binary_and_spoofed_inputs() {
        let oversized = vec![b'x'; MAX_SPREADSHEET_INPUT_BYTES + 1];
        let error = validate_spreadsheet_payload(&oversized).expect_err("oversized input");
        assert_eq!(
            (error.kind(), error.message()),
            (
                SpreadsheetValidationErrorKind::TooLarge,
                "Import preview file exceeds the size limit"
            )
        );

        let error = validate_spreadsheet_payload(OLE_MAGIC).expect_err("legacy XLS");
        assert_eq!(error.kind(), SpreadsheetValidationErrorKind::Unsupported);
        assert!(error.message().contains("Legacy binary XLS"));

        let error = validate_spreadsheet_payload(b"not-a-spreadsheet").expect_err("spoofed input");
        assert_eq!(error.kind(), SpreadsheetValidationErrorKind::Invalid);
    }

    #[test]
    fn html_probe_accepts_bom_case_and_rejects_plain_text() {
        assert!(looks_like_html_spreadsheet(b"\xef\xbb\xbf  <HTML><table/>"));
        assert!(looks_like_html_spreadsheet(b" <!DOCTYPE HTML><html/>"));
        assert!(!looks_like_html_spreadsheet(b"plain text"));
    }
}
