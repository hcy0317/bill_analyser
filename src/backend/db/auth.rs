use bill_analyser_core::{auth::recovery_code_hash_input, UserId};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSessionRow {
    pub id: i64,
    pub user_agent: String,
    pub ip_address: String,
    pub expires_at: String,
    pub last_activity_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthTokenUserRow {
    pub id: UserId,
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLoginUserRow {
    pub profile: AuthUserProfileRow,
    pub password_hash: String,
    pub is_active: bool,
    pub two_factor_enabled: bool,
    pub two_factor_secret: String,
    pub failed_login_attempts: i64,
    pub locked_until: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRefreshSessionRow {
    pub id: i64,
    pub user_id: UserId,
    pub username: String,
    pub refresh_expires_at: String,
    pub user_is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLogoutSessionRow {
    pub id: i64,
    pub user_id: UserId,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUserProfileRow {
    pub id: UserId,
    pub username: String,
    pub email: String,
    pub nickname: String,
    pub avatar: String,
    pub default_account_id: Option<i64>,
    pub transaction_edit_scope: i64,
    pub language: String,
    pub default_currency: String,
    pub first_day_of_week: i64,
    pub fiscal_year_start: i64,
    pub calendar_display_type: i64,
    pub date_display_type: i64,
    pub long_date_format: i64,
    pub short_date_format: i64,
    pub long_time_format: i64,
    pub short_time_format: i64,
    pub fiscal_year_format: i64,
    pub currency_display_type: i64,
    pub numeral_system: i64,
    pub decimal_separator: i64,
    pub digit_grouping_symbol: i64,
    pub digit_grouping: i64,
    pub coordinate_display_type: i64,
    pub expense_amount_color: i64,
    pub income_amount_color: i64,
    pub cash_account_id: Option<i64>,
    pub cash_transfer_category_id: Option<i64>,
    pub import_learning_enabled: bool,
    pub investment_platform_keywords: Option<String>,
    pub investment_product_keywords: Option<String>,
    pub investment_exclude_keywords: Option<String>,
    pub email_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationCloudSettingRow {
    pub setting_key: String,
    pub setting_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAuthRow {
    pub external_auth_category: String,
    pub external_auth_type: String,
    pub external_username: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationCloudSettingDraft {
    pub setting_key: String,
    pub setting_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthUserProfileUpdate {
    Nickname(String),
    Email(String),
    Avatar(String),
    Language(String),
    DefaultCurrency(String),
    FirstDayOfWeek(i64),
    DefaultAccountId(Option<i64>),
    TransactionEditScope(i64),
    FiscalYearStart(i64),
    CalendarDisplayType(i64),
    DateDisplayType(i64),
    LongDateFormat(i64),
    ShortDateFormat(i64),
    LongTimeFormat(i64),
    ShortTimeFormat(i64),
    FiscalYearFormat(i64),
    CurrencyDisplayType(i64),
    NumeralSystem(i64),
    DecimalSeparator(i64),
    DigitGroupingSymbol(i64),
    DigitGrouping(i64),
    CoordinateDisplayType(i64),
    ExpenseAmountColor(i64),
    IncomeAmountColor(i64),
    CashAccountId(Option<i64>),
    CashTransferCategoryId(Option<i64>),
    ImportLearningEnabled(bool),
    InvestmentPlatformKeywords(String),
    InvestmentProductKeywords(String),
    InvestmentExcludeKeywords(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTokenSessionDraft {
    pub user_id: UserId,
    pub token_hash: String,
    pub refresh_token_hash: Option<String>,
    pub expires_at: String,
    pub refresh_expires_at: Option<String>,
    pub user_agent: String,
    pub ip_address: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLogDraft {
    pub user_id: Option<UserId>,
    pub username: String,
    pub event_type: String,
    pub ip_address: String,
    pub user_agent: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginFailureUpdate {
    pub failed_attempts: i64,
    pub locked: bool,
}

include!("auth/users.rs");
include!("auth/cloud_settings.rs");
include!("auth/profile.rs");
include!("auth/sessions.rs");
include!("auth/logs.rs");
include!("auth/two_factor.rs");
include!("auth/login_security.rs");
include!("auth/rows_helpers.rs");
include!("../../../tests/backend/db/internal/auth.rs");
