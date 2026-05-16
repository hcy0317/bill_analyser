use rusqlite::{params, Connection, OptionalExtension};

use crate::auth::{create_auth_log, AuthLogDraft};
use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterUserDraft {
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub nickname: String,
    pub language: String,
    pub default_currency: String,
    pub first_day_of_week: i64,
    pub email_verified: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterPresetCategory {
    pub name: String,
    pub type_code: i64,
    pub icon: String,
    pub color: String,
    pub sub_categories: Vec<RegisterPresetSubCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterPresetSubCategory {
    pub name: String,
    pub icon: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterDefaultSeedSummary {
    pub categories_created: i64,
    pub categories_skipped: i64,
    pub rules_created: i64,
    pub rules_skipped: i64,
    pub rules_missing_categories: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterUserResult {
    pub user_id: i64,
    pub preset_categories_saved: bool,
    pub preset_accounts_saved: bool,
    pub cash_account_id: Option<i64>,
    pub default_account_id: Option<i64>,
    pub default_seed: RegisterDefaultSeedSummary,
}

struct DefaultSubCategory {
    name: &'static str,
    icon: &'static str,
    color: &'static str,
}

struct DefaultCategory {
    type_code: i64,
    name: &'static str,
    icon: &'static str,
    color: &'static str,
    priority: i64,
    sub_categories: &'static [DefaultSubCategory],
}

struct DefaultCategoryRule {
    name: &'static str,
    main_category: &'static str,
    sub_category: &'static str,
    rule_expression: &'static str,
    priority: i64,
}

struct DefaultAccountTemplate {
    name: &'static str,
    type_code: i64,
    category: i64,
    currency: &'static str,
    icon: &'static str,
    color: &'static str,
    aliases: &'static [&'static str],
    display_order: i64,
}

const EXPENSE: i64 = 3;
const INCOME: i64 = 2;
const TRANSFER: i64 = 4;

include!("auth_registration/default_subcategories.rs");
include!("auth_registration/default_templates.rs");
include!("auth_registration/registration.rs");
include!("auth_registration/categories.rs");
include!("auth_registration/accounts.rs");
include!("../../../tests/backend/db/internal/auth_registration.rs");
