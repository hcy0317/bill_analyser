// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use std::{collections::HashMap, io::Cursor, path::Path};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use encoding_rs::{GB18030, GBK};
use regex::Regex;

/// dedicated parser 行数据的统一键值视图，key 是清洗后的来源表头。
pub(super) type RowMap = HashMap<String, String>;

/// 读取上传文件扩展名，用于来源 parser 选择 CSV、Excel 或 HTML 表格分支。
pub(super) fn file_suffix(filename: &str) -> String {
    Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// 按 UTF-8、GB18030、GBK 顺序解码来源文本，并去除 BOM。
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

/// 从文本中定位业务表头后解析 CSV/TSV/分号表格，返回清洗后的行映射和分隔符。
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

/// 根据表头行中常见分隔符出现次数推断 CSV reader 使用的分隔符。
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

/// 读取第一个 Excel 工作表并把所有单元格转换为 parser 可比较的文本。
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

/// 读取银行导出的 Excel 或 HTML 表格；Excel 失败时回退到 HTML 表格解析。
pub(super) fn sheet_or_html_rows(bytes: &[u8]) -> Vec<Vec<String>> {
    if looks_like_html_table_payload(bytes) {
        html_rows(bytes)
    } else {
        workbook_rows(bytes).unwrap_or_else(|| html_rows(bytes))
    }
}

/// 为通用预览读取 HTML 表格，并在构造结果时同步执行预算。
pub(super) fn html_spreadsheet_preview_rows(bytes: &[u8]) -> Option<(Vec<Vec<String>>, usize)> {
    let preview = crate::spreadsheet::parse_html_preview_rows(bytes).ok()?;
    Some((preview.rows, preview.total_rows))
}

/// 只在 payload 确认为 HTML 表格时执行关键词探测，避免把二进制 Excel 当文本扫描。
pub(super) fn html_payload_contains_any(bytes: &[u8], needles: &[&str]) -> Option<bool> {
    if !looks_like_html_table_payload(bytes) {
        return None;
    }
    let text = decode_text(bytes);
    Some(needles.iter().any(|needle| text.contains(needle)))
}

/// 用有限字节判断上传内容是否像 HTML 表格导出，兼容 BOM 和大小写前缀。
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

/// ASCII 前缀比较 helper，用于 HTML payload 的轻量格式探测。
fn starts_with_ignore_ascii_case(value: &[u8], prefix: &[u8]) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

/// 将 Excel 单元格转换为稳定文本，整数浮点值避免带上无意义小数。
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::Float(value) if value.fract() == 0.0 => format!("{value:.0}"),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        _ => clean_cell(&cell.to_string()),
    }
}

/// 从 HTML 表格中抽取行列文本，供银行 `.xls` 实为 HTML 的导出文件复用。
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

/// 清除简单 HTML 标签并反转义常见实体，保留来源单元格可读文本。
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

/// 从二维表格中定位业务表头并生成 RowMap，丢弃空表头列。
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

/// 清洗来源单元格文本，统一去除 BOM、引号、转义 tab 和不间断空格。
pub(super) fn clean_cell(value: &str) -> String {
    value
        .trim_matches(|ch: char| ch == '\u{feff}' || ch == '"' || ch == '\'')
        .trim()
        .replace("\\t", " ")
        .replace('\u{a0}', " ")
        .trim()
        .to_string()
}

/// 判断文本是否同时包含来源 parser 需要的所有关键词。
pub(super) fn contains_all_text(text: &str, needles: &[&str]) -> bool {
    needles.iter().all(|needle| text.contains(needle))
}

/// 判断表格行是否同时包含来源 parser 需要的所有关键词。
pub(super) fn row_contains_all(row: &[String], needles: &[&str]) -> bool {
    let joined = row.join(" ");
    contains_all_text(&joined, needles)
}

/// 按候选表头顺序读取首个非空、非占位来源字段。
pub(super) fn get(row: &RowMap, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            row.get(*key)
                .map(|value| clean_cell(value))
                .filter(|value| !is_empty_placeholder(value))
        })
        .unwrap_or_default()
}

/// 判断来源字段是否是空值占位，避免把 nan/null 写进 RawBill。
fn is_empty_placeholder(value: &str) -> bool {
    let text = value.trim();
    text.is_empty() || matches!(text, "nan" | "NaN" | "None" | "null")
}

/// 解析来源金额文本，兼容货币符号和中英文千分位。
pub(super) fn parse_amount(value: &str) -> Option<f64> {
    let cleaned = value.replace(['¥', '$', ',', '，'], "").trim().to_string();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

/// 把已判定方向的金额转换为正数文本，保留来源小数精度。
pub(super) fn positive_amount_text(value: f64) -> String {
    let abs = value.abs();
    if abs.fract() == 0.0 {
        format!("{abs:.0}")
    } else {
        abs.to_string()
    }
}

/// 将 `YYYYMMDD` 形式的银行日期压缩值转换为标准日期文本。
pub(super) fn compact_date(date: &str) -> String {
    let text = date.trim();
    if text.len() >= 8 && text.chars().take(8).all(|ch| ch.is_ascii_digit()) {
        let part = &text[..8];
        format!("{}-{}-{}", &part[0..4], &part[4..6], &part[6..8])
    } else {
        text.to_string()
    }
}

/// 将 `HHMMSS` 形式的银行时间压缩值转换为标准时间文本。
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
