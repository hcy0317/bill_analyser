use std::borrow::Cow;

use crate::ParserInfo;

/// 返回 dedicated parser 的稳定注册顺序，供导入 route 和前端展示使用。
#[tracing::instrument(level = "debug", skip_all)]
pub fn parser_registry() -> &'static [ParserInfo] {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parser_registry",
        "business operation entered"
    );
    &[
        ParserInfo {
            id: "wechat",
            name: "微信支付",
            source_label: "微信",
            class_name: "WeChatParser",
            channel_tag: "wallet",
            supported_extensions: &[".csv", ".xlsx"],
        },
        ParserInfo {
            id: "alipay",
            name: "支付宝",
            source_label: "支付宝",
            class_name: "AlipayParser",
            channel_tag: "wallet",
            supported_extensions: &[".csv"],
        },
        ParserInfo {
            id: "icbc",
            name: "工商银行",
            source_label: "工商银行",
            class_name: "ICBCParser",
            channel_tag: "bank",
            supported_extensions: &[".csv", ".xlsx", ".xls"],
        },
        ParserInfo {
            id: "cmbc",
            name: "民生银行",
            source_label: "民生银行",
            class_name: "CMBCParser",
            channel_tag: "bank",
            supported_extensions: &[".csv", ".xlsx", ".xls"],
        },
        ParserInfo {
            id: "abc",
            name: "农业银行",
            source_label: "农业银行",
            class_name: "ABCParser",
            channel_tag: "bank",
            supported_extensions: &[".csv", ".xlsx", ".xls"],
        },
        ParserInfo {
            id: "ccb",
            name: "建设银行",
            source_label: "建设银行",
            class_name: "CCBParser",
            channel_tag: "bank",
            supported_extensions: &[".xlsx", ".xls"],
        },
    ]
}

/// 将 parser id 映射为中文来源标签，未知 id 保留归一化后的原值便于排查。
#[tracing::instrument(level = "debug", skip_all)]
pub fn parser_source_label(parser_id: &str) -> Cow<'static, str> {
    let normalized_parser_id = parser_id.trim().to_lowercase();
    match normalized_parser_id.as_str() {
        "" => Cow::Borrowed(""),
        "wechat" => Cow::Borrowed("微信"),
        "alipay" => Cow::Borrowed("支付宝"),
        "abc" => Cow::Borrowed("农业银行"),
        "ccb" => Cow::Borrowed("建设银行"),
        "cmbc" => Cow::Borrowed("民生银行"),
        "icbc" => Cow::Borrowed("工商银行"),
        "generic" => Cow::Borrowed("通用来源"),
        _ => Cow::Owned(normalized_parser_id),
    }
}
