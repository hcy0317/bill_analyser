use std::{collections::HashMap, io::Cursor, path::Path};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use encoding_rs::{GB18030, GBK};
use regex::Regex;

use crate::{post_process_raw_bills, RawBill, StandardBill};

#[derive(Debug, Clone, PartialEq)]
pub struct DedicatedParseResult {
    pub parser_id: String,
    pub bills: Vec<StandardBill>,
    pub delimiter: Option<char>,
}

type RowMap = HashMap<String, String>;

pub fn parse_dedicated_import_bytes(
    filename: &str,
    bytes: &[u8],
    requested_parser: &str,
) -> Option<DedicatedParseResult> {
    let requested = requested_parser.trim().to_ascii_lowercase();
    if matches!(
        requested.as_str(),
        "generic" | "csv" | "xlsx" | "xls" | "txt"
    ) {
        return None;
    }

    let parser_ids: Vec<&str> = if requested.is_empty() || requested == "auto" {
        vec!["wechat", "alipay", "icbc", "cmbc", "abc", "ccb"]
    } else {
        vec![requested.as_str()]
    };

    for parser_id in parser_ids {
        if let Some(result) = parse_for_parser(filename, bytes, parser_id) {
            if !result.bills.is_empty() {
                return Some(result);
            }
        }
    }
    None
}

fn parse_for_parser(filename: &str, bytes: &[u8], parser_id: &str) -> Option<DedicatedParseResult> {
    let suffix = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let bills = match (parser_id, suffix.as_str()) {
        ("wechat", "csv") | ("wechat", "txt") => parse_wechat_csv(filename, bytes),
        ("wechat", "xlsx") | ("wechat", "xls") => parse_wechat_sheet_or_html(bytes),
        ("alipay", "csv") | ("alipay", "txt") => parse_alipay_csv(bytes),
        ("icbc", "csv") | ("icbc", "txt") => parse_icbc_csv(bytes),
        ("icbc", "xlsx") | ("icbc", "xls") => parse_icbc_sheet_or_html(bytes),
        ("cmbc", "csv") | ("cmbc", "txt") => parse_cmbc_csv(bytes),
        ("cmbc", "xlsx") | ("cmbc", "xls") => parse_cmbc_sheet_or_html(bytes),
        ("abc", "csv") | ("abc", "txt") => parse_abc_csv(bytes),
        ("abc", "xlsx") | ("abc", "xls") => parse_abc_sheet(bytes),
        ("ccb", "xlsx") | ("ccb", "xls") => parse_ccb_sheet_or_html(bytes),
        _ => Vec::new(),
    };

    if bills.is_empty() {
        None
    } else {
        Some(DedicatedParseResult {
            parser_id: parser_id.to_string(),
            bills,
            delimiter: None,
        })
    }
}

fn decode_text(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.trim_start_matches('\u{feff}').to_string();
    }
    let (text, _, had_errors) = GB18030.decode(bytes);
    if !had_errors {
        return text.trim_start_matches('\u{feff}').to_string();
    }
    let (text, _, _) = GBK.decode(bytes);
    text.trim_start_matches('\u{feff}').to_string()
}

fn csv_records_from_text(
    text: &str,
    header_matcher: impl Fn(&str) -> bool,
) -> Option<(Vec<RowMap>, char)> {
    let lines = text.lines().collect::<Vec<_>>();
    let header_index = lines.iter().position(|line| header_matcher(line))?;
    let header_line = lines[header_index];
    let delimiter = detect_delimiter(header_line);
    let csv_text = lines[header_index..].join("\n");
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .delimiter(delimiter as u8)
        .from_reader(csv_text.as_bytes());
    let headers = reader.headers().ok()?.clone();
    let records = reader
        .records()
        .filter_map(Result::ok)
        .map(|record| {
            headers
                .iter()
                .enumerate()
                .map(|(index, header)| {
                    (
                        clean_cell(header),
                        clean_cell(record.get(index).unwrap_or_default()),
                    )
                })
                .collect::<RowMap>()
        })
        .collect::<Vec<_>>();
    Some((records, delimiter))
}

fn detect_delimiter(line: &str) -> char {
    [
        (',', line.matches(',').count()),
        ('\t', line.matches('\t').count()),
        (';', line.matches(';').count()),
    ]
    .into_iter()
    .max_by_key(|(_, count)| *count)
    .map(|(delimiter, _)| delimiter)
    .unwrap_or(',')
}

fn workbook_rows(bytes: &[u8]) -> Option<Vec<Vec<String>>> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = open_workbook_auto_from_rs(cursor).ok()?;
    let range = workbook.worksheet_range_at(0)?.ok()?;
    Some(
        range
            .rows()
            .map(|row| row.iter().map(cell_to_string).collect::<Vec<_>>())
            .collect(),
    )
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::Float(value) if value.fract() == 0.0 => format!("{value:.0}"),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        _ => clean_cell(&cell.to_string()),
    }
}

fn html_rows(bytes: &[u8]) -> Vec<Vec<String>> {
    let text = decode_text(bytes);
    let row_re = Regex::new(r"(?is)<tr[^>]*>(.*?)</tr>").expect("valid row regex");
    let cell_re = Regex::new(r"(?is)<t[dh][^>]*>(.*?)</t[dh]>").expect("valid cell regex");
    row_re
        .captures_iter(&text)
        .map(|row_capture| {
            let row_html = row_capture.get(1).map(|m| m.as_str()).unwrap_or_default();
            cell_re
                .captures_iter(row_html)
                .map(|cell_capture| {
                    clean_cell(&strip_html_tags(
                        cell_capture.get(1).map(|m| m.as_str()).unwrap_or_default(),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .filter(|row| row.iter().any(|cell| !cell.is_empty()))
        .collect()
}

fn strip_html_tags(value: &str) -> String {
    let tag_re = Regex::new(r"(?is)<[^>]+>").expect("valid tag regex");
    tag_re
        .replace_all(value, "")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#13;", " ")
}

fn rows_to_maps(rows: &[Vec<String>], header_matcher: impl Fn(&[String]) -> bool) -> Vec<RowMap> {
    let Some(header_index) = rows.iter().position(|row| header_matcher(row)) else {
        return Vec::new();
    };
    let headers = rows[header_index]
        .iter()
        .map(|value| clean_cell(value))
        .collect::<Vec<_>>();
    rows.iter()
        .skip(header_index + 1)
        .map(|row| {
            headers
                .iter()
                .enumerate()
                .filter(|(_, header)| !header.is_empty())
                .map(|(index, header)| {
                    (
                        header.clone(),
                        row.get(index)
                            .map(|value| clean_cell(value))
                            .unwrap_or_default(),
                    )
                })
                .collect::<RowMap>()
        })
        .collect()
}

fn clean_cell(value: &str) -> String {
    value
        .trim_matches(|ch: char| ch == '\u{feff}' || ch == '"' || ch == '\'')
        .trim()
        .replace("\\t", " ")
        .replace('\u{a0}', " ")
        .trim()
        .to_string()
}

fn contains_all_text(text: &str, needles: &[&str]) -> bool {
    needles.iter().all(|needle| text.contains(needle))
}

fn row_contains_all(row: &[String], needles: &[&str]) -> bool {
    let joined = row.join(" ");
    contains_all_text(&joined, needles)
}

fn get(row: &RowMap, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            row.get(*key)
                .map(|value| clean_cell(value))
                .filter(|value| !is_empty_placeholder(value))
        })
        .unwrap_or_default()
}

fn is_empty_placeholder(value: &str) -> bool {
    let text = value.trim();
    text.is_empty() || matches!(text, "nan" | "NaN" | "None" | "null")
}

fn parse_amount(value: &str) -> Option<f64> {
    let cleaned = value.replace(['¥', '$', ',', '，'], "").trim().to_string();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

fn positive_amount_text(value: f64) -> String {
    let abs = value.abs();
    if abs.fract() == 0.0 {
        format!("{abs:.0}")
    } else {
        abs.to_string()
    }
}

fn compact_date(date: &str) -> String {
    let text = date.trim();
    if text.len() >= 8 && text.chars().take(8).all(|ch| ch.is_ascii_digit()) {
        let part = &text[..8];
        format!("{}-{}-{}", &part[0..4], &part[4..6], &part[6..8])
    } else {
        text.to_string()
    }
}

fn compact_time(time: &str) -> String {
    let text = time.trim();
    if text.len() >= 6 && text.chars().take(6).all(|ch| ch.is_ascii_digit()) {
        let part = &text[..6];
        format!("{}:{}:{}", &part[0..2], &part[2..4], &part[4..6])
    } else {
        text.to_string()
    }
}

fn build_ccb_trade_time(row: &RowMap) -> String {
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

fn build_cmbc_trade_time(value: &str) -> String {
    let text = value.replace('\t', " ");
    let compact = text.trim();
    if compact.len() >= 8 && compact.chars().take(8).all(|ch| ch.is_ascii_digit()) {
        let date = compact_date(&compact[..8]);
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

fn parse_wechat_csv(filename: &str, bytes: &[u8]) -> Vec<StandardBill> {
    let text = decode_text(bytes);
    let filename_hint =
        filename.to_ascii_lowercase().contains("wechat") || filename.contains("微信");
    let has_wechat_marker = text
        .lines()
        .take(8)
        .any(|line| line.contains("微信支付账单"));
    let has_wechat_export_header = text
        .lines()
        .any(|line| row_text_like_wechat_header(line) && line.contains("金额(元)"));
    if !has_wechat_marker && !has_wechat_export_header && !filename_hint {
        return Vec::new();
    }
    let Some((rows, _)) = csv_records_from_text(&text, row_text_like_wechat_header) else {
        return Vec::new();
    };
    post_process_raw_bills(
        "wechat",
        &rows.iter().filter_map(raw_wechat).collect::<Vec<_>>(),
    )
}

fn row_text_like_wechat_header(line: &str) -> bool {
    line.contains("交易时间")
        && (line.contains("金额(元)") || line.contains("金额"))
        && (line.contains("收/支") || line.contains("收支类型") || line.contains("交易类型"))
        && (line.contains("商品") || line.contains("交易对方") || line.contains("支付方式"))
}

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

fn parse_wechat_sheet_or_html(bytes: &[u8]) -> Vec<StandardBill> {
    let rows = workbook_rows(bytes).unwrap_or_else(|| html_rows(bytes));
    if !rows
        .iter()
        .take(5)
        .any(|row| row.join(" ").contains("微信支付账单"))
    {
        return Vec::new();
    }
    let maps = rows_to_maps(&rows, |row| row_text_like_wechat_header(&row.join(",")));
    post_process_raw_bills(
        "wechat",
        &maps.iter().filter_map(raw_wechat).collect::<Vec<_>>(),
    )
}

fn parse_alipay_csv(bytes: &[u8]) -> Vec<StandardBill> {
    let text = decode_text(bytes);
    let probe = text.lines().take(15).collect::<Vec<_>>().join("\n");
    if ![
        "支付宝",
        "alipay",
        "支付宝账户",
        "支付宝（中国）网络技术有限公司",
    ]
    .iter()
    .any(|indicator| probe.to_lowercase().contains(&indicator.to_lowercase()))
    {
        return Vec::new();
    }
    let Some((rows, _)) = csv_records_from_text(&text, |line| {
        line.contains("交易时间") && line.contains("交易")
    }) else {
        return Vec::new();
    };
    post_process_raw_bills(
        "alipay",
        &rows.iter().filter_map(raw_alipay).collect::<Vec<_>>(),
    )
}

fn raw_alipay(row: &RowMap) -> Option<RawBill> {
    let date = get(row, &["交易时间", "交易创建时间"]);
    if date.is_empty() || date.contains('共') {
        return None;
    }
    Some(RawBill {
        trade_time: date,
        transaction_type: get(row, &["收/支"]),
        counterparty: get(row, &["交易对方", "对方"]),
        opponent_account: get(row, &["对方账号"]),
        description: get(row, &["商品说明", "商品名称"]),
        goods: get(row, &["商品说明", "商品名称"]),
        amount: get(row, &["金额", "金额(元)"]),
        payment_method: get(row, &["收/付款方式"]),
        status: get(row, &["交易状态"]),
        transaction_id: get(row, &["交易订单号"]),
        merchant_id: get(row, &["商家订单号"]),
        remark: get(row, &["备注"]),
        original_category: get(row, &["交易分类"]),
        ..Default::default()
    })
}

fn parse_icbc_csv(bytes: &[u8]) -> Vec<StandardBill> {
    let text = decode_text(bytes);
    let probe = text.lines().take(10).collect::<Vec<_>>().join("\n");
    if ["民生银行", "农业银行", "建设银行"]
        .iter()
        .any(|bank| probe.contains(bank))
    {
        return Vec::new();
    }
    if !probe.contains("工商银行") && !probe.contains("ICBC") && !row_text_like_icbc_header(&probe)
    {
        return Vec::new();
    }
    let Some((rows, _)) = csv_records_from_text(&text, row_text_like_icbc_header) else {
        return Vec::new();
    };
    post_process_raw_bills(
        "icbc",
        &rows.iter().filter_map(raw_icbc).collect::<Vec<_>>(),
    )
}

fn row_text_like_icbc_header(line: &str) -> bool {
    contains_all_text(line, &["交易日期", "交易金额", "对方户名", "对方账号"])
        || contains_all_text(line, &["记账日期", "金额", "对方户名", "对方账号"])
        || contains_all_text(line, &["交易日期", "收入/支出金额", "对方户名"])
}

fn parse_icbc_sheet_or_html(bytes: &[u8]) -> Vec<StandardBill> {
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
    let maps = rows_to_maps(&rows, |row| row_text_like_icbc_header(&row.join(",")));
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

fn parse_cmbc_csv(bytes: &[u8]) -> Vec<StandardBill> {
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

fn parse_cmbc_sheet_or_html(bytes: &[u8]) -> Vec<StandardBill> {
    let rows = workbook_rows(bytes).unwrap_or_else(|| html_rows(bytes));
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
        trade_time: build_cmbc_trade_time(&raw_date),
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

fn parse_abc_csv(bytes: &[u8]) -> Vec<StandardBill> {
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

fn parse_abc_sheet(bytes: &[u8]) -> Vec<StandardBill> {
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

fn parse_ccb_sheet_or_html(bytes: &[u8]) -> Vec<StandardBill> {
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

fn raw_ccb(row: &RowMap) -> Option<RawBill> {
    let trade_time = build_ccb_trade_time(row);
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
