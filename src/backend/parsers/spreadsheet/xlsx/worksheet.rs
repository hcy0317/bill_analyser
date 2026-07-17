use super::super::{
    SpreadsheetPreviewRows, SpreadsheetValidationError, MAX_SPREADSHEET_CELLS,
    MAX_SPREADSHEET_CELL_BYTES, MAX_SPREADSHEET_COLUMNS, MAX_SPREADSHEET_ROWS,
    MAX_SPREADSHEET_TOTAL_CELL_BYTES,
};
use super::xml::{
    collect_text_nodes, find_start_tag, first_text_node, parse_cell_reference, xml_attribute,
};

pub(super) fn parse_shared_strings(xml: &str) -> Result<Vec<String>, SpreadsheetValidationError> {
    let mut values = Vec::new();
    let mut total_bytes = 0usize;
    let mut cursor = 0usize;
    while let Some((_, tag_end, self_closing)) = find_start_tag(xml, "si", cursor) {
        if values.len() >= MAX_SPREADSHEET_CELLS {
            return Err(SpreadsheetValidationError::too_large(
                "Import preview shared strings exceed the cell limit",
            ));
        }
        let (body, next) = if self_closing {
            ("", tag_end.saturating_add(1))
        } else {
            let body_start = tag_end.saturating_add(1);
            let close = xml[body_start..]
                .find("</si>")
                .map(|offset| body_start.saturating_add(offset))
                .ok_or_else(|| {
                    SpreadsheetValidationError::invalid("Import preview shared strings are invalid")
                })?;
            (&xml[body_start..close], close.saturating_add(5))
        };
        let value = collect_text_nodes(body)?;
        if value.len() > MAX_SPREADSHEET_CELL_BYTES {
            return Err(SpreadsheetValidationError::too_large(
                "Import preview shared string exceeds the cell limit",
            ));
        }
        total_bytes = total_bytes
            .checked_add(value.len())
            .ok_or_else(byte_limit)?;
        if total_bytes > MAX_SPREADSHEET_TOTAL_CELL_BYTES {
            return Err(SpreadsheetValidationError::too_large(
                "Import preview shared strings exceed the byte limit",
            ));
        }
        values.push(value);
        cursor = next;
    }
    Ok(values)
}

pub(super) fn parse_worksheet_rows(
    xml: &str,
    shared_strings: &[String],
    materialized_row_limit: usize,
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    validate_dimension(xml)?;
    let mut sample = Vec::new();
    let mut cursor = 0usize;
    let mut total_cells = 0usize;
    let mut total_cell_bytes = 0usize;
    let mut total_rows = 0usize;
    let mut current_row_index = None;
    let mut current_row = Vec::new();
    let mut current_row_has_value = false;
    let mut current_column = 0usize;
    while let Some((start, tag_end, self_closing)) = find_start_tag(xml, "c", cursor) {
        total_cells = total_cells.checked_add(1).ok_or_else(cell_limit)?;
        if total_cells > MAX_SPREADSHEET_CELLS {
            return Err(cell_limit());
        }
        let tag = &xml[start..=tag_end];
        let reference = xml_attribute(tag, "r").ok_or_else(|| {
            SpreadsheetValidationError::invalid(
                "Import preview worksheet cell reference is missing",
            )
        })?;
        let (row, column) = parse_cell_reference(reference).ok_or_else(|| {
            SpreadsheetValidationError::invalid(
                "Import preview worksheet cell reference is invalid",
            )
        })?;
        if row > MAX_SPREADSHEET_ROWS || column > MAX_SPREADSHEET_COLUMNS {
            return Err(SpreadsheetValidationError::too_large(
                "Import preview worksheet cell reference exceeds the row or column limit",
            ));
        }
        match current_row_index {
            Some(previous_row) if row < previous_row => return Err(cell_order_error()),
            Some(previous_row) if row > previous_row => {
                finish_row(
                    &mut sample,
                    std::mem::take(&mut current_row),
                    current_row_has_value,
                    materialized_row_limit,
                    &mut total_rows,
                )?;
                current_row_has_value = false;
                current_column = 0;
                current_row_index = Some(row);
            }
            None => current_row_index = Some(row),
            _ => {}
        }
        if column <= current_column {
            return Err(cell_order_error());
        }
        current_column = column;
        let (body, next) = if self_closing {
            ("", tag_end.saturating_add(1))
        } else {
            let body_start = tag_end.saturating_add(1);
            let close = xml[body_start..]
                .find("</c>")
                .map(|offset| body_start.saturating_add(offset))
                .ok_or_else(|| {
                    SpreadsheetValidationError::invalid("Import preview worksheet cell is invalid")
                })?;
            (&xml[body_start..close], close.saturating_add(4))
        };
        let value = match xml_attribute(tag, "t").unwrap_or_default() {
            "s" => {
                let index = first_text_node(body, "v")?
                    .ok_or_else(|| {
                        SpreadsheetValidationError::invalid(
                            "Import preview shared string index is missing",
                        )
                    })?
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| {
                        SpreadsheetValidationError::invalid(
                            "Import preview shared string index is invalid",
                        )
                    })?;
                shared_strings.get(index).cloned().ok_or_else(|| {
                    SpreadsheetValidationError::invalid(
                        "Import preview shared string index is invalid",
                    )
                })?
            }
            "inlineStr" => collect_text_nodes(body)?,
            _ => first_text_node(body, "v")?.unwrap_or_default(),
        };
        if value.len() > MAX_SPREADSHEET_CELL_BYTES {
            return Err(SpreadsheetValidationError::too_large(
                "Import preview worksheet contains an oversized cell",
            ));
        }
        total_cell_bytes = total_cell_bytes
            .checked_add(value.len())
            .ok_or_else(byte_limit)?;
        if total_cell_bytes > MAX_SPREADSHEET_TOTAL_CELL_BYTES {
            return Err(byte_limit());
        }
        current_row_has_value |= !value.is_empty();
        if sample.len() < materialized_row_limit {
            current_row.resize(column, String::new());
            if let Some(slot) = current_row.get_mut(column.saturating_sub(1)) {
                *slot = value;
            }
        }
        cursor = next;
    }
    if current_row_index.is_some() {
        finish_row(
            &mut sample,
            current_row,
            current_row_has_value,
            materialized_row_limit,
            &mut total_rows,
        )?;
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

fn finish_row(
    sample: &mut Vec<Vec<String>>,
    row: Vec<String>,
    has_value: bool,
    materialized_row_limit: usize,
    total_rows: &mut usize,
) -> Result<(), SpreadsheetValidationError> {
    if !has_value {
        return Ok(());
    }
    *total_rows = total_rows.checked_add(1).ok_or_else(shape_limit)?;
    if sample.len() < materialized_row_limit {
        sample.push(row);
    }
    Ok(())
}

fn validate_dimension(xml: &str) -> Result<(), SpreadsheetValidationError> {
    let dimension = xml
        .find("<dimension")
        .and_then(|start| xml[start..].find('>').map(|end| &xml[start..start + end]))
        .and_then(|tag| xml_attribute(tag, "ref"))
        .ok_or_else(|| {
            SpreadsheetValidationError::invalid("Import preview worksheet has no bounded dimension")
        })?;
    let mut endpoints = dimension.split(':');
    let start = endpoints
        .next()
        .and_then(parse_cell_reference)
        .ok_or_else(|| {
            SpreadsheetValidationError::invalid("Import preview worksheet dimension is invalid")
        })?;
    let end = endpoints
        .next()
        .and_then(parse_cell_reference)
        .unwrap_or(start);
    if end.0 < start.0 || end.1 < start.1 {
        return Err(SpreadsheetValidationError::invalid(
            "Import preview worksheet dimension is invalid",
        ));
    }
    let rows = end.0 - start.0 + 1;
    let columns = end.1 - start.1 + 1;
    let cells = rows.checked_mul(columns).ok_or_else(shape_limit)?;
    if end.0 > MAX_SPREADSHEET_ROWS
        || end.1 > MAX_SPREADSHEET_COLUMNS
        || cells > MAX_SPREADSHEET_CELLS
    {
        return Err(shape_limit());
    }
    if cell_tag_count(xml) > MAX_SPREADSHEET_CELLS {
        return Err(cell_limit());
    }
    Ok(())
}

fn cell_tag_count(xml: &str) -> usize {
    xml.as_bytes()
        .windows(3)
        .filter(|window| {
            window[0] == b'<'
                && window[1] == b'c'
                && matches!(window[2], b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/')
        })
        .count()
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

fn cell_order_error() -> SpreadsheetValidationError {
    SpreadsheetValidationError::invalid(
        "Import preview worksheet cell references are not strictly ordered",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spreadsheet::SPREADSHEET_PREVIEW_SAMPLE_ROWS;

    #[test]
    fn dimension_and_cell_count_are_bounded() {
        assert!(validate_dimension(
            r#"<worksheet><dimension ref="A1:B2"/><c r="A1"/></worksheet>"#
        )
        .is_ok());
        assert_eq!(
            validate_dimension(r#"<worksheet><dimension ref="A1:XFD1048576"/></worksheet>"#)
                .unwrap_err()
                .kind(),
            crate::SpreadsheetValidationErrorKind::TooLarge
        );
        assert!(validate_dimension("<worksheet/>").is_err());
    }

    #[test]
    fn shared_strings_cover_empty_rich_and_invalid_entries() {
        assert_eq!(
            parse_shared_strings("<sst><si/><si><r><t>a</t></r><r><t>&amp;b</t></r></si></sst>")
                .unwrap(),
            ["", "a&b"]
        );
        assert!(parse_shared_strings("<sst><si><t>x</t></sst>").is_err());
        let oversized = format!(
            "<sst><si><t>{}</t></si></sst>",
            "x".repeat(MAX_SPREADSHEET_CELL_BYTES + 1)
        );
        assert_eq!(
            parse_shared_strings(&oversized).unwrap_err().kind(),
            crate::SpreadsheetValidationErrorKind::TooLarge
        );
    }

    #[test]
    fn worksheet_cells_require_valid_references_and_shared_string_indexes() {
        let base = |cell: &str| {
            format!(
                r#"<worksheet><dimension ref="A1:A1"/><sheetData>{cell}</sheetData></worksheet>"#
            )
        };
        for cell in [
            "<c><v>1</v></c>",
            r#"<c r="A0"><v>1</v></c>"#,
            r#"<c r="A1" t="s"></c>"#,
            r#"<c r="A1" t="s"><v>x</v></c>"#,
            r#"<c r="A1" t="s"><v>1</v></c>"#,
        ] {
            assert!(parse_worksheet_rows(
                &base(cell),
                &["only".to_string()],
                SPREADSHEET_PREVIEW_SAMPLE_ROWS
            )
            .is_err());
        }
        assert!(
            parse_worksheet_rows(&base(r#"<c r="A1">"#), &[], SPREADSHEET_PREVIEW_SAMPLE_ROWS)
                .is_err()
        );
        assert!(parse_worksheet_rows(
            &base(r#"<c r="A1"/>"#),
            &[],
            SPREADSHEET_PREVIEW_SAMPLE_ROWS
        )
        .is_err());
    }

    #[test]
    fn dimensions_reject_reverse_ranges_and_count_only_real_cell_tags() {
        assert!(validate_dimension(r#"<worksheet><dimension ref="B2:A1"/></worksheet>"#).is_err());
        assert!(validate_dimension(r#"<worksheet><dimension ref="bad"/></worksheet>"#).is_err());
        assert_eq!(
            cell_tag_count("<cells><custom/><c r='A1'/><c>v</c></cells>"),
            2
        );
    }

    #[test]
    fn worksheet_streams_only_requested_rows_and_rejects_out_of_order_cells() {
        let ordered = r#"<worksheet><dimension ref="A1:B3"/><sheetData>
            <c r="A1"><v>one</v></c><c r="B1"><v>1</v></c>
            <c r="A2"><v>two</v></c><c r="A3"><v>three</v></c>
        </sheetData></worksheet>"#;
        let parsed = parse_worksheet_rows(ordered, &[], 1).expect("bounded worksheet");
        assert_eq!(parsed.total_rows, 3);
        assert_eq!(parsed.rows, [vec!["one".to_string(), "1".to_string()]]);

        let out_of_order = r#"<worksheet><dimension ref="A1:A2"/><sheetData>
            <c r="A2"><v>two</v></c><c r="A1"><v>one</v></c>
        </sheetData></worksheet>"#;
        assert_eq!(
            parse_worksheet_rows(out_of_order, &[], 1)
                .expect_err("out-of-order cells")
                .kind(),
            crate::SpreadsheetValidationErrorKind::Invalid
        );
    }
}
