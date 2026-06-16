// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use crate::{post_process_raw_bills, RawBill, StandardBill};

use super::common::{
    contains_all_text, csv_records_from_text, decode_text, file_suffix, get,
    html_payload_contains_any, parse_amount, positive_amount_text, rows_to_maps,
    sheet_or_html_rows, RowMap,
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
        "xlsx" | "xls" => parse_sheet_or_html(bytes),
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
    if !probe.contains("民生银行")
        && !probe.contains("CMBC")
        && !contains_all_text(&probe, &["交易时间", "交易金额", "交易对手"])
    {
        return Vec::new();
    }
    let Some((rows, _)) = csv_records_from_text(&text, |line| {
        line.contains("交易日期") || line.contains("记账日期") || line.contains("交易时间")
    }) else {
        return Vec::new();
    };
    post_process_raw_bills(
        "cmbc",
        &rows.iter().filter_map(raw_cmbc).collect::<Vec<_>>(),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_sheet_or_html(bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "import_parser",
        operation = "parse_sheet_or_html",
        "business operation entered"
    );
    if html_payload_contains_any(
        bytes,
        &[
            "民生银行",
            "个人账户对账单",
            "账户余额",
            "支出金额",
            "存入金额",
        ],
    ) == Some(false)
    {
        return Vec::new();
    }
    let rows = sheet_or_html_rows(bytes);
    let content = rows.iter().flatten().cloned().collect::<Vec<_>>().join(" ");
    if !content.contains("民生银行")
        && !content.contains("个人账户对账单")
        && !contains_all_text(&content, &["交易时间", "支出金额", "存入金额", "账户余额"])
    {
        return Vec::new();
    }
    let maps = rows_to_maps(&rows, |row| {
        let text = row.join(",");
        text.contains("交易日期") || text.contains("记账日期") || text.contains("交易时间")
    });
    post_process_raw_bills(
        "cmbc",
        &maps.iter().filter_map(raw_cmbc).collect::<Vec<_>>(),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn raw_cmbc(row: &RowMap) -> Option<RawBill> {
    let raw_date = get(row, &["交易日期", "记账日期", "交易时间"]);
    if raw_date.is_empty() {
        return None;
    }
    let mut transaction_type = get(row, &["收/支"]);
    let mut amount_text = get(row, &["交易金额", "金额"]);
    if amount_text.is_empty() {
        let credit = get(row, &["存入金额"]);
        let debit = get(row, &["支出金额"]);
        if !credit.is_empty() {
            transaction_type = "收入".to_string();
            amount_text = credit;
        } else if !debit.is_empty() {
            transaction_type = "支出".to_string();
            amount_text = debit;
        }
    }
    let amount = parse_amount(&amount_text)?;
    if amount == 0.0 {
        return None;
    }
    if transaction_type.is_empty() {
        transaction_type = if amount > 0.0 { "收入" } else { "支出" }.to_string();
    }
    Some(RawBill {
        trade_time: build_trade_time(&raw_date),
        transaction_type,
        counterparty: get(row, &["交易对手", "对方户名", "对方名称"]),
        description: get(row, &["交易说明", "摘要", "交易方式"]),
        summary: get(row, &["摘要"]),
        amount: positive_amount_text(amount),
        channel: "民生银行".to_string(),
        opponent_account: get(row, &["对方账号"]),
        payment_method: get(row, &["交易方式"]),
        ..Default::default()
    })
}

fn build_trade_time(value: &str) -> String {
    let text = value.replace('\t', " ");
    let compact = text.trim();
    if compact.len() >= 8 && compact.chars().take(8).all(|ch| ch.is_ascii_digit()) {
        let date = super::common::compact_date(&compact[..8]);
        let time = compact[8..].trim();
        if time.is_empty() {
            date
        } else {
            format!("{date} {time}")
        }
    } else {
        compact.to_string()
    }
}
