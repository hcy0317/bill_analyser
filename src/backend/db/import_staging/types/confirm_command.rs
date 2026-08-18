/// Canonical confirm request passed from the HTTP boundary to the single DB transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmCommand {
    pub session_id: String,
    pub expected_session_version: Option<i64>,
    pub preview_patches: Vec<ImportPreviewPatch>,
    pub selected_preview_ids: Option<Vec<i64>>,
    pub preserve_unpatched_selection: bool,
    pub history_acknowledgement: Option<ImportHistoryRewriteAcknowledgement>,
    pub declared_confirm_time_effects: Vec<ConfirmTimeEffect>,
}

/// Authority used only when replaying a terminal confirm receipt.
///
/// `Metadata` remains the safe default until a target database has completed the typed receipt
/// backfill and all-target parity audit. `TypedV1` is deliberately strict and never falls back to
/// metadata when its projection is missing or invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmReceiptReadSource {
    Metadata,
    TypedV1,
}

impl ConfirmReceiptReadSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::TypedV1 => "typed_v1",
        }
    }
}

/// Confirm-time effects intentionally have no variants until a persisted domain effect is supported.
///
/// Keeping the collection typed prevents arbitrary JSON effects from crossing the repository boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmTimeEffect {}

/// Durable response returned by both the first confirm and same-command replay.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmReceiptResponse {
    pub http_status: u16,
    pub success_envelope: Value,
    pub replayed: bool,
}

#[cfg(test)]
mod confirm_command_contract_tests {
    use super::*;

    #[test]
    fn confirm_command_preserves_absent_selection_and_optional_version() {
        let command = ConfirmCommand {
            session_id: "session-1".to_string(),
            expected_session_version: None,
            preview_patches: Vec::new(),
            selected_preview_ids: None,
            preserve_unpatched_selection: false,
            history_acknowledgement: None,
            declared_confirm_time_effects: Vec::new(),
        };

        assert!(command.expected_session_version.is_none());
        assert!(command.selected_preview_ids.is_none());
        assert!(command.declared_confirm_time_effects.is_empty());
    }
}
