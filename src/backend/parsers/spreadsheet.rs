mod error;
mod html;
mod xls;
mod xls_preflight;
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

#[derive(Clone, Copy)]
enum XlsxRowsContract {
    BoundedXml,
    DedicatedCalamine,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpreadsheetPreviewRows {
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedSpreadsheet {
    rows: Vec<Vec<String>>,
}

impl PreparedSpreadsheet {
    pub(crate) fn rows(&self) -> &[Vec<String>] {
        &self.rows
    }

    pub(crate) fn contains(&self, needle: &str) -> bool {
        self.rows.iter().flatten().any(|cell| cell.contains(needle))
    }

    pub(crate) fn contains_all(&self, needles: &[&str]) -> bool {
        needles.iter().all(|needle| self.contains(needle))
    }
}

/// 为通用列映射预览解析一个受统一资源预算约束的 HTML/XLS/XLSX 表格。
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

pub(crate) fn prepare_dedicated_spreadsheet_payload(
    bytes: &[u8],
) -> Result<PreparedSpreadsheet, SpreadsheetValidationError> {
    let parsed = parse_spreadsheet_rows_with_contract(
        bytes,
        MAX_SPREADSHEET_ROWS,
        XlsxRowsContract::DedicatedCalamine,
    )?;
    if parsed.rows.len() != parsed.total_rows {
        return Err(SpreadsheetValidationError::too_large(
            "Import spreadsheet rows could not be fully materialized",
        ));
    }
    Ok(PreparedSpreadsheet { rows: parsed.rows })
}

fn parse_spreadsheet_rows_with_limit(
    bytes: &[u8],
    materialized_row_limit: usize,
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    parse_spreadsheet_rows_with_contract(
        bytes,
        materialized_row_limit,
        XlsxRowsContract::BoundedXml,
    )
}

fn parse_spreadsheet_rows_with_contract(
    bytes: &[u8],
    materialized_row_limit: usize,
    xlsx_contract: XlsxRowsContract,
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    validate_input_size(bytes)?;
    if looks_like_html_spreadsheet(bytes) {
        return html::parse_rows(bytes, materialized_row_limit);
    }
    if bytes.starts_with(OLE_MAGIC) {
        return xls::parse_rows(bytes, materialized_row_limit);
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(SpreadsheetValidationError::invalid(
            "Import preview spreadsheet content is invalid",
        ));
    }
    match xlsx_contract {
        XlsxRowsContract::BoundedXml => xlsx::parse_rows(bytes, materialized_row_limit),
        XlsxRowsContract::DedicatedCalamine => {
            xlsx::parse_dedicated_rows(bytes, materialized_row_limit)
        }
    }
}

/// 在通用预览前验证上传内容，包括受统一预算约束的 OLE/BIFF XLS。
pub fn validate_spreadsheet_payload(bytes: &[u8]) -> Result<(), SpreadsheetValidationError> {
    parse_spreadsheet_preview_rows(bytes).map(|_| ())
}

/// 在 dedicated parser 选择前执行统一资源边界校验。
///
/// HTML/XLSX 使用各自的受限解析；OLE/BIFF XLS 在交给 dedicated parser 前
/// 先完整遍历首个工作表并执行统一行、列、单元格和物化字节预算。
pub fn validate_dedicated_spreadsheet_payload(
    bytes: &[u8],
) -> Result<(), SpreadsheetValidationError> {
    validate_input_size(bytes)?;
    if bytes.starts_with(OLE_MAGIC) {
        return xls::parse_preview_rows(bytes).map(|_| ());
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
    use std::io::{Cursor, Write};

    use zip::{write::SimpleFileOptions, ZipWriter};

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

        let error = validate_spreadsheet_payload(OLE_MAGIC).expect_err("truncated XLS");
        assert_eq!(error.kind(), SpreadsheetValidationErrorKind::Invalid);

        let error = validate_spreadsheet_payload(b"not-a-spreadsheet").expect_err("spoofed input");
        assert_eq!(error.kind(), SpreadsheetValidationErrorKind::Invalid);
    }

    #[test]
    fn html_probe_accepts_bom_case_and_rejects_plain_text() {
        assert!(looks_like_html_spreadsheet(b"\xef\xbb\xbf  <HTML><table/>"));
        assert!(looks_like_html_spreadsheet(b" <!DOCTYPE HTML><html/>"));
        assert!(!looks_like_html_spreadsheet(b"plain text"));
    }

    #[test]
    fn dedicated_preparation_exposes_complete_bounded_rows_and_search() {
        let bytes =
            include_bytes!("../../../tests/fixtures/import_samples/abc_statement_sample.xls");

        let prepared = prepare_dedicated_spreadsheet_payload(bytes)
            .expect("bounded dedicated spreadsheet preparation");

        assert_eq!(prepared.rows().len(), 4);
        assert!(prepared.contains("交易日期"));
        assert!(prepared.contains_all(&["交易日期", "收入金额", "支出金额"]));
        assert!(!prepared.contains("支付宝"));
    }

    #[test]
    fn dedicated_preparation_preserves_calamine_row_and_rich_text_contract() {
        let bytes = xlsx_archive(&[
            (
                "[Content_Types].xml",
                br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
            (
                "xl/workbook.xml",
                br#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#,
            ),
            (
                "xl/sharedStrings.xml",
                br#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2" uniqueCount="2"><si><t>Header</t></si><si><r><t>2026-08-15 </t></r><r><t> 12:34:56</t></r></si></sst>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                br#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:A3"/><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c></row><row r="3"><c r="A3" t="s"><v>1</v></c></row></sheetData></worksheet>"#,
            ),
        ]);

        let prepared = prepare_dedicated_spreadsheet_payload(&bytes)
            .expect("bounded dedicated XLSX preparation");

        assert_eq!(prepared.rows().len(), 3);
        assert_eq!(prepared.rows()[1][0], "");
        assert_eq!(prepared.rows()[2][0], "2026-08-15  12:34:56");
    }

    fn xlsx_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        {
            let mut archive = ZipWriter::new(&mut output);
            for (name, body) in entries {
                archive
                    .start_file(*name, SimpleFileOptions::default())
                    .expect("start XLSX entry");
                archive.write_all(body).expect("write XLSX entry");
            }
            archive.finish().expect("finish XLSX archive");
        }
        output.into_inner()
    }
}
