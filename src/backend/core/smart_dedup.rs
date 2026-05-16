use std::collections::{HashMap, HashSet};

use chrono::NaiveDateTime;
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::primitives::{parse_bill_datetime, Money};

const PLATFORM_SOURCES: &[&str] = &["wechat", "alipay"];
const BANK_SOURCES: &[&str] = &["icbc", "cmbc", "abc", "ccb"];
const TRANSFER_INTENT_KEYWORDS: &[&str] = &[
    "转账",
    "转入",
    "转出",
    "提现",
    "充值",
    "还款",
    "划转",
    "内部转",
    "存入",
    "取出",
];
const TIME_TOLERANCE_SECONDS: i64 = 30;
const DATABASE_TIME_TOLERANCE_SECONDS: i64 = 300;
const AMOUNT_TOLERANCE_CENTS: i128 = 1;
const SIMILARITY_THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationType {
    Exact,
    Transfer,
    PlatformBank,
    Similar,
    SplitMerge,
    DatabaseDuplicate,
}

impl DeduplicationType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Transfer => "transfer",
            Self::PlatformBank => "platform_bank",
            Self::Similar => "similar",
            Self::SplitMerge => "split_merge",
            Self::DatabaseDuplicate => "database_duplicate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationCandidateType {
    Duplicate,
    Transfer,
}

impl ReconciliationCandidateType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate",
            Self::Transfer => "transfer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MergedBillSource {
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub source: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub date: String,
    #[serde(default, deserialize_with = "deserialize_optional_stringish")]
    pub amount: Option<String>,
    #[serde(
        default,
        rename = "_template_id",
        alias = "template_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub template_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TransferSourceSnapshot {
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub role: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub parser_id: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub payment_method: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub counterparty: String,
    #[serde(default, deserialize_with = "deserialize_optional_stringish")]
    pub source_account_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub account_name: String,
    #[serde(default, deserialize_with = "deserialize_string_vec")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DedupBill {
    #[serde(default, deserialize_with = "deserialize_optional_stringish")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub date: String,
    #[serde(
        default = "default_money",
        deserialize_with = "deserialize_yuan_money",
        serialize_with = "serialize_yuan_money"
    )]
    pub amount: Money,
    #[serde(default, rename = "type", deserialize_with = "deserialize_stringish")]
    pub transaction_type: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub source_account_id: String,
    #[serde(
        default,
        rename = "_parser_id",
        alias = "parser_id",
        deserialize_with = "deserialize_stringish"
    )]
    pub parser_id: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub source: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub counterparty: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub payment_method: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub description: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub original_category: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub main_category: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub sub_category: String,
    #[serde(
        default,
        rename = "_template_id",
        alias = "template_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub template_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub account_name: String,
    #[serde(
        default,
        rename = "_parser_tags",
        alias = "parser_tags",
        deserialize_with = "deserialize_string_vec"
    )]
    pub parser_tags: Vec<String>,
    #[serde(
        default,
        rename = "_dedup_type",
        alias = "dedup_type",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub dedup_type: Option<String>,
    #[serde(
        default,
        rename = "_removed",
        alias = "removed",
        deserialize_with = "deserialize_boolish"
    )]
    pub removed: bool,
    #[serde(
        default,
        rename = "_merged_template_ids",
        alias = "merged_template_ids",
        deserialize_with = "deserialize_string_vec"
    )]
    pub merged_template_ids: Vec<String>,
    #[serde(default, rename = "_merged_from")]
    pub merged_from: Vec<MergedBillSource>,
    #[serde(
        default,
        rename = "_merged_into",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub merged_into: Option<String>,
    #[serde(
        default,
        rename = "_duplicate_of_db_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub duplicate_of_db_id: Option<String>,
    #[serde(
        default,
        rename = "_cross_batch_db_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub cross_batch_db_id: Option<String>,
    #[serde(
        default,
        rename = "_transfer_pair_order",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub transfer_pair_order: Option<String>,
    #[serde(default, rename = "_transfer_pair_sources")]
    pub transfer_pair_sources: Vec<TransferSourceSnapshot>,
    #[serde(
        default,
        rename = "_destination_parser_id",
        deserialize_with = "deserialize_stringish"
    )]
    pub destination_parser_id: String,
    #[serde(
        default,
        rename = "_destination_payment_method",
        deserialize_with = "deserialize_stringish"
    )]
    pub destination_payment_method: String,
    #[serde(
        default,
        rename = "_destination_counterparty",
        deserialize_with = "deserialize_stringish"
    )]
    pub destination_counterparty: String,
    #[serde(
        default,
        rename = "_destination_account_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub destination_account_id: Option<String>,
    #[serde(
        default,
        rename = "_destination_account_name",
        deserialize_with = "deserialize_stringish"
    )]
    pub destination_account_name: String,
    #[serde(
        default,
        rename = "_session_id",
        alias = "session_id",
        alias = "import_session_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub session_id: Option<String>,
    #[serde(
        default,
        rename = "_preview_id",
        alias = "preview_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub preview_id: Option<String>,
    #[serde(
        default,
        rename = "_skip_reconciliation_candidate",
        deserialize_with = "deserialize_boolish"
    )]
    pub skip_reconciliation_candidate: bool,
}

impl Default for DedupBill {
    fn default() -> Self {
        Self {
            id: None,
            date: String::new(),
            amount: Money::ZERO,
            transaction_type: String::new(),
            source_account_id: String::new(),
            parser_id: String::new(),
            source: String::new(),
            counterparty: String::new(),
            payment_method: String::new(),
            description: String::new(),
            original_category: String::new(),
            main_category: String::new(),
            sub_category: String::new(),
            template_id: None,
            account_name: String::new(),
            parser_tags: Vec::new(),
            dedup_type: None,
            removed: false,
            merged_template_ids: Vec::new(),
            merged_from: Vec::new(),
            merged_into: None,
            duplicate_of_db_id: None,
            cross_batch_db_id: None,
            transfer_pair_order: None,
            transfer_pair_sources: Vec::new(),
            destination_parser_id: String::new(),
            destination_payment_method: String::new(),
            destination_counterparty: String::new(),
            destination_account_id: None,
            destination_account_name: String::new(),
            session_id: None,
            preview_id: None,
            skip_reconciliation_candidate: false,
        }
    }
}

impl DedupBill {
    pub fn source_type(&self) -> String {
        let parser_id = normalized_source(&self.parser_id);
        if !parser_id.is_empty() {
            return parser_id;
        }

        let source = normalized_source(&self.source);
        if !source.is_empty() {
            return source;
        }

        let source_account_id = normalized_source(&self.source_account_id);
        if is_platform_source(&source_account_id) || is_bank_source(&source_account_id) {
            source_account_id
        } else {
            String::new()
        }
    }

    pub fn source_identifier(&self) -> String {
        let parser_id = normalized_source(&self.parser_id);
        if parser_id.is_empty() {
            normalized_source(&self.source_account_id)
        } else {
            parser_id
        }
    }

    pub fn dedup_source_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        append_optional_id(&mut ids, &self.template_id);
        append_ids(&mut ids, &self.merged_template_ids);
        ids
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub dedup_type: DeduplicationType,
    pub bill_indices: Vec<usize>,
    pub keep_index: usize,
    pub remove_indices: Vec<usize>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferPair {
    pub outgoing_index: usize,
    pub incoming_index: usize,
    pub cross_batch_db_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitGroup {
    pub total_index: usize,
    pub split_indices: Vec<usize>,
    #[serde(
        default = "default_money",
        deserialize_with = "deserialize_yuan_money",
        serialize_with = "serialize_yuan_money"
    )]
    pub total_amount: Money,
    pub source_total: String,
    pub source_splits: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeduplicationResult {
    pub original_count: usize,
    pub kept_bills: Vec<DedupBill>,
    pub removed_count: usize,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub transfer_pairs: Vec<TransferPair>,
    pub split_groups: Vec<SplitGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseDuplicateMatch {
    pub imported_index: usize,
    pub existing_index: usize,
    pub existing_bill_id: Option<String>,
    pub is_platform_bank_pair: bool,
    pub dedup_type: DeduplicationType,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossBatchTransferMatch {
    pub imported_index: usize,
    pub existing_index: usize,
    pub existing_bill_id: Option<String>,
    pub dedup_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReconciliationCandidate {
    pub candidate_id: String,
    pub candidate_type: ReconciliationCandidateType,
    pub import_bill_key: String,
    pub existing_bill_id: Option<String>,
    pub group_key: String,
    #[serde(
        default = "default_money",
        deserialize_with = "deserialize_yuan_money",
        serialize_with = "serialize_yuan_money"
    )]
    pub amount_abs: Money,
    pub time_diff_seconds: i64,
    pub score_percent: u8,
    pub level: String,
    pub reason: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SmartDeduplicationEngine;

impl SmartDeduplicationEngine {
    pub fn process(&self, bills: Vec<DedupBill>) -> DeduplicationResult {
        let original_count = bills.len();
        let mut bills = bills;
        let mut duplicate_groups = Vec::new();
        let mut transfer_pairs = Vec::new();
        let mut split_groups = Vec::new();

        duplicate_groups.extend(find_exact_duplicates(&mut bills));
        duplicate_groups.extend(find_platform_bank_duplicates(&mut bills));
        transfer_pairs.extend(find_transfer_pairs(&mut bills));
        duplicate_groups.extend(find_similar_duplicates(&mut bills));
        split_groups.extend(find_split_bills(&mut bills));

        let kept_bills: Vec<DedupBill> = bills
            .into_iter()
            .filter(|bill| !bill.removed)
            .map(clean_runtime_markers)
            .collect();
        let removed_count = original_count.saturating_sub(kept_bills.len());

        DeduplicationResult {
            original_count,
            kept_bills,
            removed_count,
            duplicate_groups,
            transfer_pairs,
            split_groups,
        }
    }
}

pub fn find_database_duplicates(
    imported_bills: &mut [DedupBill],
    existing_bills: &[DedupBill],
) -> Vec<DatabaseDuplicateMatch> {
    let mut matches = Vec::new();

    for (imported_index, imported_bill) in imported_bills.iter_mut().enumerate() {
        if imported_bill.removed {
            continue;
        }
        let Some(imported_dt) = bill_datetime(imported_bill) else {
            continue;
        };
        for (existing_index, existing_bill) in existing_bills.iter().enumerate() {
            let Some(existing_dt) = bill_datetime(existing_bill) else {
                continue;
            };
            if !same_day(imported_dt, existing_dt)
                || !time_close(imported_dt, existing_dt, DATABASE_TIME_TOLERANCE_SECONDS)
            {
                continue;
            }

            let is_platform_bank_pair = is_platform_source(&imported_bill.source_type())
                != is_platform_source(&existing_bill.source_type());
            let amount_match = if is_platform_bank_pair {
                abs_amount_close(imported_bill.amount, existing_bill.amount)
            } else {
                amount_equal_same_direction(imported_bill.amount, existing_bill.amount)
            };
            if !amount_match {
                continue;
            }

            if duplicate_text_match(imported_bill, existing_bill, is_platform_bank_pair) {
                let reason_type = if is_platform_bank_pair {
                    "平台-银行跨文件重复"
                } else {
                    "与数据库已有账单重复"
                };
                imported_bill.removed = true;
                imported_bill.dedup_type =
                    Some(DeduplicationType::DatabaseDuplicate.as_str().to_string());
                imported_bill.duplicate_of_db_id = existing_bill.id.clone();
                matches.push(DatabaseDuplicateMatch {
                    imported_index,
                    existing_index,
                    existing_bill_id: existing_bill.id.clone(),
                    is_platform_bank_pair,
                    dedup_type: DeduplicationType::DatabaseDuplicate,
                    reason: format!(
                        "{reason_type} (ID={}, 日期={}, 金额={})",
                        existing_bill.id.as_deref().unwrap_or_default(),
                        existing_bill.date,
                        existing_bill.amount.to_yuan_string()
                    ),
                });
                break;
            }
        }
    }

    matches
}

pub fn find_cross_batch_transfer_pairs(
    imported_bills: &mut [DedupBill],
    existing_bills: &[DedupBill],
) -> Vec<CrossBatchTransferMatch> {
    let mut matches = Vec::new();
    let mut matched_existing_ids = HashSet::new();

    for (imported_index, imported_bill) in imported_bills.iter_mut().enumerate() {
        if imported_bill.removed || imported_bill.dedup_type.as_deref() == Some("transfer") {
            continue;
        }
        let Some(imported_dt) = bill_datetime(imported_bill) else {
            continue;
        };
        for (existing_index, existing_bill) in existing_bills.iter().enumerate() {
            let existing_match_key = existing_bill
                .id
                .clone()
                .unwrap_or_else(|| existing_index.to_string());
            if matched_existing_ids.contains(&existing_match_key) {
                continue;
            }

            let Some(existing_dt) = bill_datetime(existing_bill) else {
                continue;
            };
            if !same_day(imported_dt, existing_dt)
                || !time_close(imported_dt, existing_dt, DATABASE_TIME_TOLERANCE_SECONDS)
            {
                continue;
            }
            if !amount_opposite(imported_bill.amount, existing_bill.amount) {
                continue;
            }

            let imported_source = imported_bill.source_type();
            let existing_source = existing_bill.source_type();
            if !imported_source.is_empty()
                && !existing_source.is_empty()
                && imported_source == existing_source
            {
                continue;
            }

            matched_existing_ids.insert(existing_match_key);
            imported_bill.transaction_type = "转账".to_string();
            imported_bill.dedup_type = Some("transfer_cross_batch".to_string());
            imported_bill.cross_batch_db_id = existing_bill.id.clone();
            matches.push(CrossBatchTransferMatch {
                imported_index,
                existing_index,
                existing_bill_id: existing_bill.id.clone(),
                dedup_type: "transfer_cross_batch".to_string(),
            });
            break;
        }
    }

    matches
}

pub fn find_import_reconciliation_candidates(
    imported_bills: &[DedupBill],
    existing_bills: &[DedupBill],
) -> Vec<ImportReconciliationCandidate> {
    let mut candidates = Vec::new();
    let mut matched_pairs = HashSet::new();

    for imported_bill in imported_bills {
        if imported_bill.skip_reconciliation_candidate {
            continue;
        }
        let Some(imported_dt) = bill_datetime(imported_bill) else {
            continue;
        };
        let import_bill_key = build_reconciliation_import_key(imported_bill);
        for existing_bill in existing_bills {
            let Some(existing_dt) = bill_datetime(existing_bill) else {
                continue;
            };
            if !same_day(imported_dt, existing_dt) {
                continue;
            }
            let time_diff_seconds = (imported_dt - existing_dt).num_seconds().abs();
            if time_diff_seconds > TIME_TOLERANCE_SECONDS {
                continue;
            }

            let Some((candidate_type, reason)) =
                resolve_reconciliation_candidate_type(imported_bill, existing_bill)
            else {
                continue;
            };
            let existing_bill_id = existing_bill.id.clone();
            let pair_key = format!(
                "{}|{}|{}",
                candidate_type.as_str(),
                existing_bill_id.as_deref().unwrap_or_default(),
                import_bill_key
            );
            if !matched_pairs.insert(pair_key) {
                continue;
            }

            let amount_abs = Money::from_cents(abs_cents(imported_bill.amount) as i64);
            let day_key = imported_dt.date().format("%Y-%m-%d").to_string();
            let group_key = format!(
                "import_reconciliation:{}:bill:{}:amount:{}:{}",
                candidate_type.as_str(),
                existing_bill_id.as_deref().unwrap_or_default(),
                amount_abs.to_yuan_string(),
                day_key
            );
            let time_score_percent = 100
                - ((time_diff_seconds * 100) / TIME_TOLERANCE_SECONDS.max(1)).clamp(0, 100) as u8;
            let score_percent = 80 + ((19 * u16::from(time_score_percent)) / 100) as u8;
            candidates.push(ImportReconciliationCandidate {
                candidate_id: build_reconciliation_candidate_id(
                    candidate_type,
                    existing_bill_id.as_deref().unwrap_or_default(),
                    &import_bill_key,
                ),
                candidate_type,
                import_bill_key: import_bill_key.clone(),
                existing_bill_id,
                group_key,
                amount_abs,
                time_diff_seconds,
                score_percent,
                level: if time_diff_seconds <= TIME_TOLERANCE_SECONDS {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                reason,
            });
        }
    }

    candidates
}

fn find_exact_duplicates(bills: &mut [DedupBill]) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();
    let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, bill) in bills.iter().enumerate() {
        if !bill.removed {
            by_key.entry(dedup_key(bill)).or_default().push(index);
        }
    }

    for mut indices in by_key.into_values() {
        if indices.len() <= 1 {
            continue;
        }
        indices.sort_by_key(|index| source_priority(&bills[*index].source_type()));
        let keep_index = indices[0];
        let remove_indices = indices[1..].to_vec();
        for index in &remove_indices {
            bills[*index].removed = true;
        }
        groups.push(DuplicateGroup {
            dedup_type: DeduplicationType::Exact,
            bill_indices: indices,
            keep_index,
            remove_indices,
            reason: format!(
                "完全重复，保留 {} 来源",
                bills[keep_index].source_identifier()
            ),
        });
    }

    groups
}

fn find_platform_bank_duplicates(bills: &mut [DedupBill]) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();
    let source_types: Vec<String> = bills.iter().map(DedupBill::source_type).collect();
    let datetimes: Vec<Option<NaiveDateTime>> = bills.iter().map(bill_datetime).collect();
    let timestamps = timestamps_from_datetimes(&datetimes);
    let amount_cents = amount_cents_for_bills(bills);
    let platform_indices: Vec<usize> = bills
        .iter()
        .enumerate()
        .filter_map(|(index, bill)| {
            (!bill.removed && is_platform_source(&source_types[index])).then_some(index)
        })
        .collect();
    let bank_indices: Vec<usize> = bills
        .iter()
        .enumerate()
        .filter_map(|(index, bill)| {
            (!bill.removed && is_bank_source(&source_types[index])).then_some(index)
        })
        .collect();

    if platform_indices.is_empty() || bank_indices.is_empty() {
        return groups;
    }

    let bank_time_amount_buckets =
        build_time_amount_buckets(bank_indices, &timestamps, |index| amount_cents[index].abs());
    let mut matched_bank_indices = HashSet::new();
    for platform_index in platform_indices {
        let Some(platform_ts) = timestamps[platform_index] else {
            continue;
        };
        let Some(platform_dt) = datetimes[platform_index] else {
            continue;
        };

        for target_amount in amount_tolerance_values(amount_cents[platform_index].abs()) {
            for bank_index in
                nearby_time_amount_indices(&bank_time_amount_buckets, platform_ts, target_amount)
            {
                if matched_bank_indices.contains(&bank_index) || bills[bank_index].removed {
                    continue;
                }
                let Some(bank_dt) = datetimes[bank_index] else {
                    continue;
                };
                if !time_close(platform_dt, bank_dt, TIME_TOLERANCE_SECONDS)
                    || !abs_amount_close(bills[platform_index].amount, bills[bank_index].amount)
                    || !is_platform_bank_duplicate_candidate(
                        &bills[platform_index],
                        &bills[bank_index],
                    )
                {
                    continue;
                }

                matched_bank_indices.insert(bank_index);
                bills[bank_index].removed = true;
                let bank_bill = bills[bank_index].clone();
                merge_bill_fields(&mut bills[platform_index], &bank_bill, true);
                bills[platform_index].dedup_type =
                    Some(DeduplicationType::PlatformBank.as_str().to_string());
                let bill_indices = vec![platform_index, bank_index];
                groups.push(DuplicateGroup {
                    dedup_type: DeduplicationType::PlatformBank,
                    bill_indices,
                    keep_index: platform_index,
                    remove_indices: vec![bank_index],
                    reason: format!(
                        "支付平台({})与银行({})重复，金额={}，保留平台账单",
                        bills[platform_index].source_type(),
                        bills[bank_index].source_type(),
                        bills[platform_index].amount.to_yuan_string()
                    ),
                });
            }
        }
    }

    groups
}

fn find_transfer_pairs(bills: &mut [DedupBill]) -> Vec<TransferPair> {
    let mut pairs = Vec::new();
    if !has_multiple_active_sources(bills, DedupBill::source_identifier) {
        return pairs;
    }

    let source_identifiers: Vec<String> = bills.iter().map(DedupBill::source_identifier).collect();
    let datetimes: Vec<Option<NaiveDateTime>> = bills.iter().map(bill_datetime).collect();
    let timestamps = timestamps_from_datetimes(&datetimes);
    let amount_cents = amount_cents_for_bills(bills);
    let time_amount_buckets =
        build_time_amount_buckets(0..bills.len(), &timestamps, |index| amount_cents[index]);
    let mut matched_indices = HashSet::new();

    for left_index in 0..bills.len() {
        if bills[left_index].removed
            || matched_indices.contains(&left_index)
            || bills[left_index].amount == Money::ZERO
        {
            continue;
        }
        let left_source = &source_identifiers[left_index];
        if left_source.is_empty() {
            continue;
        }
        let Some(left_ts) = timestamps[left_index] else {
            continue;
        };
        let Some(left_dt) = datetimes[left_index] else {
            continue;
        };
        let mut matched_right_index = None;
        for target_amount in amount_tolerance_values(-amount_cents[left_index]) {
            for right_index in
                nearby_time_amount_indices(&time_amount_buckets, left_ts, target_amount)
            {
                if right_index <= left_index {
                    continue;
                }
                if bills[right_index].removed
                    || matched_indices.contains(&right_index)
                    || bills[right_index].amount == Money::ZERO
                {
                    continue;
                }
                let right_source = &source_identifiers[right_index];
                if right_source.is_empty() || right_source == left_source {
                    continue;
                }
                if !amount_opposite(bills[left_index].amount, bills[right_index].amount) {
                    continue;
                }
                let Some(right_dt) = datetimes[right_index] else {
                    continue;
                };
                if !time_close(left_dt, right_dt, TIME_TOLERANCE_SECONDS) {
                    continue;
                }
                matched_right_index = Some(right_index);
                break;
            }
            if matched_right_index.is_some() {
                break;
            }
        }

        let Some(right_index) = matched_right_index else {
            continue;
        };
        let (outgoing_index, incoming_index) = if bills[left_index].amount.is_negative() {
            (left_index, right_index)
        } else {
            (right_index, left_index)
        };
        matched_indices.insert(outgoing_index);
        matched_indices.insert(incoming_index);
        bills[outgoing_index].transaction_type = "转账".to_string();
        bills[outgoing_index].dedup_type = Some(DeduplicationType::Transfer.as_str().to_string());
        bills[incoming_index].transaction_type = "转账".to_string();
        bills[incoming_index].removed = true;
        bills[incoming_index].merged_into = bills[outgoing_index].template_id.clone();
        let incoming_bill = bills[incoming_index].clone();
        bills[outgoing_index].transfer_pair_order = Some("outgoing_first".to_string());
        bills[outgoing_index].transfer_pair_sources = vec![
            build_transfer_source_snapshot(&bills[outgoing_index], "outgoing"),
            build_transfer_source_snapshot(&incoming_bill, "incoming"),
        ];
        bills[outgoing_index].destination_parser_id = incoming_bill.parser_id.clone();
        bills[outgoing_index].destination_payment_method = incoming_bill.payment_method.clone();
        bills[outgoing_index].destination_counterparty = incoming_bill.counterparty.clone();
        bills[outgoing_index].destination_account_id =
            Some(incoming_bill.source_account_id.clone()).filter(|value| !value.is_empty());
        bills[outgoing_index].destination_account_name = first_non_empty([
            incoming_bill.account_name.as_str(),
            incoming_bill.payment_method.as_str(),
        ]);
        merge_template_ids_from_bill(&mut bills[outgoing_index], &incoming_bill);
        bills[outgoing_index].description = merge_field_values(
            &bills[outgoing_index].description,
            &incoming_bill.description,
        );
        pairs.push(TransferPair {
            outgoing_index,
            incoming_index,
            cross_batch_db_id: None,
        });
    }

    pairs
}

fn find_similar_duplicates(bills: &mut [DedupBill]) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();
    if !has_multiple_active_sources(bills, DedupBill::source_type) {
        return groups;
    }

    let source_types: Vec<String> = bills.iter().map(DedupBill::source_type).collect();
    let datetimes: Vec<Option<NaiveDateTime>> = bills.iter().map(bill_datetime).collect();
    let timestamps = timestamps_from_datetimes(&datetimes);
    let amount_cents = amount_cents_for_bills(bills);
    let time_amount_buckets =
        build_time_amount_buckets(0..bills.len(), &timestamps, |index| amount_cents[index]);
    let mut matched_indices = HashSet::new();

    for left_index in 0..bills.len() {
        if bills[left_index].removed || matched_indices.contains(&left_index) {
            continue;
        }
        let left_source = &source_types[left_index];
        if left_source.is_empty() {
            continue;
        }
        let Some(left_ts) = timestamps[left_index] else {
            continue;
        };
        let Some(left_dt) = datetimes[left_index] else {
            continue;
        };
        let mut matched_right_index = None;
        for candidate_amount in amount_tolerance_values(amount_cents[left_index]) {
            for right_index in
                nearby_time_amount_indices(&time_amount_buckets, left_ts, candidate_amount)
            {
                if right_index <= left_index {
                    continue;
                }
                if bills[right_index].removed || matched_indices.contains(&right_index) {
                    continue;
                }
                let right_source = &source_types[right_index];
                if left_source.is_empty() || right_source.is_empty() || left_source == right_source
                {
                    continue;
                }
                if !amount_equal_same_direction(bills[left_index].amount, bills[right_index].amount)
                {
                    continue;
                }
                let Some(right_dt) = datetimes[right_index] else {
                    continue;
                };
                if !time_close(left_dt, right_dt, TIME_TOLERANCE_SECONDS) {
                    continue;
                }
                matched_right_index = Some(right_index);
                break;
            }
            if matched_right_index.is_some() {
                break;
            }
        }

        let Some(right_index) = matched_right_index else {
            continue;
        };
        let counterparty_similarity = normalized_similarity(
            &bills[left_index].counterparty,
            &bills[right_index].counterparty,
        );
        let payment_similarity = normalized_similarity(
            &bills[left_index].payment_method,
            &bills[right_index].payment_method,
        );
        if counterparty_similarity < SIMILARITY_THRESHOLD
            && payment_similarity < SIMILARITY_THRESHOLD
        {
            continue;
        }

        let right_source = &source_types[right_index];
        let (keep_index, remove_index) =
            if source_priority(left_source) <= source_priority(right_source) {
                (left_index, right_index)
            } else {
                (right_index, left_index)
            };
        matched_indices.insert(keep_index);
        matched_indices.insert(remove_index);
        bills[remove_index].removed = true;
        let remove_bill = bills[remove_index].clone();
        merge_bill_fields(&mut bills[keep_index], &remove_bill, false);
        bills[keep_index].dedup_type = Some(DeduplicationType::Similar.as_str().to_string());
        groups.push(DuplicateGroup {
            dedup_type: DeduplicationType::Similar,
            bill_indices: vec![left_index, right_index],
            keep_index,
            remove_indices: vec![remove_index],
            reason: "类似账单去重".to_string(),
        });
    }

    groups
}

fn find_split_bills(bills: &mut [DedupBill]) -> Vec<SplitGroup> {
    let mut groups = Vec::new();
    if !has_multiple_active_sources(bills, |bill| normalized_source(&bill.source_account_id)) {
        return groups;
    }

    let source_account_ids: Vec<String> = bills
        .iter()
        .map(|bill| normalized_source(&bill.source_account_id))
        .collect();
    let datetimes: Vec<Option<NaiveDateTime>> = bills.iter().map(bill_datetime).collect();
    let timestamps = timestamps_from_datetimes(&datetimes);
    let amount_cents = amount_cents_for_bills(bills);
    let mut matched_indices = HashSet::new();
    let active_indices: Vec<usize> = bills
        .iter()
        .enumerate()
        .filter_map(|(index, bill)| (!bill.removed && bill.amount != Money::ZERO).then_some(index))
        .collect();
    let split_groups_by_source = build_split_source_groups(
        active_indices.iter().copied(),
        &timestamps,
        &amount_cents,
        &source_account_ids,
    );
    let split_group_ranges = build_split_group_ranges(&split_groups_by_source, &amount_cents);

    for total_index in &active_indices {
        if matched_indices.contains(total_index) || abs_cents(bills[*total_index].amount) < 1_000 {
            continue;
        }
        let Some(total_ts) = timestamps[*total_index] else {
            continue;
        };
        let Some(total_dt) = datetimes[*total_index] else {
            continue;
        };
        let total_source = &source_account_ids[*total_index];
        if total_source.is_empty() {
            continue;
        }

        let total = SplitTotalCandidate {
            index: *total_index,
            timestamp: total_ts,
            datetime: total_dt,
            source: total_source,
            amount_cents: amount_cents[*total_index],
        };
        let Some((candidates, source_splits)) = find_split_candidates_for_total(
            &split_groups_by_source,
            &split_group_ranges,
            &amount_cents,
            &datetimes,
            &matched_indices,
            total,
        ) else {
            continue;
        };

        bills[*total_index].removed = true;
        matched_indices.insert(*total_index);
        let total_template_id = bills[*total_index].template_id.clone();
        for candidate_index in &candidates {
            matched_indices.insert(*candidate_index);
            bills[*candidate_index].dedup_type = Some("split".to_string());
            merge_template_id(&mut bills[*candidate_index], &total_template_id);
        }
        groups.push(SplitGroup {
            total_index: *total_index,
            split_indices: candidates,
            total_amount: bills[*total_index].amount,
            source_total: total_source.clone(),
            source_splits,
            reason: format!("总账单({total_source})拆分为多笔分账单"),
        });
    }

    groups
}

fn has_multiple_active_sources<F>(bills: &[DedupBill], mut source_for: F) -> bool
where
    F: FnMut(&DedupBill) -> String,
{
    let mut first_source: Option<String> = None;
    for bill in bills {
        if bill.removed {
            continue;
        }
        let source = source_for(bill);
        if source.is_empty() {
            continue;
        }
        match &first_source {
            Some(existing) if existing != &source => return true,
            Some(_) => {}
            None => first_source = Some(source),
        }
    }
    false
}

fn timestamps_from_datetimes(datetimes: &[Option<NaiveDateTime>]) -> Vec<Option<i64>> {
    datetimes
        .iter()
        .map(|datetime| datetime.map(|value| value.and_utc().timestamp()))
        .collect()
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct TimeAmountKey {
    bucket: i64,
    amount_cents: i128,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct SplitSourceKey {
    bucket: i64,
    direction: i8,
    source: String,
}

#[derive(Debug, Clone, Copy)]
struct SplitTotalCandidate<'a> {
    index: usize,
    timestamp: i64,
    datetime: NaiveDateTime,
    source: &'a str,
    amount_cents: i128,
}

#[derive(Debug, Clone, Copy)]
struct SplitAmountRange {
    min_possible_sum: i128,
    max_possible_sum: i128,
}

fn amount_cents_for_bills(bills: &[DedupBill]) -> Vec<i128> {
    bills
        .iter()
        .map(|bill| i128::from(bill.amount.to_cents()))
        .collect()
}

fn build_time_amount_buckets<I, F>(
    indices: I,
    timestamps: &[Option<i64>],
    mut amount_for: F,
) -> HashMap<TimeAmountKey, Vec<usize>>
where
    I: IntoIterator<Item = usize>,
    F: FnMut(usize) -> i128,
{
    let mut buckets: HashMap<TimeAmountKey, Vec<usize>> = HashMap::new();
    for index in indices {
        if let Some(timestamp) = timestamps.get(index).and_then(|value| *value) {
            buckets
                .entry(TimeAmountKey {
                    bucket: time_bucket_key(timestamp),
                    amount_cents: amount_for(index),
                })
                .or_default()
                .push(index);
        }
    }
    buckets
}

fn nearby_time_amount_indices(
    buckets: &HashMap<TimeAmountKey, Vec<usize>>,
    timestamp: i64,
    amount_cents: i128,
) -> impl Iterator<Item = usize> + '_ {
    let bucket = time_bucket_key(timestamp);
    (bucket - 1..=bucket + 1).flat_map(move |nearby_bucket| {
        buckets
            .get(&TimeAmountKey {
                bucket: nearby_bucket,
                amount_cents,
            })
            .into_iter()
            .flat_map(|values| values.iter().copied())
    })
}

fn amount_tolerance_values(amount_cents: i128) -> impl Iterator<Item = i128> {
    (amount_cents - AMOUNT_TOLERANCE_CENTS)..=(amount_cents + AMOUNT_TOLERANCE_CENTS)
}

fn build_split_source_groups<I>(
    indices: I,
    timestamps: &[Option<i64>],
    amount_cents: &[i128],
    source_account_ids: &[String],
) -> HashMap<SplitSourceKey, Vec<usize>>
where
    I: IntoIterator<Item = usize>,
{
    let mut groups: HashMap<SplitSourceKey, Vec<usize>> = HashMap::new();
    for index in indices {
        let Some(timestamp) = timestamps.get(index).and_then(|value| *value) else {
            continue;
        };
        let source = source_account_ids[index].clone();
        if source.is_empty() {
            continue;
        }
        let direction = amount_direction(amount_cents[index]);
        if direction == 0 {
            continue;
        }
        groups
            .entry(SplitSourceKey {
                bucket: time_bucket_key(timestamp),
                direction,
                source,
            })
            .or_default()
            .push(index);
    }
    groups
}

fn build_split_group_ranges(
    groups: &HashMap<SplitSourceKey, Vec<usize>>,
    amount_cents: &[i128],
) -> HashMap<SplitSourceKey, SplitAmountRange> {
    groups
        .iter()
        .filter_map(|(key, indices)| {
            split_group_possible_sum_range(indices, amount_cents).map(|range| (key.clone(), range))
        })
        .collect()
}

fn split_group_possible_sum_range(
    indices: &[usize],
    amount_cents: &[i128],
) -> Option<SplitAmountRange> {
    if indices.len() < 2 {
        return None;
    }

    let mut values = indices
        .iter()
        .map(|index| amount_cents[*index])
        .collect::<Vec<_>>();
    values.sort_unstable();

    let full_sum = values.iter().sum::<i128>();
    let first = *values.first()?;
    let second = values.get(1).copied()?;
    let last = *values.last()?;
    let before_last = values.get(values.len().saturating_sub(2)).copied()?;

    if last < 0 {
        Some(SplitAmountRange {
            min_possible_sum: full_sum,
            max_possible_sum: before_last + last,
        })
    } else if first > 0 {
        Some(SplitAmountRange {
            min_possible_sum: first + second,
            max_possible_sum: full_sum,
        })
    } else {
        None
    }
}

fn split_group_range_can_match(
    ranges: &HashMap<SplitSourceKey, SplitAmountRange>,
    key: &SplitSourceKey,
    target_amount_cents: i128,
) -> bool {
    ranges
        .get(key)
        .map(|range| {
            target_amount_cents >= range.min_possible_sum - AMOUNT_TOLERANCE_CENTS
                && target_amount_cents <= range.max_possible_sum + AMOUNT_TOLERANCE_CENTS
        })
        .unwrap_or(false)
}

fn find_split_candidates_for_total(
    groups: &HashMap<SplitSourceKey, Vec<usize>>,
    group_ranges: &HashMap<SplitSourceKey, SplitAmountRange>,
    amount_cents: &[i128],
    datetimes: &[Option<NaiveDateTime>],
    matched_indices: &HashSet<usize>,
    total: SplitTotalCandidate<'_>,
) -> Option<(Vec<usize>, String)> {
    let direction = amount_direction(total.amount_cents);
    if direction == 0 {
        return None;
    }
    let total_bucket = time_bucket_key(total.timestamp);
    let mut source_keys = groups
        .keys()
        .filter(|key| {
            key.direction == direction
                && key.source != total.source
                && (total_bucket - 1..=total_bucket + 1).contains(&key.bucket)
        })
        .collect::<Vec<_>>();
    source_keys.sort_by(|left, right| {
        let left_first = groups
            .get(*left)
            .and_then(|indices| indices.iter().min())
            .copied()
            .unwrap_or(usize::MAX);
        let right_first = groups
            .get(*right)
            .and_then(|indices| indices.iter().min())
            .copied()
            .unwrap_or(usize::MAX);
        left_first
            .cmp(&right_first)
            .then_with(|| left.bucket.cmp(&right.bucket))
            .then_with(|| left.source.cmp(&right.source))
    });

    for key in source_keys {
        if !split_group_range_can_match(group_ranges, key, total.amount_cents) {
            continue;
        }
        let candidates = groups
            .get(key)?
            .iter()
            .copied()
            .filter(|candidate_index| {
                *candidate_index != total.index && !matched_indices.contains(candidate_index)
            })
            .filter(|candidate_index| {
                datetimes[*candidate_index]
                    .map(|candidate_dt| {
                        time_close(total.datetime, candidate_dt, TIME_TOLERANCE_SECONDS)
                    })
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        if candidates.len() < 2 {
            continue;
        }
        let split_sum = candidates
            .iter()
            .map(|index| amount_cents[*index])
            .sum::<i128>();
        if (split_sum - total.amount_cents).abs() <= AMOUNT_TOLERANCE_CENTS {
            return Some((candidates, key.source.clone()));
        }
    }
    None
}

fn amount_direction(amount_cents: i128) -> i8 {
    match amount_cents.cmp(&0) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn time_bucket_key(timestamp: i64) -> i64 {
    timestamp.div_euclid(TIME_TOLERANCE_SECONDS.max(1) + 1)
}

fn dedup_key(bill: &DedupBill) -> String {
    format!(
        "{}|{}|{}|{}",
        bill.date,
        bill.amount.to_cents(),
        bill.counterparty,
        bill.description
    )
}

fn clean_runtime_markers(mut bill: DedupBill) -> DedupBill {
    bill.removed = false;
    bill.merged_from.clear();
    bill.duplicate_of_db_id = None;
    bill
}

fn merge_template_id(target: &mut DedupBill, template_id: &Option<String>) {
    if let Some(template_id) = template_id {
        append_id(&mut target.merged_template_ids, template_id);
    }
}

fn merge_template_ids_from_bill(target: &mut DedupBill, secondary: &DedupBill) {
    append_optional_id(&mut target.merged_template_ids, &secondary.template_id);
    append_ids(
        &mut target.merged_template_ids,
        &secondary.merged_template_ids,
    );
}

fn merge_bill_fields(target: &mut DedupBill, secondary: &DedupBill, merge_parser_tags: bool) {
    target.counterparty = merge_field_values(&target.counterparty, &secondary.counterparty);
    target.payment_method = merge_field_values(&target.payment_method, &secondary.payment_method);
    target.description = merge_field_values(&target.description, &secondary.description);
    target.merged_from.push(MergedBillSource {
        source: secondary.source_account_id.clone(),
        date: secondary.date.clone(),
        amount: Some(secondary.amount.to_yuan_string()),
        template_id: secondary.template_id.clone(),
    });
    merge_template_ids_from_bill(target, secondary);
    if merge_parser_tags {
        merge_parser_tags_from_bill(target, secondary);
    }
}

fn merge_field_values(left: &str, right: &str) -> String {
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() && right.is_empty() {
        return String::new();
    }
    if left.is_empty() {
        return right.to_string();
    }
    if right.is_empty() || left == right || left.contains(right) {
        return left.to_string();
    }
    if right.contains(left) {
        return right.to_string();
    }

    let mut parts = Vec::new();
    let mut seen = HashSet::new();
    for value in [left, right] {
        for part in value
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if let Some(existing_index) = parts
                .iter()
                .position(|existing: &String| part.contains(existing) || existing.contains(part))
            {
                if part.len() > parts[existing_index].len() {
                    seen.remove(&parts[existing_index]);
                    parts[existing_index] = part.to_string();
                    seen.insert(part.to_string());
                }
            } else if seen.insert(part.to_string()) {
                parts.push(part.to_string());
            }
        }
    }
    parts.join(" | ")
}

fn merge_parser_tags_from_bill(target: &mut DedupBill, secondary: &DedupBill) {
    let mut tags = normalized_parser_tags(target);
    for tag in normalized_parser_tags(secondary) {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    target.parser_tags = tags;
}

fn normalized_parser_tags(bill: &DedupBill) -> Vec<String> {
    let mut tags = Vec::new();
    for tag in &bill.parser_tags {
        let tag = tag.trim().to_lowercase();
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    if tags.is_empty() {
        let source_type = bill.source_type();
        if !source_type.is_empty() {
            tags.push(format!("parser:{source_type}"));
        }
    }
    tags
}

fn build_transfer_source_snapshot(bill: &DedupBill, role: &str) -> TransferSourceSnapshot {
    TransferSourceSnapshot {
        role: role.to_string(),
        parser_id: bill.parser_id.clone(),
        payment_method: bill.payment_method.clone(),
        counterparty: bill.counterparty.clone(),
        source_account_id: Some(bill.source_account_id.clone()).filter(|value| !value.is_empty()),
        account_name: first_non_empty([bill.account_name.as_str(), bill.payment_method.as_str()]),
        tags: normalized_parser_tags(bill),
    }
}

fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .find_map(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then_some(trimmed.to_string())
        })
        .unwrap_or_default()
}

fn append_optional_id(target: &mut Vec<String>, value: &Option<String>) {
    if let Some(value) = value {
        append_id(target, value);
    }
}

fn append_ids(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        append_id(target, value);
    }
}

fn append_id(target: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() && !target.iter().any(|existing| existing == value) {
        target.push(value.to_string());
    }
}

fn is_platform_bank_duplicate_candidate(platform_bill: &DedupBill, bank_bill: &DedupBill) -> bool {
    if amount_same_direction(platform_bill.amount, bank_bill.amount) {
        return true;
    }
    if has_transfer_intent_keywords(platform_bill) || has_transfer_intent_keywords(bank_bill) {
        return false;
    }
    has_platform_bank_duplicate_text_evidence(platform_bill, bank_bill)
}

fn has_transfer_intent_keywords(bill: &DedupBill) -> bool {
    let text = bill_text_for_intent(bill).to_lowercase();
    TRANSFER_INTENT_KEYWORDS
        .iter()
        .any(|keyword| text.contains(&keyword.to_lowercase()))
}

fn has_platform_bank_duplicate_text_evidence(
    platform_bill: &DedupBill,
    bank_bill: &DedupBill,
) -> bool {
    for (left, right) in [
        (&platform_bill.counterparty, &bank_bill.counterparty),
        (&platform_bill.description, &bank_bill.description),
        (&platform_bill.payment_method, &bank_bill.payment_method),
        (
            &platform_bill.original_category,
            &bank_bill.original_category,
        ),
    ] {
        if text_evidence_matches(left, right, SIMILARITY_THRESHOLD) {
            return true;
        }
    }
    text_evidence_matches(
        &bill_text_for_intent(platform_bill),
        &bill_text_for_intent(bank_bill),
        0.62,
    )
}

fn duplicate_text_match(left: &DedupBill, right: &DedupBill, is_platform_bank_pair: bool) -> bool {
    let desc_similar =
        text_evidence_matches(&left.description, &right.description, SIMILARITY_THRESHOLD);
    let counterparty_similar = text_evidence_matches(
        &left.counterparty,
        &right.counterparty,
        SIMILARITY_THRESHOLD,
    );
    if is_platform_bank_pair {
        desc_similar || counterparty_similar
    } else {
        desc_similar
            || counterparty_similar
            || (left.description.is_empty() && right.description.is_empty())
    }
}

fn text_evidence_matches(left: &str, right: &str, threshold: f64) -> bool {
    let left = left.trim().to_lowercase();
    let right = right.trim().to_lowercase();
    if left.is_empty() || right.is_empty() {
        return false;
    }
    left.contains(&right)
        || right.contains(&left)
        || normalized_similarity(&left, &right) >= threshold
}

fn bill_text_for_intent(bill: &DedupBill) -> String {
    [
        &bill.counterparty,
        &bill.payment_method,
        &bill.description,
        &bill.original_category,
        &bill.main_category,
        &bill.sub_category,
    ]
    .iter()
    .filter_map(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
    .collect::<Vec<_>>()
    .join(" ")
}

fn bill_datetime(bill: &DedupBill) -> Option<NaiveDateTime> {
    parse_bill_datetime(&bill.date).map(|value| value.inner())
}

fn time_close(left: NaiveDateTime, right: NaiveDateTime, tolerance_seconds: i64) -> bool {
    (left - right).num_seconds().abs() <= tolerance_seconds
}

fn same_day(left: NaiveDateTime, right: NaiveDateTime) -> bool {
    left.date() == right.date()
}

fn abs_amount_close(left: Money, right: Money) -> bool {
    (abs_cents(left) - abs_cents(right)).abs() <= AMOUNT_TOLERANCE_CENTS
}

fn amount_equal_same_direction(left: Money, right: Money) -> bool {
    abs_amount_close(left, right) && amount_same_direction(left, right)
}

fn amount_opposite(left: Money, right: Money) -> bool {
    abs_amount_close(left, right) && i128::from(left.to_cents()) * i128::from(right.to_cents()) < 0
}

fn amount_same_direction(left: Money, right: Money) -> bool {
    (left.to_cents() >= 0 && right.to_cents() >= 0) || (left.to_cents() < 0 && right.to_cents() < 0)
}

fn abs_cents(amount: Money) -> i128 {
    let cents = i128::from(amount.to_cents());
    if cents < 0 {
        -cents
    } else {
        cents
    }
}

fn normalized_similarity(left: &str, right: &str) -> f64 {
    let left = left.trim().to_lowercase();
    let right = right.trim().to_lowercase();
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    if left == right || left.contains(&right) || right.contains(&left) {
        return 1.0;
    }

    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    let lcs = longest_common_subsequence_len(&left_chars, &right_chars);
    (2.0 * lcs as f64) / (left_chars.len() + right_chars.len()) as f64
}

fn build_reconciliation_import_key(bill: &DedupBill) -> String {
    if let Some(preview_id) = &bill.preview_id {
        if !preview_id.trim().is_empty() {
            return format!("preview:{}", preview_id.trim());
        }
    }

    if let (Some(session_id), Some(template_id)) = (&bill.session_id, &bill.template_id) {
        if !session_id.trim().is_empty() && !template_id.trim().is_empty() {
            return format!(
                "session:{}:template:{}",
                session_id.trim(),
                template_id.trim()
            );
        }
    }

    let key = format!(
        "{}|{}|{}|{}|{}|{}",
        bill.date,
        bill.amount.to_yuan_string(),
        bill.counterparty,
        bill.description,
        bill.parser_id,
        bill.source_account_id
    );
    format!("hash:{}", stable_hash_hex(&key))
}

fn build_reconciliation_candidate_id(
    candidate_type: ReconciliationCandidateType,
    existing_bill_id: &str,
    import_bill_key: &str,
) -> String {
    format!(
        "reconcile:import:{}:bill:{}:{}",
        candidate_type.as_str(),
        existing_bill_id,
        stable_hash_hex(import_bill_key)
    )
}

fn resolve_reconciliation_candidate_type(
    imported_bill: &DedupBill,
    existing_bill: &DedupBill,
) -> Option<(ReconciliationCandidateType, String)> {
    if (abs_cents(imported_bill.amount) - abs_cents(existing_bill.amount)).abs()
        > AMOUNT_TOLERANCE_CENTS
    {
        return None;
    }

    let imported_has_transfer_intent = has_transfer_intent_keywords(imported_bill);
    let existing_has_transfer_intent = has_transfer_intent_keywords(existing_bill);
    let duplicate_evidence = has_reconciliation_duplicate_evidence(imported_bill, existing_bill);

    if amount_opposite(imported_bill.amount, existing_bill.amount) {
        if duplicate_evidence && !(imported_has_transfer_intent || existing_has_transfer_intent) {
            return Some((
                ReconciliationCandidateType::Duplicate,
                "opposite_amount|duplicate_text_evidence".to_string(),
            ));
        }

        let imported_source = reconciliation_source_token(imported_bill);
        let existing_source = reconciliation_source_token(existing_bill);
        if !imported_source.is_empty() && imported_source == existing_source {
            return None;
        }
        return Some((
            ReconciliationCandidateType::Transfer,
            "opposite_amount|same_day|time_close".to_string(),
        ));
    }

    if amount_equal_same_direction(imported_bill.amount, existing_bill.amount) {
        let mut reason = "same_amount|same_direction|same_day|time_close".to_string();
        if duplicate_evidence {
            reason.push_str("|duplicate_text_evidence");
        }
        return Some((ReconciliationCandidateType::Duplicate, reason));
    }

    None
}

fn has_reconciliation_duplicate_evidence(
    imported_bill: &DedupBill,
    existing_bill: &DedupBill,
) -> bool {
    for (left, right) in [
        (&imported_bill.counterparty, &existing_bill.counterparty),
        (&imported_bill.description, &existing_bill.description),
        (&imported_bill.payment_method, &existing_bill.payment_method),
    ] {
        if text_evidence_matches(left, right, SIMILARITY_THRESHOLD) {
            return true;
        }
    }

    let imported_text = bill_text_for_intent(imported_bill);
    let existing_text = bill_text_for_intent(existing_bill);
    if imported_text.is_empty() && existing_text.is_empty() {
        return true;
    }
    text_evidence_matches(&imported_text, &existing_text, 0.62)
}

fn reconciliation_source_token(bill: &DedupBill) -> String {
    for value in [
        bill.parser_id.as_str(),
        bill.source.as_str(),
        bill.source_account_id.as_str(),
        bill.payment_method.as_str(),
    ] {
        let value = value.trim().to_lowercase();
        if !value.is_empty() && value != "0" {
            return value;
        }
    }
    String::new()
}

fn stable_hash_hex(input: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn longest_common_subsequence_len(left: &[char], right: &[char]) -> usize {
    let mut previous = vec![0; right.len() + 1];
    let mut current = vec![0; right.len() + 1];

    for left_char in left {
        for (right_index, right_char) in right.iter().enumerate() {
            current[right_index + 1] = if left_char == right_char {
                previous[right_index] + 1
            } else {
                previous[right_index + 1].max(current[right_index])
            };
        }
        std::mem::swap(&mut previous, &mut current);
        current.fill(0);
    }

    previous[right.len()]
}

fn source_priority(source_id: &str) -> i32 {
    match normalized_source(source_id).as_str() {
        "wechat" => 1,
        "alipay" => 2,
        "icbc" | "cmbc" | "abc" | "ccb" => 10,
        _ => 100,
    }
}

fn normalized_source(source_id: &str) -> String {
    source_id.trim().to_lowercase()
}

fn is_platform_source(source: &str) -> bool {
    PLATFORM_SOURCES.contains(&source)
}

fn is_bank_source(source: &str) -> bool {
    BANK_SOURCES.contains(&source)
}

fn default_money() -> Money {
    Money::ZERO
}

fn deserialize_yuan_money<'de, D>(deserializer: D) -> Result<Money, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    let text = match value {
        Value::Null => return Ok(Money::ZERO),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text,
        other => {
            return Err(D::Error::custom(format!(
                "expected yuan amount string or number, got {other}"
            )))
        }
    };
    if text.trim().is_empty() {
        return Ok(Money::ZERO);
    }
    Money::from_yuan_str(&text).map_err(D::Error::custom)
}

fn serialize_yuan_money<S>(amount: &Money, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_f64(amount.to_cents() as f64 / 100.0)
}

fn deserialize_stringish<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value_to_string(&value).unwrap_or_default())
}

fn deserialize_optional_stringish<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value_to_string(&value).filter(|value| !value.trim().is_empty()))
}

fn deserialize_string_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Null => Vec::new(),
        Value::Array(values) => values
            .iter()
            .filter_map(value_to_string)
            .filter(|value| !value.trim().is_empty())
            .collect(),
        Value::String(text) => parse_string_vec_text(&text),
        other => value_to_string(&other)
            .filter(|value| !value.trim().is_empty())
            .into_iter()
            .collect(),
    })
}

fn deserialize_boolish<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Bool(value) => value,
        Value::Number(number) => number.as_i64().unwrap_or_default() != 0,
        Value::String(text) => matches!(
            text.trim().to_lowercase().as_str(),
            "true" | "1" | "yes" | "y"
        ),
        _ => false,
    })
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

fn parse_string_vec_text(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if text.starts_with('[') {
        if let Ok(Value::Array(values)) = serde_json::from_str::<Value>(text) {
            return values
                .iter()
                .filter_map(value_to_string)
                .filter(|value| !value.trim().is_empty())
                .collect();
        }
    }
    text.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}
