use super::{
    common,
    registry::{parser_slice, AUTO_PARSERS},
    types::DedicatedParser,
};

/// 根据文件名和轻量内容探针缩小自动检测候选 parser，避免无谓扫描所有来源解析器。
pub(super) fn auto_parser_candidates(filename: &str, bytes: &[u8]) -> &'static [DedicatedParser] {
    let normalized = filename.trim().to_ascii_lowercase();
    let parser_id = if normalized.contains("微信") || normalized.contains("wechat") {
        Some("wechat")
    } else if normalized.contains("支付宝") || normalized.contains("alipay") {
        Some("alipay")
    } else if normalized.contains("工商银行") || normalized.contains("icbc") {
        Some("icbc")
    } else if normalized.contains("民生银行") || normalized.contains("cmbc") {
        Some("cmbc")
    } else if normalized.contains("农业银行") || normalized.contains("abc") {
        Some("abc")
    } else if normalized.contains("建设银行") || normalized.contains("ccb") {
        Some("ccb")
    } else if normalized.starts_with("detail") && normalized.ends_with(".xlsx") {
        Some("abc")
    } else if normalized.starts_with("交易明细_") && normalized.ends_with(".xls") {
        Some("ccb")
    } else {
        auto_parser_id_from_content(filename, bytes)
    };
    match parser_id {
        Some(parser_id) => parser_slice(parser_id).unwrap_or(AUTO_PARSERS),
        None => AUTO_PARSERS,
    }
}

/// 从文件内容前缀中提取来源 hint，只读取有限字节以保持自动检测的轻量性。
fn auto_parser_id_from_content(filename: &str, bytes: &[u8]) -> Option<&'static str> {
    let suffix = common::file_suffix(filename);
    if !matches!(suffix.as_str(), "csv" | "txt" | "xls")
        && !common::looks_like_html_table_payload(bytes)
    {
        return None;
    }
    let probe_len = bytes.len().min(128 * 1024);
    let probe = common::decode_text(&bytes[..probe_len]);
    if probe.contains("微信支付") || probe.contains("微信零钱") {
        Some("wechat")
    } else if probe.contains("支付宝") || probe.contains("Alipay") {
        Some("alipay")
    } else if probe.contains("中国民生银行") || probe.contains("民生银行") || probe.contains("CMBC")
    {
        Some("cmbc")
    } else if probe.contains("中国工商银行") || probe.contains("工商银行") || probe.contains("ICBC")
    {
        Some("icbc")
    } else if probe.contains("农业银行") || probe.contains("农业银⾏") {
        Some("abc")
    } else if probe.contains("建设银行") || probe.contains("China Construction Bank") {
        Some("ccb")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::auto_parser_candidates;

    fn candidate_ids(filename: &str, bytes: &[u8]) -> Vec<&'static str> {
        auto_parser_candidates(filename, bytes)
            .iter()
            .map(|parser| parser.id)
            .collect()
    }

    #[test]
    fn auto_parser_candidates_use_lightweight_content_hints() {
        assert_eq!(
            candidate_ids(
                "账号6226192003866332交易明细.xls",
                b"<html>\xd6\xd0\xb9\xfa\xc3\xf1\xc9\xfa\xd2\xf8\xd0\xd0</html>"
            ),
            ["cmbc"]
        );
        assert_eq!(
            candidate_ids("unknown.csv", "交易时间,交易金额\n微信支付,1.00".as_bytes()),
            ["wechat"]
        );
    }

    #[test]
    fn auto_parser_candidates_use_known_bank_export_filename_hints() {
        assert_eq!(candidate_ids("detail20250825.xlsx", b""), ["abc"]);
        assert_eq!(
            candidate_ids("交易明细_9316_20230827_20250827.xls", b""),
            ["ccb"]
        );
    }
}
