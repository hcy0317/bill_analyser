use crate::{post_process_raw_bills, RawBill, StandardBill};

use super::common::{
    contains_all_text, csv_records_from_text, decode_text, file_suffix, get, html_rows,
    parse_amount, positive_amount_text, rows_to_maps, workbook_rows, RowMap,
};

pub(super) fn parse(filename: &str, bytes: &[u8]) -> Vec<StandardBill> {
    let suffix = file_suffix(filename);
    match suffix.as_str() {
        "csv" | "txt" => parse_csv(bytes),
        "xlsx" | "xls" => parse_sheet_or_html(bytes),
        _ => Vec::new(),
    }
}

fn parse_csv(bytes: &[u8]) -> Vec<StandardBill> {
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

fn row_text_like_header(line: &str) -> bool {
    contains_all_text(line, &["交易日期", "交易金额", "对方户名", "对方账号"])
        || contains_all_text(line, &["记账日期", "金额", "对方户名", "对方账号"])
        || contains_all_text(line, &["交易日期", "收入/支出金额", "对方户名"])
}

fn parse_sheet_or_html(bytes: &[u8]) -> Vec<StandardBill> {
    let rows = workbook_rows(bytes).unwrap_or_else(|| html_rows(bytes));
    let content = rows.iter().flatten().cloned().collect::<Vec<_>>().join(" ");
    if !content.contains("中国工商银行")
        && !content.contains("工商银行")
        && !content.contains("ICBC")
        && !content.contains("收入/支出金额")
        && !content.contains("交易附言")
    {
        return Vec::new();
    }
    let maps = rows_to_maps(&rows, |row| row_text_like_header(&row.join(",")));
    post_process_raw_bills(
        "icbc",
        &maps.iter().filter_map(raw_icbc).collect::<Vec<_>>(),
    )
}

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
