// 中文导读：正式账单查询合同，隔离 HTTP 查询参数、PostgreSQL 行与前端响应模型。
// 维护重点：这里仅表达账本读取语义，不携带 SQL、连接池、HTTP envelope 或 serde_json::Value。
// 不变式：金额始终使用 Money（整数分），用户范围由调用方显式传入 UserId。

use crate::{Money, TransactionType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerBalanceInput {
    pub transaction_type: TransactionType,
    pub amount: Money,
    pub destination_amount: Option<Money>,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerBalanceLeg {
    pub account_id: i64,
    pub delta: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerBalanceEffects {
    source: Option<LedgerBalanceLeg>,
    destination: Option<LedgerBalanceLeg>,
}

impl LedgerBalanceEffects {
    pub const fn source(&self) -> Option<LedgerBalanceLeg> {
        self.source
    }

    pub const fn destination(&self) -> Option<LedgerBalanceLeg> {
        self.destination
    }

    pub fn legs(&self) -> impl Iterator<Item = LedgerBalanceLeg> {
        [self.source, self.destination].into_iter().flatten()
    }
}

pub fn derive_ledger_balance_effects(
    input: LedgerBalanceInput,
) -> Result<LedgerBalanceEffects, crate::RuntimeError> {
    let amount = input.amount.checked_abs()?;
    let destination_amount = input.destination_amount.unwrap_or(amount).checked_abs()?;
    let source_account_id = input.source_account_id.filter(|value| *value > 0);
    let destination_account_id = input.destination_account_id.filter(|value| *value > 0);

    let (source_delta, destination_delta) = match input.transaction_type {
        TransactionType::Income => (amount, None),
        TransactionType::Expense => (amount.checked_negated()?, None),
        TransactionType::Transfer | TransactionType::Investment => {
            (amount.checked_negated()?, Some(destination_amount))
        }
    };

    Ok(LedgerBalanceEffects {
        source: source_account_id.map(|account_id| LedgerBalanceLeg {
            account_id,
            delta: source_delta,
        }),
        destination: destination_delta.and_then(|delta| {
            destination_account_id.map(|account_id| LedgerBalanceLeg { account_id, delta })
        }),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerListQuery {
    pub page: usize,
    pub page_size: usize,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub transaction_type: Option<String>,
    pub main_category: Option<String>,
    pub sub_category: Option<String>,
    pub batch_id: Option<String>,
    pub counterparty: Option<String>,
    pub description: Option<String>,
    pub keyword: Option<String>,
    pub account_ids: Vec<i64>,
    pub category_ids: Vec<i64>,
    pub tag_ids: Vec<i64>,
    pub amount_filter_cents: Option<String>,
}

impl Default for LedgerListQuery {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 20,
            date_from: None,
            date_to: None,
            transaction_type: None,
            main_category: None,
            sub_category: None,
            batch_id: None,
            counterparty: None,
            description: None,
            keyword: None,
            account_ids: Vec::new(),
            category_ids: Vec::new(),
            tag_ids: Vec::new(),
            amount_filter_cents: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerTag {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub id: i64,
    pub transaction_type: TransactionType,
    pub category_id: Option<i64>,
    pub main_category: String,
    pub sub_category: String,
    pub date: String,
    pub amount: Money,
    pub destination_amount: Money,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
    pub tags: Vec<LedgerTag>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntryPage {
    pub items: Vec<LedgerEntry>,
    pub total: i64,
    pub page: usize,
    pub page_size: usize,
}
