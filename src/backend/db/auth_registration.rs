// 中文导读：PostgreSQL 注册 DTO。注册写入与默认数据 seed 由 `auth_postgres` 实现。
// 维护重点：不保留 non-Postgres 注册仓储、默认账号名称模板或迁移路径。

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

pub(crate) struct DefaultSubCategory {
    pub(crate) name: &'static str,
    pub(crate) icon: &'static str,
    pub(crate) color: &'static str,
}

pub(crate) struct DefaultCategory {
    pub(crate) type_code: i64,
    pub(crate) name: &'static str,
    pub(crate) icon: &'static str,
    pub(crate) color: &'static str,
    pub(crate) priority: i64,
    pub(crate) sub_categories: &'static [DefaultSubCategory],
}

pub(crate) struct DefaultCategoryRule {
    pub(crate) name: &'static str,
    pub(crate) main_category: &'static str,
    pub(crate) sub_category: &'static str,
    pub(crate) rule_expression: &'static str,
    pub(crate) priority: i64,
}

const EXPENSE: i64 = 3;
const INCOME: i64 = 2;
const TRANSFER: i64 = 4;

const CAT_DINING: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "早餐",
        icon: "2",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "午餐",
        icon: "2",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "晚餐",
        icon: "2",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "外卖",
        icon: "2",
        color: "ff6b22",
    },
];
const CAT_TRANSFER: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "银行卡互转",
        icon: "900",
        color: "2196f3",
    },
    DefaultSubCategory {
        name: "信用卡还款",
        icon: "980",
        color: "2196f3",
    },
];
const CAT_INCOME: &[DefaultSubCategory] = &[
    DefaultSubCategory {
        name: "工资",
        icon: "2010",
        color: "ff6b22",
    },
    DefaultSubCategory {
        name: "退款",
        icon: "920",
        color: "4cd964",
    },
];
const CAT_OTHER_EXPENSE: &[DefaultSubCategory] = &[DefaultSubCategory {
    name: "无法归类",
    icon: "1010",
    color: "8e8e93",
}];

pub(crate) const DEFAULT_DAILY_CATEGORIES: &[DefaultCategory] = &[
    DefaultCategory {
        type_code: EXPENSE,
        name: "餐饮",
        icon: "1",
        color: "ff6b22",
        priority: 100,
        sub_categories: CAT_DINING,
    },
    DefaultCategory {
        type_code: EXPENSE,
        name: "其他支出",
        icon: "1000",
        color: "8e8e93",
        priority: 1300,
        sub_categories: CAT_OTHER_EXPENSE,
    },
    DefaultCategory {
        type_code: INCOME,
        name: "收入",
        icon: "2000",
        color: "4caf50",
        priority: 2000,
        sub_categories: CAT_INCOME,
    },
    DefaultCategory {
        type_code: TRANSFER,
        name: "账户互转",
        icon: "4000",
        color: "2196f3",
        priority: 3000,
        sub_categories: CAT_TRANSFER,
    },
];

pub(crate) const DEFAULT_DAILY_CATEGORY_RULES: &[DefaultCategoryRule] = &[
    DefaultCategoryRule {
        name: "default:餐饮/外卖",
        main_category: "餐饮",
        sub_category: "外卖",
        rule_expression: "OR={美团外卖,饿了么,外卖}",
        priority: 100,
    },
    DefaultCategoryRule {
        name: "default:收入/工资",
        main_category: "收入",
        sub_category: "工资",
        rule_expression: "REGEX={(工资|薪资|薪金)}",
        priority: 2000,
    },
    DefaultCategoryRule {
        name: "default:账户互转/信用卡还款",
        main_category: "账户互转",
        sub_category: "信用卡还款",
        rule_expression: "OR={信用卡还款,还信用卡}",
        priority: 3000,
    },
];
