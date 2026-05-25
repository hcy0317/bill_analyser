// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use crate::{post_process_raw_bills, RawBill, StandardBill};

use super::common::{
    contains_all_text, csv_records_from_text, decode_text, file_suffix, get, parse_amount,
    positive_amount_text, rows_to_maps, workbook_rows, RowMap,
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
        "csv" | "txt" => parse_csv(bytes),
        "xlsx" | "xls" => parse_sheet(bytes),
        _ => Vec::new(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_csv(bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "import_parser",
        operation = "parse_csv",
        "business operation entered"
    );
    let text = decode_text(bytes);
    let probe = text.lines().take(15).collect::<Vec<_>>().join("\n");
    if !probe.contains("农业银行")
        && !contains_all_text(&probe, &["交易日期", "交易时间", "收入金额", "支出金额"])
        && !contains_all_text(&probe, &["交易日期", "交易金额", "对手信息"])
    {
        return Vec::new();
    }
    let Some((rows, _)) = csv_records_from_text(&text, |line| {
        line.contains("交易日期") || line.contains("记账日期") || line.contains("交易⽇期")
    }) else {
        return Vec::new();
    };
    post_process_raw_bills("abc", &rows.iter().filter_map(raw_abc).collect::<Vec<_>>())
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_sheet(bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "import_parser",
        operation = "parse_sheet",
        "business operation entered"
    );
    let Some(rows) = workbook_rows(bytes) else {
        return Vec::new();
    };
    let content = rows.iter().flatten().cloned().collect::<Vec<_>>().join(" ");
    if !content.contains("农业银行")
        && !content.contains("农业银⾏")
        && !contains_all_text(&content, &["交易日期", "交易时间", "交易金额"])
        && !contains_all_text(&content, &["交易日期", "交易时间", "收入金额", "支出金额"])
    {
        return Vec::new();
    }
    let maps = rows_to_maps(&rows, |row| {
        let text = row.join(",");
        text.contains("交易日期") || text.contains("记账日期") || text.contains("交易⽇期")
    });
    post_process_raw_bills("abc", &maps.iter().filter_map(raw_abc).collect::<Vec<_>>())
}

#[tracing::instrument(level = "debug", skip_all)]
fn raw_abc(row: &RowMap) -> Option<RawBill> {
    let mut date = get(row, &["交易日期", "交易⽇期", "记账日期"]);
    let time = get(row, &["交易时间"]);
    if !date.is_empty() && !time.is_empty() {
        date = format!("{date} {time}");
    }
    if date.is_empty() {
        return None;
    }
    let income = get(row, &["收入金额", "转入金额", "收⼊⾦额"]);
    let expense = get(row, &["支出金额", "转出金额", "⽀出⾦额"]);
    let single = get(row, &["交易金额", "交易⾦额", "金额"]);
    let (amount, transaction_type) =
        if let Some(value) = parse_amount(&income).filter(|value| *value > 0.0) {
            (value, "收入")
        } else if let Some(value) = parse_amount(&expense).filter(|value| *value > 0.0) {
            (value, "支出")
        } else if let Some(value) = parse_amount(&single) {
            (value, if value > 0.0 { "收入" } else { "支出" })
        } else {
            return None;
        };
    if amount == 0.0 {
        return None;
    }
    Some(RawBill {
        trade_time: date,
        transaction_type: transaction_type.to_string(),
        counterparty: get(row, &["对方户名", "对方名称", "对⼿信息", "对方账号"]),
        description: get(row, &["交易用途", "用途", "摘要", "交易附⾔"]),
        summary: get(row, &["交易摘要"]),
        amount: positive_amount_text(amount),
        channel: "农业银行".to_string(),
        ..Default::default()
    })
}
