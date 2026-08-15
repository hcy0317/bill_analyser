// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use crate::{post_process_raw_bills, RawBill, StandardBill};

use super::common::{
    contains_all_text, csv_records_from_text, decode_text, file_suffix, get, parse_amount,
    positive_amount_text, rows_to_maps, RowMap,
};
use super::types::DedicatedParserInput;

/// 解析工商银行导出文件，按扩展名分派 CSV/TXT 或 Excel/HTML 表格分支。
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
        "csv" | "txt" => parse_csv(input.bytes()),
        "xlsx" | "xls" => parse_sheet_or_html(input),
        _ => Vec::new(),
    }
}

/// 解析工商银行 CSV/TXT 内容，先排除其他银行标记再确认工行表头。
#[tracing::instrument(level = "debug", skip_all)]
fn parse_csv(bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parse_csv",
        "business operation entered"
    );
    let text = decode_text(bytes);
    let probe = text.lines().take(10).collect::<Vec<_>>().join("\n");
    if ["民生银行", "农业银行", "建设银行"]
        .iter()
        .any(|bank| probe.contains(bank))
    {
        return Vec::new();
    }
    if !probe.contains("工商银行") && !probe.contains("ICBC") && !row_text_like_header(&probe) {
        return Vec::new();
    }
    let Some((rows, _)) = csv_records_from_text(&text, row_text_like_header) else {
        return Vec::new();
    };
    post_process_raw_bills(
        "icbc",
        &rows.iter().filter_map(raw_icbc).collect::<Vec<_>>(),
    )
}

/// 判断文本是否符合工商银行交易明细表头，兼容多种金额列命名。
fn row_text_like_header(line: &str) -> bool {
    contains_all_text(line, &["交易日期", "交易金额", "对方户名", "对方账号"])
        || contains_all_text(line, &["记账日期", "金额", "对方户名", "对方账号"])
        || contains_all_text(line, &["交易日期", "收入/支出金额", "对方户名"])
}

/// 解析工商银行 Excel/HTML 表格导出，使用银行名称或交易附言列确认来源。
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
    if !spreadsheet.contains("中国工商银行")
        && !spreadsheet.contains("工商银行")
        && !spreadsheet.contains("ICBC")
        && !spreadsheet.contains("收入/支出金额")
        && !spreadsheet.contains("交易附言")
    {
        return Vec::new();
    }
    let maps = rows_to_maps(spreadsheet.rows(), |row| {
        row_text_like_header(&row.join(","))
    });
    post_process_raw_bills(
        "icbc",
        &maps.iter().filter_map(raw_icbc).collect::<Vec<_>>(),
    )
}

/// 将工商银行表格行映射为 RawBill，并用借贷标志或金额符号推断收支。
#[tracing::instrument(level = "debug", skip_all)]
fn raw_icbc(row: &RowMap) -> Option<RawBill> {
    let date = get(row, &["交易日期", "记账日期"]);
    if date.is_empty() {
        return None;
    }
    let amount_text = get(row, &["收入/支出金额", "交易金额", "金额"]);
    let amount = parse_amount(&amount_text)?;
    if amount == 0.0 {
        return None;
    }
    let explicit_type = get(row, &["收/支"]);
    let transaction_type = if !explicit_type.is_empty() {
        explicit_type
    } else if get(row, &["借贷标志"]) == "贷" || amount > 0.0 {
        "收入".to_string()
    } else {
        "支出".to_string()
    };
    Some(RawBill {
        trade_time: date.replace('\n', " "),
        transaction_type,
        counterparty: get(row, &["对方户名", "交易对方", "对方账号名称"]),
        opponent_account: get(row, &["对方账号"]),
        description: get(row, &["摘要", "用途"]),
        abstract_text: get(row, &["摘要", "交易摘要", "交易附言"]),
        amount: positive_amount_text(amount),
        transaction_id: get(row, &["交易流水号"]),
        channel: "工商银行".to_string(),
        ..Default::default()
    })
}
