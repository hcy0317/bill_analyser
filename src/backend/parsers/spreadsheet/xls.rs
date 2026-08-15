use std::{io::Cursor, panic::AssertUnwindSafe};

use calamine::{open_workbook_auto_from_rs, Data, Reader};

use super::{
    SpreadsheetPreviewRows, SpreadsheetValidationError, MAX_SPREADSHEET_CELLS,
    MAX_SPREADSHEET_CELL_BYTES, MAX_SPREADSHEET_COLUMNS, MAX_SPREADSHEET_ROWS,
    MAX_SPREADSHEET_TOTAL_CELL_BYTES,
};

pub(super) fn parse_preview_rows(
    bytes: &[u8],
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    parse_rows(bytes, super::SPREADSHEET_PREVIEW_SAMPLE_ROWS)
}

pub(super) fn parse_rows(
    bytes: &[u8],
    materialized_row_limit: usize,
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    super::xls_preflight::validate(bytes)?;
    parse_calamine_rows(bytes, materialized_row_limit)
}

pub(super) fn parse_calamine_rows(
    bytes: &[u8],
    materialized_row_limit: usize,
) -> Result<SpreadsheetPreviewRows, SpreadsheetValidationError> {
    let range = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let cursor = Cursor::new(bytes);
        let mut workbook = open_workbook_auto_from_rs(cursor).map_err(|_| invalid())?;
        workbook
            .worksheet_range_at(0)
            .ok_or_else(|| {
                SpreadsheetValidationError::invalid("Import spreadsheet has no worksheet")
            })?
            .map_err(|_| invalid())
    }))
    .map_err(|_| invalid())??;

    let total_rows = range.height();
    let total_columns = range.width();
    validate_materialized_shape(total_rows, total_columns)?;

    let mut total_cell_bytes = 0usize;
    let mut rows = Vec::with_capacity(total_rows.min(materialized_row_limit));
    for (row_index, row) in range.rows().enumerate() {
        let mut materialized =
            (row_index < materialized_row_limit).then(|| Vec::with_capacity(total_columns));
        for cell in row {
            let value = cell_to_string(cell);
            add_cell_bytes(&mut total_cell_bytes, value.len())?;
            if let Some(output) = materialized.as_mut() {
                output.push(value);
            }
        }
        if let Some(output) = materialized {
            rows.push(output);
        }
    }

    Ok(SpreadsheetPreviewRows { rows, total_rows })
}

fn validate_materialized_shape(
    total_rows: usize,
    total_columns: usize,
) -> Result<(), SpreadsheetValidationError> {
    if total_rows > MAX_SPREADSHEET_ROWS || total_columns > MAX_SPREADSHEET_COLUMNS {
        return Err(SpreadsheetValidationError::too_large(
            "Import spreadsheet exceeds the row or column limit",
        ));
    }
    let cell_count = total_rows
        .checked_mul(total_columns)
        .ok_or_else(cell_limit)?;
    if cell_count > MAX_SPREADSHEET_CELLS {
        return Err(cell_limit());
    }
    Ok(())
}

fn add_cell_bytes(
    total_cell_bytes: &mut usize,
    value_bytes: usize,
) -> Result<(), SpreadsheetValidationError> {
    if value_bytes > MAX_SPREADSHEET_CELL_BYTES {
        return Err(SpreadsheetValidationError::too_large(
            "Import spreadsheet cell exceeds the byte limit",
        ));
    }
    *total_cell_bytes = total_cell_bytes
        .checked_add(value_bytes)
        .ok_or_else(total_cell_byte_limit)?;
    if *total_cell_bytes > MAX_SPREADSHEET_TOTAL_CELL_BYTES {
        return Err(total_cell_byte_limit());
    }
    Ok(())
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::Float(value) if value.fract() == 0.0 => format!("{value:.0}"),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        _ => cell.to_string().trim().to_string(),
    }
}

fn invalid() -> SpreadsheetValidationError {
    SpreadsheetValidationError::invalid("Import preview spreadsheet content is invalid")
}

fn cell_limit() -> SpreadsheetValidationError {
    SpreadsheetValidationError::too_large("Import spreadsheet exceeds the cell limit")
}

fn total_cell_byte_limit() -> SpreadsheetValidationError {
    SpreadsheetValidationError::too_large(
        "Import spreadsheet materialized cells exceed the byte limit",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xls_materialization_helpers_cover_limits_and_cell_types() {
        let bytes =
            include_bytes!("../../../../tests/fixtures/import_samples/abc_statement_sample.xls");
        let preview = parse_rows(bytes, 0).expect("bounded XLS parses without materialized rows");
        assert!(preview.rows.is_empty());
        assert_eq!(preview.total_rows, 4);

        assert!(validate_materialized_shape(MAX_SPREADSHEET_ROWS + 1, 1).is_err());
        assert!(validate_materialized_shape(1, MAX_SPREADSHEET_COLUMNS + 1).is_err());
        assert!(validate_materialized_shape(usize::MAX, 2).is_err());
        assert!(validate_materialized_shape(10_000, 101).is_err());
        assert!(validate_materialized_shape(2, 2).is_ok());

        let mut total = 0usize;
        assert!(add_cell_bytes(&mut total, MAX_SPREADSHEET_CELL_BYTES + 1).is_err());
        total = usize::MAX;
        assert!(add_cell_bytes(&mut total, 1).is_err());
        total = MAX_SPREADSHEET_TOTAL_CELL_BYTES;
        assert!(add_cell_bytes(&mut total, 1).is_err());
        total = 0;
        assert!(add_cell_bytes(&mut total, 1).is_ok());

        assert_eq!(cell_to_string(&Data::Empty), "");
        assert_eq!(cell_to_string(&Data::Float(2.0)), "2");
        assert_eq!(cell_to_string(&Data::Float(2.5)), "2.5");
        assert_eq!(cell_to_string(&Data::Int(3)), "3");
        assert_eq!(
            cell_to_string(&Data::String(" value ".to_string())),
            "value"
        );
        assert_eq!(
            invalid().kind(),
            super::super::SpreadsheetValidationErrorKind::Invalid
        );
        assert_eq!(
            cell_limit().kind(),
            super::super::SpreadsheetValidationErrorKind::TooLarge
        );
        assert_eq!(
            total_cell_byte_limit().kind(),
            super::super::SpreadsheetValidationErrorKind::TooLarge
        );
    }
}
