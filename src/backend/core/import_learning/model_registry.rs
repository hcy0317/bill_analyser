use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportLearningDatasetSnapshotPayload {
    pub feature_schema_version: String,
    pub policy_version: String,
    pub sample_ids: Vec<i64>,
    pub semantic_label_counts: BTreeMap<String, i64>,
    pub joint_label_confirmation_counts: BTreeMap<String, i64>,
}

/// 构建训练数据快照的持久化载荷，固定记录特征版本、策略版本和标签分布。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_dataset_snapshot_payload(
    samples: &[ImportLearningTrainingSample],
) -> ImportLearningDatasetSnapshotPayload {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_dataset_snapshot_payload",
        "business operation entered"
    );
    ImportLearningDatasetSnapshotPayload {
        feature_schema_version: FEATURE_SCHEMA_VERSION.to_string(),
        policy_version: POLICY_VERSION.to_string(),
        sample_ids: samples.iter().map(|sample| sample.sample_id).collect(),
        semantic_label_counts: build_semantic_label_counts(samples),
        joint_label_confirmation_counts: build_label_confirmation_counts(samples),
    }
}

/// 将数据快照编号转换为模型版本号，保证模型登记表与快照记录可追溯。
#[tracing::instrument(level = "debug", skip_all)]
pub fn import_learning_model_version(dataset_snapshot_id: i64) -> String {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_learning_model_version",
        "business operation entered"
    );
    format!("v{dataset_snapshot_id}")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportLearningModelRegistryPayload {
    pub feature_schema_version: String,
    pub policy_version: String,
    pub parameter_ref: String,
    pub model_parameters: Value,
    pub training_metrics: Value,
    pub joint_label_confirmation_counts: BTreeMap<String, i64>,
}

/// 汇总模型参数、训练指标和确认计数，生成模型登记表使用的业务载荷。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_model_registry_payload(
    model_parameters: Value,
    training_metrics: Value,
    confirmation_counts: BTreeMap<String, i64>,
) -> ImportLearningModelRegistryPayload {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_model_registry_payload",
        "business operation entered"
    );
    ImportLearningModelRegistryPayload {
        feature_schema_version: FEATURE_SCHEMA_VERSION.to_string(),
        policy_version: POLICY_VERSION.to_string(),
        parameter_ref: "metrics_json.model_parameters".to_string(),
        model_parameters,
        training_metrics,
        joint_label_confirmation_counts: confirmation_counts,
    }
}
