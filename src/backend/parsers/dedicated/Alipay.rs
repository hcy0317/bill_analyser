// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use crate::{post_process_raw_bills, RawBill, StandardBill};

use super::common::{csv_records_from_text, decode_text, file_suffix, get, RowMap};
use super::types::DedicatedParserInput;

/// 解析支付宝导出文件，只接受 CSV/TXT 来源并返回标准化后的账单。
#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn parse(input: &DedicatedParserInput<'_>) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parse",
        "business operation entered"
    );
    let suffix = file_suffix(input.filename());
    match suffix.as_str() {
        "csv" | "txt" => parse_csv(input.bytes()),
        _ => Vec::new(),
    }
}

/// 解析支付宝 CSV/TXT 内容，先用文件前缀探测支付宝标记再读取业务表头。
#[tracing::instrument(level = "debug", skip_all)]
fn parse_csv(bytes: &[u8]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parse_csv",
        "business operation entered"
    );
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

/// 将支付宝行映射为 RawBill，保留对方、订单号、状态和原始分类等来源字段。
#[tracing::instrument(level = "debug", skip_all)]
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
