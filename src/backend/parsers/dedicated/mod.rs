use crate::StandardBill;

mod common;

#[path = "ABC.rs"]
mod abc;
#[path = "Alipay.rs"]
mod alipay;
#[path = "CCB.rs"]
mod ccb;
#[path = "CMBC.rs"]
mod cmbc;
#[path = "ICBC.rs"]
mod icbc;
#[path = "WeChat.rs"]
mod wechat;

#[derive(Debug, Clone, PartialEq)]
pub struct DedicatedParseResult {
    pub parser_id: String,
    pub bills: Vec<StandardBill>,
    pub delimiter: Option<char>,
}

struct DedicatedParser {
    id: &'static str,
    parse: fn(&str, &[u8]) -> Vec<StandardBill>,
}

const AUTO_PARSERS: &[DedicatedParser] = &[
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

    if requested.is_empty() || requested == "auto" {
        return AUTO_PARSERS
            .iter()
            .find_map(|parser| parse_with_parser(parser, filename, bytes));
    }

    AUTO_PARSERS
        .iter()
        .find(|parser| parser.id == requested)
        .and_then(|parser| parse_with_parser(parser, filename, bytes))
}

fn parse_with_parser(
    parser: &DedicatedParser,
    filename: &str,
    bytes: &[u8],
) -> Option<DedicatedParseResult> {
    let bills = (parser.parse)(filename, bytes);
    if bills.is_empty() {
        None
    } else {
        Some(DedicatedParseResult {
            parser_id: parser.id.to_string(),
            bills,
            delimiter: None,
        })
    }
}
