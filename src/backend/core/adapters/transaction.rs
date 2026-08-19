// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use crate::{
    error::{ErrorCode, RuntimeError},
    ledger::{derive_ledger_balance_effects, LedgerBalanceInput},
    primitives::{parse_bill_datetime, Money, TransactionType, UtcOffsetMinutes},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::{Map, Number, Value};
use unicode_normalization::UnicodeNormalization;

const DEFAULT_UTC_OFFSET_MINUTES: i32 = 480;
const FORMULA_PREFIXES: &[char] = &['=', '+', '-', '@'];
pub const ALLOWED_TRANSACTION_PICTURE_EXTENSIONS: &[&str] =
    &["bmp", "gif", "jpeg", "jpg", "png", "webp"];
const WINDOWS_DEVICE_FILES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];
pub const EXPORT_COLUMNS: &[(&str, &str)] = &[
    ("date", "date"),
    ("type", "type"),
    ("amount_cents", "amount_cents"),
    ("counterparty", "counterparty"),
    ("description", "description"),
    ("payment_method", "payment_method"),
    ("main_category", "main_category"),
    ("sub_category", "sub_category"),
    ("source_account_id", "source_account_id"),
    ("destination_account_id", "destination_account_id"),
    ("destination_amount_cents", "destination_amount_cents"),
];
pub const EXPORT_TEXT_KEYS: &[&str] = &[
    "date",
    "type",
    "counterparty",
    "description",
    "payment_method",
    "main_category",
    "sub_category",
    "source_account",
    "destination_account",
    "tags",
    "comment",
    "created_at",
    "updated_at",
];
pub const BILL_CREATE_COLUMNS: &[&str] = &[
    "user_id",
    "date",
    "type",
    "amount_cents",
    "counterparty",
    "description",
    "payment_method",
    "main_category",
    "sub_category",
    "batch_id",
    "hash",
    "created_at",
    "updated_at",
    "source_account_id",
    "destination_account_id",
    "destination_amount_cents",
    "created_from_template",
    "created_from_recurring",
    "import_history_id",
];
pub const BILL_UPDATE_COLUMNS: &[&str] = &[
    "date",
    "type",
    "amount_cents",
    "counterparty",
    "description",
    "payment_method",
    "main_category",
    "sub_category",
    "batch_id",
    "hash",
    "source_account_id",
    "destination_account_id",
    "destination_amount_cents",
    "created_from_template",
    "created_from_recurring",
    "import_history_id",
];
pub const ROUTE_ALLOWED_BATCH_UPDATE_FIELDS: &[&str] = &[
    "date",
    "type",
    "amount_cents",
    "counterparty",
    "description",
    "payment_method",
    "main_category",
    "sub_category",
    "source_account_id",
    "destination_account_id",
    "destination_amount_cents",
];

include!("transaction/types.rs");
include!("transaction/mutation_payloads.rs");
include!("transaction/batch_routes.rs");
include!("transaction/pictures.rs");
include!("transaction/reconciliation.rs");
include!("transaction/frontend_projection.rs");
include!("transaction/value_helpers.rs");
