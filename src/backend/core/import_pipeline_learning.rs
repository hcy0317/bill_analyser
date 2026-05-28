// 中文导读：导入预览 learning matching payload。
// 维护重点：生命周期字段会随推荐策略演进，保持独立避免 import_pipeline 主合同继续膨胀。
// 不变式：该结构仍通过 ImportPreviewMatchingPayload.learning 对外序列化，字段默认值需保持前端兼容。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LearningMatchingPayload {
    pub rule_id: Option<i64>,
    pub score: f64,
    pub level: String,
    pub reason: String,
    pub recommended_type: String,
    pub summary: String,
    pub review_status: String,
    pub suppressed: bool,
    pub recommendation_key: String,
    pub lifecycle_status: String,
    pub signal_state: String,
    pub accepted_count: i64,
    pub rejected_count: i64,
    pub auto_applied_count: i64,
    pub auto_apply: bool,
}
