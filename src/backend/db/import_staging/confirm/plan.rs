#[derive(Debug, Clone)]
struct HistoryConfirmPlan {
    preview_id: i64,
    operation: bill_analyser_core::ImportHistoryRewriteOperation,
    operation_id: String,
    history_bill_id: i64,
    history_bill_version: i64,
    acknowledgement_token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmPlanValidationKind {
    UnknownSignalState,
    HistoryAcknowledgement,
    Identity,
    ReviewState,
}

impl ConfirmPlanValidationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::UnknownSignalState => "unknown_signal_state",
            Self::HistoryAcknowledgement => "history_acknowledgement",
            Self::Identity => "identity",
            Self::ReviewState => "review_state",
        }
    }
}

#[derive(Debug)]
struct ConfirmPlanBuildError {
    validation_kind: ConfirmPlanValidationKind,
    validation_error_count: Option<usize>,
    error: DbError,
}

impl ConfirmPlanBuildError {
    fn new(validation_kind: ConfirmPlanValidationKind, error: DbError) -> Self {
        Self {
            validation_kind,
            validation_error_count: None,
            error,
        }
    }

    fn with_count(
        validation_kind: ConfirmPlanValidationKind,
        validation_error_count: usize,
        error: DbError,
    ) -> Self {
        Self {
            validation_kind,
            validation_error_count: Some(validation_error_count),
            error,
        }
    }

    fn validation_kind(&self) -> &'static str {
        self.validation_kind.as_str()
    }

    fn validation_error_count(&self) -> Option<usize> {
        self.validation_error_count
    }

    fn into_db_error(self) -> DbError {
        self.error
    }
}

#[derive(Debug)]
struct PreparedConfirmPlan {
    previews: Vec<ImportPreviewRow>,
    history_plans: Vec<HistoryConfirmPlan>,
    history_preview_ids: BTreeSet<i64>,
}

#[derive(Debug)]
struct PlannedHistoryWrite {
    preview: ImportPreviewRow,
    plan: HistoryConfirmPlan,
}

#[derive(Debug)]
struct ConfirmPlan {
    history_writes: Vec<PlannedHistoryWrite>,
    bill_drafts: Vec<BillCreateDraft>,
    result: ConfirmPreviewResult,
}

fn prepare_confirm_plan(
    session_id: &str,
    previews: Vec<ImportPreviewRow>,
    acknowledgement: Option<&ImportHistoryRewriteAcknowledgement>,
) -> Result<PreparedConfirmPlan, ConfirmPlanBuildError> {
    let unknown_state_errors = previews
        .iter()
        .filter(|preview| {
            import_preview_matching_feedback_has_unknown_signal_status(
                &preview.preview_matching_feedback,
            )
        })
        .map(|preview| format!("preview {} has an unknown signal state", preview.id))
        .collect::<Vec<_>>();
    if !unknown_state_errors.is_empty() {
        return Err(ConfirmPlanBuildError::with_count(
            ConfirmPlanValidationKind::UnknownSignalState,
            unknown_state_errors.len(),
            DbError::InvalidOperation(format!(
                "import preview requires review: {}",
                unknown_state_errors.join("; ")
            )),
        ));
    }

    let history_plans = validate_history_acknowledgement(session_id, &previews, acknowledgement)
        .map_err(|error| {
            ConfirmPlanBuildError::new(ConfirmPlanValidationKind::HistoryAcknowledgement, error)
        })?;
    let history_preview_ids = history_plans
        .iter()
        .map(|plan| plan.preview_id)
        .collect::<BTreeSet<_>>();
    Ok(PreparedConfirmPlan {
        previews,
        history_plans,
        history_preview_ids,
    })
}

fn build_confirm_plan(
    prepared: PreparedConfirmPlan,
    identity_maps: &ImportIdentityMaps,
) -> Result<ConfirmPlan, ConfirmPlanBuildError> {
    let identity_errors = prepared
        .previews
        .iter()
        .flat_map(|preview| preview_identity_error_messages(preview, identity_maps))
        .collect::<Vec<_>>();
    if !identity_errors.is_empty() {
        return Err(ConfirmPlanBuildError::with_count(
            ConfirmPlanValidationKind::Identity,
            identity_errors.len(),
            DbError::InvalidOperation(format!(
                "import preview identity validation failed: {}",
                identity_errors.join("; ")
            )),
        ));
    }

    let review_errors = prepared
        .previews
        .iter()
        .filter(|preview| {
            preview_requires_review(preview)
                && !prepared.history_preview_ids.contains(&preview.id)
        })
        .map(|preview| format!("preview {} requires review before confirm", preview.id))
        .collect::<Vec<_>>();
    if !review_errors.is_empty() {
        return Err(ConfirmPlanBuildError::with_count(
            ConfirmPlanValidationKind::ReviewState,
            review_errors.len(),
            DbError::InvalidOperation(format!(
                "import preview requires review: {}",
                review_errors.join("; ")
            )),
        ));
    }

    let mut history_plans = prepared
        .history_plans
        .into_iter()
        .map(|plan| (plan.preview_id, plan))
        .collect::<BTreeMap<_, _>>();
    let mut history_writes = Vec::with_capacity(history_plans.len());
    let mut bill_drafts = Vec::with_capacity(
        prepared
            .previews
            .len()
            .saturating_sub(history_plans.len()),
    );
    for preview in prepared.previews {
        if let Some(plan) = history_plans.remove(&preview.id) {
            history_writes.push(PlannedHistoryWrite { preview, plan });
        } else {
            bill_drafts.push(BillCreateDraft {
                fields: bill_create_fields_from_preview(&preview),
                tag_ids: Vec::new(),
            });
        }
    }
    if !history_plans.is_empty() {
        return Err(ConfirmPlanBuildError::new(
            ConfirmPlanValidationKind::HistoryAcknowledgement,
            DbError::InvalidOperation(
                "history acknowledgement preview not selected".to_string(),
            ),
        ));
    }

    let result = ConfirmPreviewResult {
        confirmed_count: bill_drafts.len() + history_writes.len(),
        skipped_count: 0,
        duplicate_count: 0,
        errors: Vec::new(),
    };
    Ok(ConfirmPlan {
        history_writes,
        bill_drafts,
        result,
    })
}
