// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use crate::{post_process_raw_bills, RawBill, StandardBill};

use super::common::{
    compact_date, compact_time, contains_all_text, file_suffix, get, html_rows, parse_amount,
    positive_amount_text, row_contains_all, rows_to_maps, workbook_rows, RowMap,
};

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn parse(filename: &str, bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "import_parser",
        operation = "parse",
        "business operation entered"
    );
    let suffix = file_suffix(filename);
    match suffix.as_str() {
        "xlsx" | "xls" => parse_sheet_or_html(bytes),
        _ => Vec::new(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_sheet_or_html(bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "import_parser",
        operation = "parse_sheet_or_html",
        "business operation entered"
    );
    let rows = workbook_rows(bytes).unwrap_or_else(|| html_rows(bytes));
    let content = rows.iter().flatten().cloned().collect::<Vec<_>>().join(" ");
    if !content.contains("建设银行")
        && !content.contains("China Construction Bank")
        && !contains_all_text(&content, &["记账日", "交易日期", "支出", "收入"])
    {
        return Vec::new();
    }
    let maps = rows_to_maps(&rows, |row| row_contains_all(row, &["记账日", "交易日期"]));
    post_process_raw_bills("ccb", &maps.iter().filter_map(raw_ccb).collect::<Vec<_>>())
}

#[tracing::instrument(level = "debug", skip_all)]
fn raw_ccb(row: &RowMap) -> Option<RawBill> {
    let trade_time = build_trade_time(row);
    if trade_time.is_empty() {
        return None;
    }
    let debit = parse_amount(&get(row, &["支出"])).unwrap_or_default();
    let credit = parse_amount(&get(row, &["收入"])).unwrap_or_default();
    let (transaction_type, amount) = if credit > 0.0 {
        ("收入", credit)
    } else if debit > 0.0 {
        ("支出", debit)
    } else {
        return None;
    };
    let description = get(row, &["摘要"]);
    Some(RawBill {
        trade_time,
        transaction_type: transaction_type.to_string(),
        counterparty: {
            let counterparty = get(row, &["对方户名"]);
            if counterparty.is_empty() {
                description.clone()
            } else {
                counterparty
            }
        },
        description,
        amount: positive_amount_text(amount),
        channel: "建设银行".to_string(),
        ..Default::default()
    })
}

fn build_trade_time(row: &RowMap) -> String {
    let date = compact_date(&get(row, &["交易日期", "记账日"]));
    if date.is_empty() {
        return String::new();
    }
    let time = compact_time(&get(row, &["交易时间"]));
    if time.is_empty() {
        date
    } else {
        format!("{date} {time}")
    }
}
