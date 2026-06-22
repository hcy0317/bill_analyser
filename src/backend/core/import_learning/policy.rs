use super::*;

/// 按语义标签与路由标签联合键统计人工确认次数，供学习策略判断自动应用门槛。
pub fn build_label_confirmation_counts(
    samples: &[ImportLearningTrainingSample],
) -> BTreeMap<String, i64> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_label_confirmation_counts",
        "business operation entered"
    );
    let mut counts = BTreeMap::new();
    for sample in samples {
        let key = format!("{}||{}", sample.semantic_label, sample.route_label);
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

/// 统计语义标签出现次数，用于训练数据快照和模型诊断展示。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_semantic_label_counts(
    samples: &[ImportLearningTrainingSample],
) -> BTreeMap<String, i64> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_semantic_label_counts",
        "business operation entered"
    );
    let mut counts = BTreeMap::new();
    for sample in samples {
        *counts.entry(sample.semantic_label.clone()).or_insert(0) += 1;
    }
    counts
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportLearningPrediction {
    pub semantic_label: String,
    pub route_label: String,
    pub semantic_confidence: f64,
    pub route_confidence: f64,
    pub semantic_margin: f64,
    pub route_margin: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningPolicyDecision {
    pub mode: String,
    pub score: f64,
    pub confidence: f64,
    pub margin: f64,
    pub auto_apply: bool,
    pub level: String,
    pub rejection_reasons: Vec<String>,
}

/// 根据预测置信度、边际、确认次数和冲突原因计算导入学习的 green/blue 策略决策。
#[tracing::instrument(level = "debug", skip_all)]
pub fn evaluate_learning_policy(
    prediction: &ImportLearningPrediction,
    confirmation_count: i64,
    conflict_reasons: &[String],
) -> LearningPolicyDecision {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "evaluate_learning_policy",
        "business operation entered"
    );
    let confidence = prediction
        .semantic_confidence
        .min(prediction.route_confidence);
    let margin = prediction.semantic_margin.min(prediction.route_margin);
    let score = round4((prediction.semantic_confidence + prediction.route_confidence) / 2.0);
    let mut rejection_reasons = Vec::new();

    if confidence < GREEN_CONFIDENCE_THRESHOLD {
        rejection_reasons.push("green_confidence".to_string());
    }
    if margin < GREEN_MARGIN_THRESHOLD {
        rejection_reasons.push("green_margin".to_string());
    }
    if !rejection_reasons.is_empty() {
        return LearningPolicyDecision {
            mode: "none".to_string(),
            score,
            confidence: round4(confidence),
            margin: round4(margin),
            auto_apply: false,
            level: String::new(),
            rejection_reasons,
        };
    }

    let mut blue_rejection_reasons = Vec::new();
    if confirmation_count <= BLUE_ACCEPT_CONFIRMATION_THRESHOLD {
        blue_rejection_reasons.push("confirmations".to_string());
    }
    if confidence < BLUE_CONFIDENCE_THRESHOLD {
        blue_rejection_reasons.push("blue_confidence".to_string());
    }
    if margin < BLUE_MARGIN_THRESHOLD {
        blue_rejection_reasons.push("blue_margin".to_string());
    }
    blue_rejection_reasons.extend(
        conflict_reasons
            .iter()
            .filter(|reason| !reason.is_empty())
            .map(|reason| format!("conflict:{reason}")),
    );

    let mode = if blue_rejection_reasons.is_empty() {
        "blue"
    } else {
        "green"
    };
    let level = if score >= 0.86 {
        "high"
    } else if score >= 0.72 {
        "medium"
    } else {
        "low"
    };
    LearningPolicyDecision {
        mode: mode.to_string(),
        score,
        confidence: round4(confidence),
        margin: round4(margin),
        auto_apply: mode == "blue",
        level: level.to_string(),
        rejection_reasons: blue_rejection_reasons,
    }
}
