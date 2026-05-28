// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn export_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillsExportQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "export_bills_handler", "business operation entered");
    let export_format = match BillExportFormat::normalize(query.format.as_deref()) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let bills = match list_postgres_bills_for_export(runtime.pool(), user_id).await {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
        return export_bills_response(export_format, &bills);
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let bills = match list_bills(runtime.connection(), user_id, &BillFilters::default()) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    export_bills_response(export_format, &bills)
}

async fn list_postgres_bills_for_export(
    pool: &PostgresPool,
    user_id: UserId,
) -> bill_analyser_db::DbResult<Vec<BillRecord>> {
    let mut page = 1_usize;
    let page_size = 500_usize;
    let mut bills = Vec::new();
    loop {
        let page_result = query_postgres_bills(
            pool,
            user_id.get() as i64,
            page,
            page_size,
            &BillFilters::default(),
        )
        .await?;
        if page_result.bills.is_empty() {
            break;
        }
        let total = page_result.total;
        bills.extend(page_result.bills);
        if i64::try_from(bills.len()).unwrap_or(i64::MAX) >= total {
            break;
        }
        page += 1;
    }
    Ok(bills)
}

fn export_bills_response(export_format: BillExportFormat, bills: &[BillRecord]) -> Response {
    if bills.is_empty() {
        return not_found("No bills to export");
    }

    match export_format {
        BillExportFormat::Csv => match render_bills_csv_export(bills) {
            Ok(body) => {
                export_file_response("text/csv; charset=utf-8", export_filename("csv"), body)
            }
            Err(_) => db_error_response(),
        },
        BillExportFormat::Excel => export_file_response(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            export_filename("xlsx"),
            render_bills_xlsx_export(bills),
        ),
    }
}

enum BillExportFormat {
    Csv,
    Excel,
}

impl BillExportFormat {
    fn normalize(value: Option<&str>) -> RouteResult<Self> {
        let normalized = value.unwrap_or("csv").trim().to_ascii_lowercase();
        match normalized.as_str() {
            "" | "csv" => Ok(Self::Csv),
            "excel" | "xlsx" | "xls" => Ok(Self::Excel),
            _ => Err(Box::new(bad_request("Unsupported export format"))),
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn export_filename(extension: &str) -> String {
    format!(
        "bills_export_{}.{}",
        Local::now().format("%Y%m%d_%H%M%S"),
        extension
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn export_file_response(content_type: &'static str, filename: String, body: Vec<u8>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content_type)
        .header(
            "content-disposition",
            format!("attachment; filename={filename}"),
        )
        .body(Body::from(body))
        .unwrap_or_else(|_| db_error_response())
}

#[tracing::instrument(level = "debug", skip_all)]
fn render_bills_csv_export(bills: &[BillRecord]) -> Result<Vec<u8>, csv::Error> {
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    writer.write_record(EXPORT_COLUMNS.iter().map(|(label, _)| *label))?;
    for bill in bills {
        writer.write_record(
            EXPORT_COLUMNS
                .iter()
                .map(|(_, key)| export_bill_value(bill, key)),
        )?;
    }
    let mut body = "\u{feff}".as_bytes().to_vec();
    body.extend(writer.into_inner().map_err(|error| error.into_error())?);
    Ok(body)
}

#[tracing::instrument(level = "debug", skip_all)]
fn render_bills_xlsx_export(bills: &[BillRecord]) -> Vec<u8> {
    let sheet_xml = render_bills_xlsx_sheet(bills);
    let entries = [
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.as_slice(),
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.as_slice(),
        ),
        (
            "xl/workbook.xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Bills" sheetId="1" r:id="rId1"/></sheets></workbook>"#.as_slice(),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.as_slice(),
        ),
        ("xl/worksheets/sheet1.xml", sheet_xml.as_bytes()),
    ];
    render_stored_zip(&entries)
}

#[tracing::instrument(level = "debug", skip_all)]
fn render_bills_xlsx_sheet(bills: &[BillRecord]) -> String {
    let mut sheet = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    render_xlsx_row(
        &mut sheet,
        1,
        EXPORT_COLUMNS.iter().map(|(label, _)| label.to_string()),
    );
    for (index, bill) in bills.iter().enumerate() {
        render_xlsx_row(
            &mut sheet,
            index + 2,
            EXPORT_COLUMNS
                .iter()
                .map(|(_, key)| export_bill_value(bill, key)),
        );
    }
    sheet.push_str("</sheetData></worksheet>");
    sheet
}

#[tracing::instrument(level = "debug", skip_all)]
fn render_xlsx_row<I>(sheet: &mut String, row_number: usize, cells: I)
where
    I: IntoIterator<Item = String>,
{
    let _ = write!(sheet, r#"<row r="{row_number}">"#);
    for (column_index, cell) in cells.into_iter().enumerate() {
        let reference = xlsx_cell_reference(column_index, row_number);
        let _ = write!(
            sheet,
            r#"<c r="{reference}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
            xml_escape(&cell)
        );
    }
    sheet.push_str("</row>");
}

fn xlsx_cell_reference(mut column_index: usize, row_number: usize) -> String {
    let mut column = Vec::new();
    loop {
        let remainder = column_index % 26;
        column.push((b'A' + u8::try_from(remainder).unwrap_or(0)) as char);
        column_index /= 26;
        if column_index == 0 {
            break;
        }
        column_index -= 1;
    }
    column.iter().rev().collect::<String>() + &row_number.to_string()
}

fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[tracing::instrument(level = "debug", skip_all)]
fn export_bill_value(bill: &BillRecord, key: &str) -> String {
    match bill.get(key) {
        Some(Value::String(text)) => serialize_optional_export_cell(key, Some(text)),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(value) => serialize_optional_export_cell(key, Some(value.to_string())),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn render_stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    let mut central_directory = Vec::new();
    for (name, data) in entries {
        let offset = u32::try_from(body.len()).unwrap_or(u32::MAX);
        write_zip_local_file(&mut body, name, data);
        write_zip_central_directory_file(&mut central_directory, name, data, offset);
    }
    let central_directory_offset = u32::try_from(body.len()).unwrap_or(u32::MAX);
    let central_directory_size = u32::try_from(central_directory.len()).unwrap_or(u32::MAX);
    body.extend(central_directory);
    write_zip_end_of_central_directory(
        &mut body,
        u16::try_from(entries.len()).unwrap_or(u16::MAX),
        central_directory_size,
        central_directory_offset,
    );
    body
}

fn write_zip_local_file(body: &mut Vec<u8>, name: &str, data: &[u8]) {
    write_u32_le(body, 0x0403_4b50);
    write_u16_le(body, 20);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_zip_data_descriptor(body, name, data);
    body.extend(name.as_bytes());
    body.extend(data);
}

fn write_zip_central_directory_file(
    body: &mut Vec<u8>,
    name: &str,
    data: &[u8],
    local_header_offset: u32,
) {
    write_u32_le(body, 0x0201_4b50);
    write_u16_le(body, 20);
    write_u16_le(body, 20);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_zip_data_descriptor(body, name, data);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u32_le(body, 0);
    write_u32_le(body, local_header_offset);
    body.extend(name.as_bytes());
}

fn write_zip_data_descriptor(body: &mut Vec<u8>, name: &str, data: &[u8]) {
    write_u32_le(body, crc32(data));
    let data_len = u32::try_from(data.len()).unwrap_or(u32::MAX);
    write_u32_le(body, data_len);
    write_u32_le(body, data_len);
    write_u16_le(body, u16::try_from(name.len()).unwrap_or(u16::MAX));
    write_u16_le(body, 0);
}

fn write_zip_end_of_central_directory(
    body: &mut Vec<u8>,
    entry_count: u16,
    central_directory_size: u32,
    central_directory_offset: u32,
) {
    write_u32_le(body, 0x0605_4b50);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, entry_count);
    write_u16_le(body, entry_count);
    write_u32_le(body, central_directory_size);
    write_u32_le(body, central_directory_offset);
    write_u16_le(body, 0);
}

fn write_u16_le(body: &mut Vec<u8>, value: u16) {
    body.extend(value.to_le_bytes());
}

fn write_u32_le(body: &mut Vec<u8>, value: u32) {
    body.extend(value.to_le_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
