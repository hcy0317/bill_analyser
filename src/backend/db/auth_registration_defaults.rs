// 中文导读：注册时可选的后端拥有默认包内容。
// 维护重点：保持通用个人/家庭账单覆盖，不写入具体银行、证券、基金或用户私有别名。

use crate::auth_registration::{
    DefaultAccount, DefaultAccountRule, DefaultCategory, DefaultCategoryRule, DefaultSubCategory,
};

include!("auth_registration_defaults/types.rs");
include!("auth_registration_defaults/expense_categories.rs");
include!("auth_registration_defaults/income_transfer_investment.rs");
include!("auth_registration_defaults/categories.rs");
include!("auth_registration_defaults/accounts.rs");
include!("auth_registration_defaults/category_rules.rs");
include!("auth_registration_defaults/account_rules.rs");
include!("auth_registration_defaults/tests.rs");
