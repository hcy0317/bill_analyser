use super::{abc, alipay, ccb, cmbc, icbc, types::DedicatedParser, wechat};

pub(super) const AUTO_PARSERS: &[DedicatedParser] = &[
    DedicatedParser {
        id: "wechat",
        parse: wechat::parse,
    },
    DedicatedParser {
        id: "alipay",
        parse: alipay::parse,
    },
    DedicatedParser {
        id: "icbc",
        parse: icbc::parse,
    },
    DedicatedParser {
        id: "cmbc",
        parse: cmbc::parse,
    },
    DedicatedParser {
        id: "abc",
        parse: abc::parse,
    },
    DedicatedParser {
        id: "ccb",
        parse: ccb::parse,
    },
];

/// 返回指定 parser 的单元素切片，用于 auto hint 命中后保持原 dispatcher 分支形态。
pub(super) fn parser_slice(parser_id: &str) -> Option<&'static [DedicatedParser]> {
    AUTO_PARSERS
        .iter()
        .find(|parser| parser.id == parser_id)
        .map(std::slice::from_ref)
}
