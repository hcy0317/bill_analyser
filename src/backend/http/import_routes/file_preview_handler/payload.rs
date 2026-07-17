use super::super::*;
use super::text::preview_text_rows;

const IMPORT_FILE_PREVIEW_SAMPLE_LIMIT: usize = 300;

pub(super) fn import_file_preview_payload(
    filename: &str,
    bytes: &[u8],
    delimiter_hint: Option<&str>,
    encoding: &str,
) -> Result<Value, ImportV2RouteResponse> {
    let extension = FsPath::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let (rows, delimiter, raw_total_rows, applied_encoding) = match extension.as_str() {
        "csv" | "txt" => {
            let requested_encoding = super::text::normalize_preview_encoding(encoding)?;
            preview_text_rows(bytes, delimiter_hint, requested_encoding)?
        }
        "xls" | "xlsx" => {
            let preview = bill_analyser_parsers::parse_spreadsheet_preview_rows(bytes)
                .map_err(spreadsheet_validation_error_response)?;
            (preview.rows, None, preview.total_rows, "auto".to_string())
        }
        _ => {
            return Err(import_v2_error_response(
                415,
                "Import preview file type is not supported",
            ))
        }
    };
    let (rows, header_offset) = trim_preview_rows_to_header(rows);
    let Some(headers) = rows.first().cloned().filter(|headers| !headers.is_empty()) else {
        return Err(import_v2_error_response(400, "No previewable rows found"));
    };
    let total_rows = raw_total_rows.saturating_sub(header_offset.saturating_add(1));
    let sample_data = rows
        .iter()
        .take(IMPORT_FILE_PREVIEW_SAMPLE_LIMIT)
        .cloned()
        .collect::<Vec<_>>();
    let preview_rows = sample_data.iter().skip(1).cloned().collect::<Vec<_>>();
    Ok(json!({
        "headers": headers,
        "sampleData": sample_data,
        "previewRows": preview_rows,
        "totalRows": total_rows,
        "encoding": applied_encoding,
        "delimiter": delimiter.map(|value| delimiter_to_response(Some(value))),
    }))
}

pub(super) fn trim_preview_rows_to_header(rows: Vec<Vec<String>>) -> (Vec<Vec<String>>, usize) {
    let start = rows
        .iter()
        .position(|row| looks_like_import_headers(row.iter().map(String::as_str)))
        .or_else(|| {
            rows.iter()
                .position(|row| row.iter().filter(|value| !value.trim().is_empty()).count() > 1)
        })
        .unwrap_or(0);
    (rows.into_iter().skip(start).collect(), start)
}
