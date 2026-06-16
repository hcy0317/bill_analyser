// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use std::{collections::HashMap, io::Cursor, path::Path};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use encoding_rs::{GB18030, GBK};
use regex::Regex;

pub(super) type RowMap = HashMap<String, String>;

pub(super) fn file_suffix(filename: &str) -> String {
    Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

pub(super) fn decode_text(bytes: &[u8]) -> String {
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn csv_records_from_text(
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn workbook_rows(bytes: &[u8]) -> Option<Vec<Vec<String>>> {
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

pub(super) fn sheet_or_html_rows(bytes: &[u8]) -> Vec<Vec<String>> {
    if looks_like_html_table_payload(bytes) {
        html_rows(bytes)
    } else {
        workbook_rows(bytes).unwrap_or_else(|| html_rows(bytes))
    }
}

pub(super) fn html_payload_contains_any(bytes: &[u8], needles: &[&str]) -> Option<bool> {
    if !looks_like_html_table_payload(bytes) {
        return None;
    }
    let text = decode_text(bytes);
    Some(needles.iter().any(|needle| text.contains(needle)))
}

pub(super) fn looks_like_html_table_payload(bytes: &[u8]) -> bool {
    let mut probe = bytes;
    if probe.starts_with(&[0xef, 0xbb, 0xbf]) {
        probe = &probe[3..];
    }
    probe = probe
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|index| &probe[index..])
        .unwrap_or_default();
    starts_with_ignore_ascii_case(probe, b"<html")
        || starts_with_ignore_ascii_case(probe, b"<!doctype html")
        || starts_with_ignore_ascii_case(probe, b"<table")
}

fn starts_with_ignore_ascii_case(value: &[u8], prefix: &[u8]) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn html_rows(bytes: &[u8]) -> Vec<Vec<String>> {
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn rows_to_maps(
    rows: &[Vec<String>],
    header_matcher: impl Fn(&[String]) -> bool,
) -> Vec<RowMap> {
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

pub(super) fn clean_cell(value: &str) -> String {
    value
        .trim_matches(|ch: char| ch == '\u{feff}' || ch == '"' || ch == '\'')
        .trim()
        .replace("\\t", " ")
        .replace('\u{a0}', " ")
        .trim()
        .to_string()
}

pub(super) fn contains_all_text(text: &str, needles: &[&str]) -> bool {
    needles.iter().all(|needle| text.contains(needle))
}

pub(super) fn row_contains_all(row: &[String], needles: &[&str]) -> bool {
    let joined = row.join(" ");
    contains_all_text(&joined, needles)
}

pub(super) fn get(row: &RowMap, keys: &[&str]) -> String {
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

pub(super) fn parse_amount(value: &str) -> Option<f64> {
    let cleaned = value.replace(['¥', '$', ',', '，'], "").trim().to_string();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

pub(super) fn positive_amount_text(value: f64) -> String {
    let abs = value.abs();
    if abs.fract() == 0.0 {
        format!("{abs:.0}")
    } else {
        abs.to_string()
    }
}

pub(super) fn compact_date(date: &str) -> String {
    let text = date.trim();
    if text.len() >= 8 && text.chars().take(8).all(|ch| ch.is_ascii_digit()) {
        let part = &text[..8];
        format!("{}-{}-{}", &part[0..4], &part[4..6], &part[6..8])
    } else {
        text.to_string()
    }
}

pub(super) fn compact_time(time: &str) -> String {
    let text = time.trim();
    if text.len() >= 6 && text.chars().take(6).all(|ch| ch.is_ascii_digit()) {
        let part = &text[..6];
        format!("{}:{}:{}", &part[0..2], &part[2..4], &part[4..6])
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{looks_like_html_table_payload, sheet_or_html_rows};

    #[test]
    fn html_payload_detection_accepts_html_table_prefixes() {
        assert!(looks_like_html_table_payload(
            b"<html><body><table></table>"
        ));
        assert!(looks_like_html_table_payload(b" \r\n\t<TABLE><tr></tr>"));
        assert!(looks_like_html_table_payload(
            b"\xef\xbb\xbf<!DOCTYPE html><html></html>"
        ));
    }

    #[test]
    fn html_payload_detection_rejects_binary_or_plain_payloads() {
        assert!(!looks_like_html_table_payload(
            b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1"
        ));
        assert!(!looks_like_html_table_payload(b"transaction,date,amount\n"));
        assert!(!looks_like_html_table_payload(b""));
    }

    #[test]
    fn sheet_or_html_rows_uses_html_fast_path_for_html_xls_exports() {
        let rows = sheet_or_html_rows(
            b"<html><body><table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table></body></html>",
        );
        assert_eq!(rows, vec![vec!["A", "B"], vec!["1", "2"]]);
    }

    #[test]
    fn html_payload_contains_any_only_reports_for_html_payloads() {
        assert_eq!(
            super::html_payload_contains_any(
                "<html><body>民生银行</body></html>".as_bytes(),
                &["民生银行"]
            ),
            Some(true)
        );
        assert_eq!(
            super::html_payload_contains_any(
                "<html><body>建设银行</body></html>".as_bytes(),
                &["民生银行"]
            ),
            Some(false)
        );
        assert_eq!(
            super::html_payload_contains_any(b"transaction,date,amount\n", &["民生银行"]),
            None
        );
    }
}
