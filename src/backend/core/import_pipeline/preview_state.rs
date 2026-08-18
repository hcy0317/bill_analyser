pub const PREVIEW_STATE_PROJECTION_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreviewSignalStatus {
    #[default]
    Absent,
    Pending,
    Accepted,
    Rejected,
    Skipped,
    AutoApplied,
    NeedsReview,
    Suppressed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreviewLearningLevel {
    #[default]
    None,
    Yellow,
    Green,
    Blue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PreviewFamilyEvidence {
    pub status: PreviewSignalStatus,
    pub has_evidence: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PreviewIdentityInput {
    pub category_required: bool,
    pub category_id: Option<i64>,
    pub category_valid: bool,
    pub source_account_required: bool,
    pub source_account_id: Option<i64>,
    pub source_account_valid: bool,
    pub destination_account_required: bool,
    pub destination_account_id: Option<i64>,
    pub destination_account_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PreviewStateInput {
    pub parser_present: bool,
    pub platform_duplicate: bool,
    pub transfer: PreviewFamilyEvidence,
    pub transfer_learning_level: Option<PreviewLearningLevel>,
    pub history: PreviewFamilyEvidence,
    pub learning: PreviewFamilyEvidence,
    pub llm: PreviewFamilyEvidence,
    pub identity: PreviewIdentityInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct SignalSet {
    families: BTreeSet<ImportPreviewSignalFamily>,
}

impl SignalSet {
    pub fn contains(&self, family: ImportPreviewSignalFamily) -> bool {
        self.families.contains(&family)
    }

    pub fn names(&self) -> Vec<&'static str> {
        ImportPreviewSignalFamily::ORDER
            .into_iter()
            .filter(|family| self.contains(*family))
            .map(ImportPreviewSignalFamily::as_str)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewIssueCode {
    MissingCategory,
    InvalidCategory,
    MissingSourceAccount,
    InvalidSourceAccount,
    MissingDestinationAccount,
    InvalidDestinationAccount,
    SameTransferAccounts,
    UnknownTransferState,
    UnknownHistoryState,
    UnknownLearningState,
    UnknownLlmState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct ReviewIssueSet {
    codes: BTreeSet<ReviewIssueCode>,
}

impl ReviewIssueSet {
    pub fn contains(&self, issue: ReviewIssueCode) -> bool {
        self.codes.contains(&issue)
    }

    pub fn codes(&self) -> Vec<ReviewIssueCode> {
        self.codes.iter().copied().collect()
    }

    pub fn is_blocking(&self) -> bool {
        !self.codes.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PreviewDecisionStateView {
    pub transfer: PreviewFamilyEvidence,
    pub history: PreviewFamilyEvidence,
    pub learning: PreviewFamilyEvidence,
    pub llm: PreviewFamilyEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EffectivePreviewFields {
    pub category_id: Option<i64>,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewState {
    pub projection_version: u16,
    pub signals: SignalSet,
    pub issues: ReviewIssueSet,
    pub decisions: PreviewDecisionStateView,
    pub effective: EffectivePreviewFields,
}

impl PreviewState {
    pub fn is_confirmable(&self) -> bool {
        !self.issues.is_blocking()
    }
}

pub struct PreviewStateKernel;

impl PreviewStateKernel {
    pub fn derive(input: PreviewStateInput) -> PreviewState {
        let mut families = BTreeSet::new();
        if input.platform_duplicate {
            families.insert(ImportPreviewSignalFamily::PlatformDuplicate);
        }
        if signal_is_transfer_visible(input.transfer) {
            families.insert(ImportPreviewSignalFamily::Transfer);
        }
        if signal_is_history_visible(input.history) {
            families.insert(ImportPreviewSignalFamily::History);
        }
        if signal_is_learning_visible(input.learning)
            || transfer_owns_learning_membership(&input)
        {
            families.insert(ImportPreviewSignalFamily::Learning);
        }
        if signal_is_learning_visible(input.llm) {
            families.insert(ImportPreviewSignalFamily::Llm);
        }
        if input.parser_present && families.is_empty() {
            families.insert(ImportPreviewSignalFamily::Parser);
        }

        let mut issues = BTreeSet::new();
        derive_identity_issue(
            input.identity.category_required,
            input.identity.category_id,
            input.identity.category_valid,
            ReviewIssueCode::MissingCategory,
            ReviewIssueCode::InvalidCategory,
            &mut issues,
        );
        derive_identity_issue(
            input.identity.source_account_required,
            input.identity.source_account_id,
            input.identity.source_account_valid,
            ReviewIssueCode::MissingSourceAccount,
            ReviewIssueCode::InvalidSourceAccount,
            &mut issues,
        );
        derive_identity_issue(
            input.identity.destination_account_required,
            input.identity.destination_account_id,
            input.identity.destination_account_valid,
            ReviewIssueCode::MissingDestinationAccount,
            ReviewIssueCode::InvalidDestinationAccount,
            &mut issues,
        );
        for (evidence, issue) in [
            (input.transfer, ReviewIssueCode::UnknownTransferState),
            (input.history, ReviewIssueCode::UnknownHistoryState),
            (input.learning, ReviewIssueCode::UnknownLearningState),
            (input.llm, ReviewIssueCode::UnknownLlmState),
        ] {
            if evidence.status == PreviewSignalStatus::Unknown {
                issues.insert(issue);
            }
        }

        let mut effective = EffectivePreviewFields {
            category_id: effective_identity(
                input.identity.category_id,
                input.identity.category_valid,
            ),
            source_account_id: effective_identity(
                input.identity.source_account_id,
                input.identity.source_account_valid,
            ),
            destination_account_id: effective_identity(
                input.identity.destination_account_id,
                input.identity.destination_account_valid,
            ),
        };
        if input.identity.destination_account_required
            && effective.source_account_id.is_some()
            && effective.source_account_id == effective.destination_account_id
        {
            issues.insert(ReviewIssueCode::SameTransferAccounts);
            effective.destination_account_id = None;
        }

        PreviewState {
            projection_version: PREVIEW_STATE_PROJECTION_VERSION,
            signals: SignalSet { families },
            issues: ReviewIssueSet { codes: issues },
            decisions: PreviewDecisionStateView {
                transfer: input.transfer,
                history: input.history,
                learning: input.learning,
                llm: input.llm,
            },
            effective,
        }
    }
}

fn signal_is_transfer_visible(evidence: PreviewFamilyEvidence) -> bool {
    matches!(
        evidence.status,
        PreviewSignalStatus::Pending
            | PreviewSignalStatus::Accepted
            | PreviewSignalStatus::AutoApplied
    )
}

fn signal_is_history_visible(evidence: PreviewFamilyEvidence) -> bool {
    evidence.has_evidence
        && !matches!(
            evidence.status,
            PreviewSignalStatus::Absent
                | PreviewSignalStatus::Suppressed
                | PreviewSignalStatus::Unknown
        )
}

fn signal_is_learning_visible(evidence: PreviewFamilyEvidence) -> bool {
    match evidence.status {
        PreviewSignalStatus::Pending | PreviewSignalStatus::NeedsReview => evidence.has_evidence,
        PreviewSignalStatus::Accepted
        | PreviewSignalStatus::Rejected
        | PreviewSignalStatus::Skipped
        | PreviewSignalStatus::AutoApplied => true,
        PreviewSignalStatus::Absent
        | PreviewSignalStatus::Suppressed
        | PreviewSignalStatus::Unknown => false,
    }
}

fn transfer_owns_learning_membership(input: &PreviewStateInput) -> bool {
    signal_is_transfer_visible(input.transfer)
        && matches!(
            input.transfer_learning_level,
            Some(
                PreviewLearningLevel::Yellow
                    | PreviewLearningLevel::Green
                    | PreviewLearningLevel::Blue
            )
        )
}

fn derive_identity_issue(
    required: bool,
    id: Option<i64>,
    valid: bool,
    missing_issue: ReviewIssueCode,
    invalid_issue: ReviewIssueCode,
    issues: &mut BTreeSet<ReviewIssueCode>,
) {
    if required && id.is_none() {
        issues.insert(missing_issue);
    } else if id.is_some() && !valid {
        issues.insert(invalid_issue);
    }
}

fn effective_identity(id: Option<i64>, valid: bool) -> Option<i64> {
    id.filter(|id| *id > 0 && valid)
}

/// Attach the versioned preview-state snapshot to an API-ready canonical row.
///
/// Staging identity validation has already normalized the row and recorded any
/// rejected identity in `matching.identity_validation`. The response projector
/// consumes that evidence instead of querying taxonomy tables for every row.
pub fn attach_import_preview_state_snapshot_to_canonical_row(
    preview_item: &mut Map<String, Value>,
) {
    let snapshot = derive_import_preview_state_snapshot_from_canonical_row(preview_item);
    preview_item.insert(
        "preview_state".to_string(),
        serde_json::to_value(snapshot).unwrap_or_else(|_| json!({})),
    );
}

pub fn derive_import_preview_state_snapshot_from_canonical_row(
    preview_item: &Map<String, Value>,
) -> PreviewState {
    let identity = canonical_row_identity_input(preview_item);
    derive_preview_state_from_legacy_row_with_identity(preview_item, identity)
}

fn derive_preview_state_from_legacy_row(
    preview_item: &Map<String, Value>,
    categories_by_id: &BTreeMap<i64, CategoryLookup>,
    accounts_by_id: &BTreeMap<i64, AccountLookup>,
) -> PreviewState {
    let preview_type = string_field_from_map(preview_item, "preview_type");
    let category_id = integer_lookup_key(preview_item.get("category_id"));
    let source_account_id = integer_lookup_key(preview_item.get("preview_source_account_id"));
    let destination_account_id =
        integer_lookup_key(preview_item.get("preview_destination_account_id"));

    derive_preview_state_from_legacy_row_with_identity(
        preview_item,
        PreviewIdentityInput {
            category_required: preview_category_required_for_state(&preview_type),
            category_id,
            category_valid: category_id.is_some_and(|id| categories_by_id.contains_key(&id)),
            source_account_required: true,
            source_account_id,
            source_account_valid: source_account_id
                .is_some_and(|id| accounts_by_id.contains_key(&id)),
            destination_account_required: preview_destination_account_required_for_state(
                &preview_type,
            ),
            destination_account_id,
            destination_account_valid: destination_account_id
                .is_some_and(|id| accounts_by_id.contains_key(&id)),
        },
    )
}

fn derive_preview_state_from_legacy_row_with_identity(
    preview_item: &Map<String, Value>,
    identity: PreviewIdentityInput,
) -> PreviewState {
    let matching = preview_matching_payload(preview_item);
    let parser_present = !string_field_from_map(preview_item, "preview_parser_id")
        .trim()
        .is_empty()
        || !list_field_from_map(preview_item, "preview_parser_tags").is_empty()
        || matching
            .and_then(|matching| object_field(matching.get("parser")))
            .is_some_and(parser_section_has_evidence);
    let dedup_type = get_first_non_empty_string([
        string_field_from_map(preview_item, "dedup_type"),
        matching
            .and_then(|matching| object_field(matching.get("dedup")))
            .map(|dedup| string_field_from_map(dedup, "type"))
            .unwrap_or_default(),
    ]);
    let transfer = legacy_family_evidence(preview_item, "transfer");
    let history = legacy_history_evidence(preview_item);
    let learning = legacy_family_evidence(preview_item, "learning");
    let llm = legacy_family_evidence(preview_item, "llm");
    let transfer_learning_level = matching
        .and_then(|matching| object_field(matching.get("transfer")))
        .map(|transfer| {
            get_first_non_empty_string([
                string_field_from_map(transfer, "learning_level"),
                string_field_from_map(transfer, "level"),
            ])
        })
        .and_then(|level| parse_preview_learning_level(&level));

    PreviewStateKernel::derive(PreviewStateInput {
        parser_present,
        platform_duplicate: dedup_type.trim().eq_ignore_ascii_case("platform_bank"),
        transfer,
        transfer_learning_level,
        history,
        learning,
        llm,
        identity,
    })
}

fn canonical_row_identity_input(preview_item: &Map<String, Value>) -> PreviewIdentityInput {
    let preview_type = string_field_from_map(preview_item, "preview_type");
    let (category_id, category_required, category_valid) =
        canonical_identity_field(preview_item, "category_id", "category_id");
    let (source_account_id, _, source_account_valid) =
        canonical_identity_field(
            preview_item,
            "preview_source_account_id",
            "source_account_id",
        );
    let (destination_account_id, destination_account_required, destination_account_valid) =
        canonical_identity_field(
            preview_item,
            "preview_destination_account_id",
            "destination_account_id",
        );

    PreviewIdentityInput {
        category_required: category_required
            || preview_category_required_for_state(&preview_type),
        category_id,
        category_valid,
        source_account_required: true,
        source_account_id,
        source_account_valid,
        destination_account_required: destination_account_required
            || preview_destination_account_required_for_state(&preview_type),
        destination_account_id,
        destination_account_valid,
    }
}

fn canonical_identity_field(
    preview_item: &Map<String, Value>,
    row_key: &str,
    issue_field: &str,
) -> (Option<i64>, bool, bool) {
    let current_id = integer_lookup_key(preview_item.get(row_key));
    let issue = preview_matching_payload(preview_item)
        .and_then(|matching| object_field(matching.get("identity_validation")))
        .and_then(|validation| validation.get("issues"))
        .and_then(Value::as_array)
        .and_then(|issues| {
            issues.iter().find_map(|issue| {
                let issue = issue.as_object()?;
                (string_field_from_map(issue, "field") == issue_field).then_some(issue)
            })
        });

    let Some(issue) = issue else {
        return (
            current_id,
            current_id.is_some(),
            current_id.is_some_and(|id| id > 0),
        );
    };
    let reason = string_field_from_map(issue, "reason");
    if reason == "missing" {
        return (None, true, false);
    }
    let rejected_id = current_id
        .or_else(|| integer_lookup_key(issue.get("value")))
        .or(Some(0));
    if reason == "same_as_source_account" {
        return (rejected_id, true, true);
    }
    (rejected_id, true, false)
}

fn parser_section_has_evidence(section: &Map<String, Value>) -> bool {
    !get_first_non_empty_string([
        string_field_from_map(section, "id"),
        string_field_from_map(section, "parser_id"),
    ])
    .is_empty()
        || section
            .get("tags")
            .or_else(|| section.get("parser_tags"))
            .is_some_and(|tags| !parse_dedup_source_ids(Some(tags)).is_empty())
}

fn legacy_family_evidence(
    preview_item: &Map<String, Value>,
    family: &str,
) -> PreviewFamilyEvidence {
    let section = preview_matching_payload(preview_item)
        .and_then(|matching| object_field(matching.get(family)));
    if section.is_some_and(matching_section_is_suppressed_or_none) {
        return PreviewFamilyEvidence {
            status: PreviewSignalStatus::Suppressed,
            has_evidence: false,
        };
    }
    let raw_status = section.map(resolve_first_nonempty_status).unwrap_or_default();
    let resolved_status = match family {
        "transfer" => resolve_import_preview_transfer_signal_status(preview_item),
        "learning" => resolve_import_preview_learning_signal_status(preview_item),
        "llm" => resolve_import_preview_llm_signal_status(preview_item),
        _ => None,
    };
    let status = if raw_status.is_empty() {
        resolved_status
            .as_deref()
            .map(parse_preview_signal_status)
            .unwrap_or(PreviewSignalStatus::Absent)
    } else {
        parse_family_signal_status(family, &raw_status)
    };

    PreviewFamilyEvidence {
        status,
        has_evidence: resolved_status.is_some(),
    }
}

fn legacy_history_evidence(preview_item: &Map<String, Value>) -> PreviewFamilyEvidence {
    let matching = build_import_preview_matching_payload(preview_item);
    let section = matching
        .as_object()
        .and_then(|matching| object_field(matching.get("reconciliation")));
    let raw_status = section.map(resolve_first_nonempty_status).unwrap_or_default();
    let has_evidence = section.is_some_and(|section| {
        section
            .get("planned_operation")
            .and_then(Value::as_str)
            .and_then(normalize_history_operation)
            .is_some()
            || section
                .get("destructive_ack_required")
                .map(import_preview_signal_value_is_truthy)
                .unwrap_or(false)
    });
    let status = if section.is_some_and(matching_section_is_suppressed_or_none) {
        PreviewSignalStatus::Suppressed
    } else if raw_status.is_empty() && has_evidence {
        PreviewSignalStatus::Pending
    } else if raw_status.is_empty() {
        PreviewSignalStatus::Absent
    } else {
        parse_family_signal_status("history", &raw_status)
    };

    PreviewFamilyEvidence {
        status,
        has_evidence,
    }
}

fn parse_family_signal_status(family: &str, status: &str) -> PreviewSignalStatus {
    let parsed = parse_preview_signal_status(status);
    if parsed == PreviewSignalStatus::NeedsReview && family != "learning" {
        PreviewSignalStatus::Unknown
    } else {
        parsed
    }
}

fn parse_preview_signal_status(status: &str) -> PreviewSignalStatus {
    match trim_import_preview_signal_text(status)
        .to_ascii_lowercase()
        .as_str()
    {
        "" => PreviewSignalStatus::Absent,
        "pending" => PreviewSignalStatus::Pending,
        "accepted" => PreviewSignalStatus::Accepted,
        "rejected" => PreviewSignalStatus::Rejected,
        "skipped" => PreviewSignalStatus::Skipped,
        "auto_applied" | "auto-applied" => PreviewSignalStatus::AutoApplied,
        "needs_review" => PreviewSignalStatus::NeedsReview,
        "none" | "suppressed" => PreviewSignalStatus::Suppressed,
        _ => PreviewSignalStatus::Unknown,
    }
}

fn parse_preview_learning_level(level: &str) -> Option<PreviewLearningLevel> {
    match trim_import_preview_signal_text(level)
        .to_ascii_lowercase()
        .as_str()
    {
        "yellow" => Some(PreviewLearningLevel::Yellow),
        "green" => Some(PreviewLearningLevel::Green),
        "blue" => Some(PreviewLearningLevel::Blue),
        _ => None,
    }
}

fn preview_category_required_for_state(preview_type: &str) -> bool {
    matches!(
        preview_type.trim().to_ascii_lowercase().as_str(),
        "收入"
            | "income"
            | "2"
            | "支出"
            | "expense"
            | "3"
            | "转账"
            | "transfer"
            | "4"
            | "投资"
            | "investment"
            | "5"
    )
}

fn preview_destination_account_required_for_state(preview_type: &str) -> bool {
    matches!(
        preview_type.trim().to_ascii_lowercase().as_str(),
        "转账" | "transfer" | "4" | "投资" | "investment" | "5"
    )
}

fn compatibility_signal_status(evidence: PreviewFamilyEvidence) -> Option<String> {
    match evidence.status {
        PreviewSignalStatus::Pending | PreviewSignalStatus::NeedsReview if evidence.has_evidence => {
            Some("pending".to_string())
        }
        PreviewSignalStatus::Accepted | PreviewSignalStatus::AutoApplied => {
            Some("accepted".to_string())
        }
        PreviewSignalStatus::Rejected => Some("rejected".to_string()),
        PreviewSignalStatus::Skipped => Some("skipped".to_string()),
        PreviewSignalStatus::Pending | PreviewSignalStatus::NeedsReview => None,
        PreviewSignalStatus::Absent
        | PreviewSignalStatus::Suppressed
        | PreviewSignalStatus::Unknown => None,
    }
}
