// 中文导读：导入学习推荐 key 与生命周期合同。
// 维护重点：recommendation_key 必须稳定且 user-scoped；yellow/green/suppressed 状态转换不能依赖 HTTP/DB 细节。
// 不变式：转账保护标记参与 key，反馈阈值不能出现 off-by-one。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::import_learning::normalize_import_learning_text;

pub const RECOMMENDATION_KEY_SCHEMA_VERSION: &str = "import-learning-recommendation-key-v1";
pub const LEARNING_LIFECYCLE_STATUS_YELLOW: &str = "yellow";
pub const LEARNING_LIFECYCLE_STATUS_GREEN: &str = "green";
pub const LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED: &str = "auto_applied";
pub const LEARNING_LIFECYCLE_STATUS_DOWNGRADED: &str = "downgraded";
pub const LEARNING_LIFECYCLE_STATUS_SUPPRESSED: &str = "suppressed";
pub const LEARNING_LIFECYCLE_ACCEPTS_TO_GREEN: i64 = 3;
pub const LEARNING_LIFECYCLE_GREEN_REJECTS_TO_DOWNGRADE: i64 = 2;
pub const LEARNING_LIFECYCLE_YELLOW_REJECTS_TO_SUPPRESS: i64 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportLearningRecommendationKeyInput {
    pub feature_schema_version: String,
    pub user_id: i64,
    pub recommendation_type: String,
    pub recommended_type: String,
    pub recommended_category_id: Option<i64>,
    pub recommended_source_account_id: Option<i64>,
    pub recommended_destination_account_id: Option<i64>,
    pub transaction_type_scope: String,
    pub parser_bucket: String,
    pub counterparty_bucket: String,
    pub payment_bucket: String,
    pub description_bucket: String,
    pub amount_bucket: Option<String>,
    pub suppression_scope: String,
    pub transfer_protected: bool,
}

impl Default for ImportLearningRecommendationKeyInput {
    fn default() -> Self {
        Self {
            feature_schema_version: RECOMMENDATION_KEY_SCHEMA_VERSION.to_string(),
            user_id: 0,
            recommendation_type: "import_preview".to_string(),
            recommended_type: String::new(),
            recommended_category_id: None,
            recommended_source_account_id: None,
            recommended_destination_account_id: None,
            transaction_type_scope: String::new(),
            parser_bucket: String::new(),
            counterparty_bucket: String::new(),
            payment_bucket: String::new(),
            description_bucket: String::new(),
            amount_bucket: None,
            suppression_scope: "default".to_string(),
            transfer_protected: false,
        }
    }
}

/// 基于用户、推荐 tuple、特征桶和转账保护信息生成稳定的导入学习 recommendation key。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_import_learning_recommendation_key(
    input: &ImportLearningRecommendationKeyInput,
) -> String {
    let mut parts = BTreeMap::new();
    parts.insert("schema", input.feature_schema_version.trim().to_string());
    parts.insert("user", input.user_id.max(0).to_string());
    parts.insert(
        "recommendation_type",
        normalize_import_learning_text(Some(&json!(input.recommendation_type))),
    );
    parts.insert(
        "tuple_type",
        normalize_import_learning_text(Some(&json!(input.recommended_type))),
    );
    parts.insert(
        "tuple_category",
        input
            .recommended_category_id
            .unwrap_or_default()
            .max(0)
            .to_string(),
    );
    parts.insert(
        "tuple_source",
        input
            .recommended_source_account_id
            .unwrap_or_default()
            .max(0)
            .to_string(),
    );
    parts.insert(
        "tuple_destination",
        input
            .recommended_destination_account_id
            .unwrap_or_default()
            .max(0)
            .to_string(),
    );
    parts.insert(
        "type_scope",
        normalize_import_learning_text(Some(&json!(input.transaction_type_scope))),
    );
    parts.insert(
        "parser",
        normalize_import_learning_text(Some(&json!(input.parser_bucket))),
    );
    parts.insert(
        "counterparty",
        normalize_import_learning_text(Some(&json!(input.counterparty_bucket))),
    );
    parts.insert(
        "payment",
        normalize_import_learning_text(Some(&json!(input.payment_bucket))),
    );
    parts.insert(
        "description",
        normalize_import_learning_text(Some(&json!(input.description_bucket))),
    );
    parts.insert(
        "amount",
        input
            .amount_bucket
            .as_deref()
            .map(|value| normalize_import_learning_text(Some(&json!(value))))
            .unwrap_or_default(),
    );
    parts.insert(
        "suppression",
        normalize_import_learning_text(Some(&json!(input.suppression_scope))),
    );
    parts.insert(
        "transfer",
        if input.transfer_protected { "1" } else { "0" }.to_string(),
    );
    let canonical = parts
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("|");
    let digest = Sha256::digest(canonical.as_bytes());
    format!("{}:{}", parts["schema"], hex_digest(&digest))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportLearningLifecycleState {
    pub status: String,
    pub accepted_count: i64,
    pub rejected_count: i64,
    pub auto_applied_count: i64,
}

impl Default for ImportLearningLifecycleState {
    fn default() -> Self {
        Self {
            status: LEARNING_LIFECYCLE_STATUS_YELLOW.to_string(),
            accepted_count: 0,
            rejected_count: 0,
            auto_applied_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportLearningLifecycleTransition {
    pub previous_status: String,
    pub next_status: String,
    pub accepted_count: i64,
    pub rejected_count: i64,
    pub auto_applied_count: i64,
    pub event_type: String,
    pub signal_state: String,
    pub auto_apply_enabled: bool,
    pub suppressed: bool,
}

/// 根据用户反馈推进导入学习生命周期，维护 yellow/green/auto_applied/suppressed 状态。
#[tracing::instrument(level = "debug", skip_all)]
pub fn transition_import_learning_lifecycle(
    current: &ImportLearningLifecycleState,
    feedback: &str,
) -> ImportLearningLifecycleTransition {
    let previous_status = normalize_learning_lifecycle_status(&current.status);
    let mut next_status = previous_status.clone();
    let mut accepted_count = current.accepted_count.max(0);
    let mut rejected_count = current.rejected_count.max(0);
    let mut auto_applied_count = current.auto_applied_count.max(0);
    let feedback = feedback.trim().to_ascii_lowercase();
    let mut event_type = feedback.clone();

    match feedback.as_str() {
        "accept" | "accepted" => {
            if previous_status != LEARNING_LIFECYCLE_STATUS_SUPPRESSED {
                accepted_count += 1;
                rejected_count = 0;
                if accepted_count >= LEARNING_LIFECYCLE_ACCEPTS_TO_GREEN {
                    next_status = LEARNING_LIFECYCLE_STATUS_GREEN.to_string();
                } else if matches!(
                    previous_status.as_str(),
                    LEARNING_LIFECYCLE_STATUS_GREEN | LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED
                ) {
                    next_status = previous_status.clone();
                } else {
                    next_status = LEARNING_LIFECYCLE_STATUS_YELLOW.to_string();
                }
            }
            event_type = "accept".to_string();
        }
        "reject" | "rejected" => {
            rejected_count += 1;
            if matches!(
                previous_status.as_str(),
                LEARNING_LIFECYCLE_STATUS_GREEN | LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED
            ) {
                if rejected_count >= LEARNING_LIFECYCLE_GREEN_REJECTS_TO_DOWNGRADE {
                    next_status = LEARNING_LIFECYCLE_STATUS_DOWNGRADED.to_string();
                    event_type = "downgrade".to_string();
                    accepted_count = 0;
                    rejected_count = 0;
                }
            } else if rejected_count >= LEARNING_LIFECYCLE_YELLOW_REJECTS_TO_SUPPRESS {
                accepted_count = 0;
                next_status = LEARNING_LIFECYCLE_STATUS_SUPPRESSED.to_string();
                event_type = "suppress".to_string();
            } else if previous_status != LEARNING_LIFECYCLE_STATUS_SUPPRESSED {
                accepted_count = 0;
                next_status = LEARNING_LIFECYCLE_STATUS_YELLOW.to_string();
            }
            if previous_status == LEARNING_LIFECYCLE_STATUS_SUPPRESSED {
                next_status = previous_status.clone();
            }
        }
        "auto_apply" | "auto-applied" | "auto_applied" => {
            if matches!(
                previous_status.as_str(),
                LEARNING_LIFECYCLE_STATUS_GREEN | LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED
            ) {
                auto_applied_count += 1;
                next_status = LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED.to_string();
            }
            event_type = "auto_apply".to_string();
        }
        "suppress" | "suppressed" => {
            next_status = LEARNING_LIFECYCLE_STATUS_SUPPRESSED.to_string();
            event_type = "suppress".to_string();
        }
        _ => {
            event_type = "feedback".to_string();
        }
    }

    let signal_state = learning_lifecycle_signal_state(&next_status).to_string();
    ImportLearningLifecycleTransition {
        previous_status,
        next_status: next_status.clone(),
        accepted_count,
        rejected_count,
        auto_applied_count,
        event_type,
        auto_apply_enabled: learning_lifecycle_is_auto_eligible(&next_status),
        suppressed: next_status == LEARNING_LIFECYCLE_STATUS_SUPPRESSED,
        signal_state,
    }
}

/// 归一化学习生命周期状态，未知值回退 yellow，避免前端/数据库状态漂移。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_learning_lifecycle_status(status: &str) -> String {
    match status.trim().to_ascii_lowercase().as_str() {
        "green" => LEARNING_LIFECYCLE_STATUS_GREEN,
        "auto_applied" | "auto-applied" => LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED,
        "downgraded" => LEARNING_LIFECYCLE_STATUS_DOWNGRADED,
        "suppressed" => LEARNING_LIFECYCLE_STATUS_SUPPRESSED,
        _ => LEARNING_LIFECYCLE_STATUS_YELLOW,
    }
    .to_string()
}

/// 判断当前生命周期状态是否允许自动应用建议。
pub fn learning_lifecycle_is_auto_eligible(status: &str) -> bool {
    matches!(
        normalize_learning_lifecycle_status(status).as_str(),
        LEARNING_LIFECYCLE_STATUS_GREEN | LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED
    )
}

/// 将生命周期状态投影为前端信号灯状态。
pub fn learning_lifecycle_signal_state(status: &str) -> &'static str {
    match normalize_learning_lifecycle_status(status).as_str() {
        LEARNING_LIFECYCLE_STATUS_GREEN | LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED => "green",
        LEARNING_LIFECYCLE_STATUS_SUPPRESSED => "suppressed",
        _ => "yellow",
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
