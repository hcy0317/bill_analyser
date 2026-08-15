// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use crate::{post_process_raw_bills, RawBill, StandardBill};

use super::common::{
    compact_date, compact_time, file_suffix, get, parse_amount, positive_amount_text,
    row_contains_all, rows_to_maps, RowMap,
};
use super::types::DedicatedParserInput;

/// 解析建设银行导出文件，目前只接受 Excel/HTML 表格来源。
#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn parse(input: &DedicatedParserInput<'_>) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parse",
        "business operation entered"
    );
    let suffix = file_suffix(input.filename());
    match suffix.as_str() {
        "xlsx" | "xls" => parse_sheet_or_html(input),
        _ => Vec::new(),
    }
}

/// 解析建设银行 Excel/HTML 表格，使用银行名称和记账日/收支列确认来源。
#[tracing::instrument(level = "debug", skip_all)]
fn parse_sheet_or_html(input: &DedicatedParserInput<'_>) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parse_sheet_or_html",
        "business operation entered"
    );
    let Some(spreadsheet) = input.spreadsheet() else {
        return Vec::new();
    };
    if !spreadsheet.contains("建设银行")
        && !spreadsheet.contains("China Construction Bank")
        && !spreadsheet.contains_all(&["记账日", "交易日期", "支出", "收入"])
    {
        return Vec::new();
    }
    let maps = rows_to_maps(spreadsheet.rows(), |row| {
        row_contains_all(row, &["记账日", "交易日期"])
    });
    post_process_raw_bills("ccb", &maps.iter().filter_map(raw_ccb).collect::<Vec<_>>())
}

/// 将建设银行表格行映射为 RawBill，分别从收入/支出列确定金额方向。
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

/// 组合建设银行交易日期和压缩时间，生成 parser 层标准时间文本。
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
