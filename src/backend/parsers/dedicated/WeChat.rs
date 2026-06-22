// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use crate::{post_process_raw_bills, RawBill, StandardBill};

use super::common::{
    csv_records_from_text, decode_text, file_suffix, get, html_payload_contains_any, rows_to_maps,
    sheet_or_html_rows, RowMap,
};

/// 解析微信支付导出文件，按扩展名分派 CSV/TXT 或 Excel/HTML 表格分支。
#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn parse(filename: &str, bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parse",
        "business operation entered"
    );
    let suffix = file_suffix(filename);
    match suffix.as_str() {
        "csv" | "txt" => parse_csv(filename, bytes),
        "xlsx" | "xls" => parse_sheet_or_html(bytes),
        _ => Vec::new(),
    }
}

/// 解析微信 CSV/TXT 内容，使用文件名、账单标记和表头共同确认来源。
#[tracing::instrument(level = "debug", skip_all)]
fn parse_csv(filename: &str, bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parse_csv",
        "business operation entered"
    );
    let text = decode_text(bytes);
    let filename_hint =
        filename.to_ascii_lowercase().contains("wechat") || filename.contains("微信");
    let has_wechat_marker = text
        .lines()
        .take(8)
        .any(|line| line.contains("微信支付账单"));
    let has_wechat_export_header = text
        .lines()
        .any(|line| row_text_like_header(line) && line.contains("金额(元)"));
    if !has_wechat_marker && !has_wechat_export_header && !filename_hint {
        return Vec::new();
    }
    let Some((rows, _)) = csv_records_from_text(&text, row_text_like_header) else {
        return Vec::new();
    };
    post_process_raw_bills(
        "wechat",
        &rows.iter().filter_map(raw_wechat).collect::<Vec<_>>(),
    )
}

/// 判断一行文本是否符合微信账单业务表头，兼容不同导出字段命名。
fn row_text_like_header(line: &str) -> bool {
    line.contains("交易时间")
        && (line.contains("金额(元)") || line.contains("金额"))
        && (line.contains("收/支") || line.contains("收支类型") || line.contains("交易类型"))
        && (line.contains("商品") || line.contains("交易对方") || line.contains("支付方式"))
}

/// 将微信表格行映射为 RawBill，保留商品、交易单号、商户单号和原始类型。
#[tracing::instrument(level = "debug", skip_all)]
fn raw_wechat(row: &RowMap) -> Option<RawBill> {
    let date = get(row, &["交易时间"]);
    if date.is_empty() || date.contains("总计") {
        return None;
    }
    Some(RawBill {
        trade_time: date,
        transaction_type: get(row, &["收/支", "收支类型", "交易类型"]),
        counterparty: get(row, &["交易对方"]),
        goods: get(row, &["商品"]),
        amount: get(row, &["金额(元)", "金额"]),
        payment_method: get(row, &["支付方式"]),
        status: get(row, &["当前状态"]),
        transaction_id: get(row, &["交易单号"]),
        merchant_id: get(row, &["商户单号"]),
        remark: get(row, &["备注"]),
        original_category: get(row, &["交易类型"]),
        ..Default::default()
    })
}

/// 解析微信 Excel 或 HTML 表格导出，要求前几行包含微信账单标记。
#[tracing::instrument(level = "debug", skip_all)]
fn parse_sheet_or_html(bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parse_sheet_or_html",
        "business operation entered"
    );
    if html_payload_contains_any(bytes, &["微信支付账单", "交易对方", "金额(元)", "收/支"])
        == Some(false)
    {
        return Vec::new();
    }
    let rows = sheet_or_html_rows(bytes);
    if !rows
        .iter()
        .take(5)
        .any(|row| row.join(" ").contains("微信支付账单"))
    {
        return Vec::new();
    }
    let maps = rows_to_maps(&rows, |row| row_text_like_header(&row.join(",")));
    post_process_raw_bills(
        "wechat",
        &maps.iter().filter_map(raw_wechat).collect::<Vec<_>>(),
    )
}
