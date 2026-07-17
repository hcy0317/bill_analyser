use encoding_rs::{GB18030, GBK};
use regex::Regex;

use super::{
    SpreadsheetPreviewRows, SpreadsheetValidationError, MAX_SPREADSHEET_CELLS,
    MAX_SPREADSHEET_CELL_BYTES, MAX_SPREADSHEET_COLUMNS, MAX_SPREADSHEET_ROWS,
    MAX_SPREADSHEET_TOTAL_CELL_BYTES, SPREADSHEET_PREVIEW_SAMPLE_ROWS,
};

pub(super) fn parse_preview_rows(
    bytes: &[u8],
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    parse_rows(bytes, SPREADSHEET_PREVIEW_SAMPLE_ROWS)
}

pub(super) fn parse_rows(
    bytes: &[u8],
    materialized_row_limit: usize,
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    let text = decode(bytes)?;
    let row_re = Regex::new(r"(?is)<tr[^>]*>(.*?)</tr>").map_err(|_| invalid())?;
    let cell_re = Regex::new(r"(?is)<t[dh][^>]*>(.*?)</t[dh]>").map_err(|_| invalid())?;
    let tag_re = Regex::new(r"(?is)<[^>]+>").map_err(|_| invalid())?;
    let mut sample = Vec::new();
    let mut total_rows = 0usize;
    let mut total_cells = 0usize;
    let mut total_cell_bytes = 0usize;

    for row_capture in row_re.captures_iter(&text) {
        let body = row_capture
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default();
        let mut row = Vec::new();
        for cell_capture in cell_re.captures_iter(body) {
            if row.len() >= MAX_SPREADSHEET_COLUMNS {
                return Err(SpreadsheetValidationError::too_large(
                    "Import preview worksheet exceeds the row or column limit",
                ));
            }
            let raw = cell_capture
                .get(1)
                .map(|value| value.as_str())
                .unwrap_or_default();
            let value = clean_cell(&tag_re.replace_all(raw, ""));
            if value.len() > MAX_SPREADSHEET_CELL_BYTES {
                return Err(SpreadsheetValidationError::too_large(
                    "Import preview worksheet contains an oversized cell",
                ));
            }
            total_cells = total_cells.checked_add(1).ok_or_else(cell_limit)?;
            if total_cells > MAX_SPREADSHEET_CELLS {
                return Err(cell_limit());
            }
            total_cell_bytes = total_cell_bytes
                .checked_add(value.len())
                .ok_or_else(byte_limit)?;
            if total_cell_bytes > MAX_SPREADSHEET_TOTAL_CELL_BYTES {
                return Err(byte_limit());
            }
            row.push(value);
        }
        if row.iter().all(|cell| cell.is_empty()) {
            continue;
        }
        total_rows = total_rows.checked_add(1).ok_or_else(shape_limit)?;
        if total_rows > MAX_SPREADSHEET_ROWS {
            return Err(shape_limit());
        }
        if sample.len() < materialized_row_limit {
            sample.push(row);
        }
    }

    if total_rows == 0 {
        return Err(SpreadsheetValidationError::invalid(
            "Import preview worksheet contains no rows",
        ));
    }
    Ok(SpreadsheetPreviewRows {
        rows: sample,
        total_rows,
    })
}

fn decode(bytes: &[u8]) -> Result<String, SpreadsheetValidationError> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.trim_start_matches('\u{feff}').to_string());
    }
    let (text, _, had_errors) = GB18030.decode(bytes);
    if !had_errors {
        return Ok(text.trim_start_matches('\u{feff}').to_string());
    }
    let (text, _, had_errors) = GBK.decode(bytes);
    if had_errors {
        return Err(invalid());
    }
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

fn clean_cell(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#13;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn invalid() -> SpreadsheetValidationError {
    SpreadsheetValidationError::invalid("Import preview spreadsheet content is invalid")
}

fn shape_limit() -> SpreadsheetValidationError {
    SpreadsheetValidationError::too_large(
        "Import preview worksheet exceeds the row or column limit",
    )
}

fn cell_limit() -> SpreadsheetValidationError {
    SpreadsheetValidationError::too_large("Import preview worksheet exceeds the cell limit")
}

fn byte_limit() -> SpreadsheetValidationError {
    SpreadsheetValidationError::too_large("Import preview worksheet exceeds the byte limit")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bounded_html_and_returns_preview_rows() {
        let preview = parse_preview_rows(
            b"<table><tr><th>Date</th><th>Amount</th></tr><tr><td>2026-01-01</td><td>1.00</td></tr></table>",
        )
        .expect("bounded HTML spreadsheet");
        assert_eq!(preview.total_rows, 2);
        assert_eq!(preview.rows[1], ["2026-01-01", "1.00"]);
    }

    #[test]
    fn rejects_empty_oversized_cell_and_wide_html() {
        let error = parse_preview_rows(b"<table></table>").expect_err("empty HTML");
        assert_eq!(error.kind(), crate::SpreadsheetValidationErrorKind::Invalid);

        let large = format!(
            "<table><tr><td>{}</td></tr></table>",
            "x".repeat(MAX_SPREADSHEET_CELL_BYTES + 1)
        );
        assert_eq!(
            parse_preview_rows(large.as_bytes()).unwrap_err().kind(),
            crate::SpreadsheetValidationErrorKind::TooLarge
        );

        let cells = "<td>x</td>".repeat(MAX_SPREADSHEET_COLUMNS + 1);
        let wide = format!("<table><tr>{cells}</tr></table>");
        assert_eq!(
            parse_preview_rows(wide.as_bytes()).unwrap_err().kind(),
            crate::SpreadsheetValidationErrorKind::TooLarge
        );
    }

    #[test]
    fn decodes_legacy_chinese_and_normalizes_html_entities() {
        let html = "<table><tr><td>  建设&nbsp;银行 &amp; 测试&#13;完成 </td></tr></table>";
        let (encoded, _, had_errors) = GBK.encode(html);
        assert!(!had_errors);
        let preview = parse_preview_rows(&encoded).expect("GBK HTML spreadsheet");
        assert_eq!(preview.rows[0], ["建设 银行 & 测试 完成"]);
        assert!(decode(&[0x81]).is_err());
        assert_eq!(clean_cell("&lt;b&gt;  one\n two &gt;"), "<b> one two >");
    }

    #[test]
    fn ignores_empty_rows_and_enforces_total_row_limit() {
        let html = "<table><tr><td> </td></tr><tr><td>x</td></tr></table>";
        let preview = parse_preview_rows(html.as_bytes()).unwrap();
        assert_eq!(preview.total_rows, 1);
        assert_eq!(preview.rows, [vec!["x".to_string()]]);

        let rows = "<tr><td>x</td></tr>".repeat(MAX_SPREADSHEET_ROWS + 1);
        assert_eq!(
            parse_preview_rows(rows.as_bytes()).unwrap_err().kind(),
            crate::SpreadsheetValidationErrorKind::TooLarge
        );
    }
}
