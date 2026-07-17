use super::super::*;

const TEXT_PREVIEW_PARSE_SAMPLE_LIMIT: usize = 512;
const TEXT_PREVIEW_MAX_TOTAL_ROWS: usize = 50_000;
const TEXT_PREVIEW_MAX_COLUMNS: usize = 128;
const TEXT_PREVIEW_MAX_CELL_BYTES: usize = 16 * 1024;
const TEXT_PREVIEW_MAX_TOTAL_CELLS: usize = 1_000_000;

pub(super) type TextPreviewRows = (Vec<Vec<String>>, Option<char>, usize, String);

pub(super) fn preview_text_rows(
    bytes: &[u8],
    delimiter_hint: Option<&str>,
    encoding: &str,
) -> Result<TextPreviewRows, ImportV2RouteResponse> {
    if bytes.contains(&0) {
        return Err(import_v2_error_response(
            400,
            "Import preview text file contains binary data",
        ));
    }
    let (text, applied_encoding) = decode_preview_text(bytes, encoding)?;
    let detected =
        detect_csv_table_start(&text, true).or_else(|| detect_csv_table_start(&text, false));
    let start_line = detected.map(|(index, _)| index).unwrap_or(0);
    let delimiter = delimiter_from_hint(delimiter_hint)
        .or_else(|| detected.map(|(_, delimiter)| delimiter))
        .unwrap_or(',');
    let table = table_text_from_line(&text, start_line);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter as u8)
        .from_reader(table.as_bytes());
    let mut rows = Vec::new();
    let mut total_rows = 0usize;
    let mut total_cells = 0usize;
    for record in reader.records() {
        let record =
            record.map_err(|_| import_v2_error_response(400, "Invalid import preview table"))?;
        if record.len() > TEXT_PREVIEW_MAX_COLUMNS {
            return Err(import_v2_error_response(
                400,
                "Import preview table has too many columns",
            ));
        }
        if record
            .iter()
            .any(|value| value.len() > TEXT_PREVIEW_MAX_CELL_BYTES)
        {
            return Err(import_v2_error_response(
                400,
                "Import preview table contains an oversized cell",
            ));
        }
        total_rows = total_rows.checked_add(1).ok_or_else(|| {
            import_v2_error_response(413, "Import preview table exceeds the row limit")
        })?;
        if total_rows > TEXT_PREVIEW_MAX_TOTAL_ROWS {
            return Err(import_v2_error_response(
                413,
                "Import preview table exceeds the row limit",
            ));
        }
        total_cells = total_cells.checked_add(record.len()).ok_or_else(|| {
            import_v2_error_response(413, "Import preview table exceeds the cell limit")
        })?;
        if total_cells > TEXT_PREVIEW_MAX_TOTAL_CELLS {
            return Err(import_v2_error_response(
                413,
                "Import preview table exceeds the cell limit",
            ));
        }
        let row = record
            .iter()
            .map(|value| value.trim().to_string())
            .collect::<Vec<_>>();
        if rows.len() < TEXT_PREVIEW_PARSE_SAMPLE_LIMIT {
            rows.push(row);
        }
    }
    Ok((rows, Some(delimiter), total_rows, applied_encoding))
}

pub(in crate::import_routes) fn normalize_preview_encoding(
    encoding: &str,
) -> Result<&'static str, ImportV2RouteResponse> {
    match encoding
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .as_str()
    {
        "" | "auto" => Ok("auto"),
        "utf-8" | "utf8" => Ok("utf-8"),
        "gbk" | "cp936" => Ok("gbk"),
        "gb18030" => Ok("gb18030"),
        _ => Err(import_v2_error_response(
            400,
            "Import preview encoding is not supported",
        )),
    }
}

pub(in crate::import_routes) fn decode_preview_text(
    bytes: &[u8],
    encoding: &str,
) -> Result<(String, String), ImportV2RouteResponse> {
    let decoded = match encoding {
        "utf-8" => std::str::from_utf8(bytes)
            .map(|text| (text.to_string(), "utf-8"))
            .map_err(|_| import_v2_error_response(400, "Import preview text is not valid UTF-8"))?,
        "gbk" => {
            let (text, had_errors) = GBK.decode_without_bom_handling(bytes);
            if had_errors {
                return Err(import_v2_error_response(
                    400,
                    "Import preview text is not valid GBK",
                ));
            }
            (text.into_owned(), "gbk")
        }
        "gb18030" => {
            let (text, had_errors) = GB18030.decode_without_bom_handling(bytes);
            if had_errors {
                return Err(import_v2_error_response(
                    400,
                    "Import preview text is not valid GB18030",
                ));
            }
            (text.into_owned(), "gb18030")
        }
        "auto" => {
            if let Ok(text) = std::str::from_utf8(bytes) {
                (text.to_string(), "utf-8")
            } else {
                let (text, had_errors) = GB18030.decode_without_bom_handling(bytes);
                if had_errors {
                    return Err(import_v2_error_response(
                        400,
                        "Import preview text encoding could not be detected",
                    ));
                }
                (text.into_owned(), "gb18030")
            }
        }
        _ => {
            return Err(import_v2_error_response(
                400,
                "Import preview encoding is not supported",
            ))
        }
    };
    Ok((
        decoded.0.trim_start_matches('\u{feff}').to_string(),
        decoded.1.to_string(),
    ))
}
