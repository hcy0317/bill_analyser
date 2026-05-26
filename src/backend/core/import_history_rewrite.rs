use sha2::{Digest, Sha256};

#[rustfmt::skip]
pub const IMPORT_STAGING_TABLES: &[&str] = &["import_sessions", "import_sources", "import_standard_rows", "import_decision_groups", "import_decision_group_members", "import_history_materializations", "import_confirm_operations", "bills_parser_template", "bills_preview"];
pub const HISTORY_REWRITE_NOTICE: &str = "将改写/合并历史账单";
const HISTORY_REWRITE_ACK_VERSION: &str = "import-history-rewrite-v1";

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_import_history_rewrite_operation_id(
    planned_operation: &str,
    history_bill_id: i64,
    history_bill_version: i64,
    group_key: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(HISTORY_REWRITE_ACK_VERSION.as_bytes());
    hasher.update(b"|operation|");
    hasher.update(planned_operation.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(history_bill_id.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(history_bill_version.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(group_key.trim().as_bytes());
    let digest = hasher.finalize();
    format!("history:{}", hex_digest(&digest))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_import_history_rewrite_ack_token(
    session_id: &str,
    operation_id: &str,
    planned_operation: &str,
    history_bill_id: i64,
    history_bill_version: i64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(HISTORY_REWRITE_ACK_VERSION.as_bytes());
    hasher.update(b"|ack|");
    hasher.update(session_id.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(operation_id.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(planned_operation.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(history_bill_id.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(history_bill_version.to_string().as_bytes());
    let digest = hasher.finalize();
    hex_digest(&digest)
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn populate_history_rewrite_confirmation_section(
    payload_object: &mut Map<String, Value>,
    preview_item: &Map<String, Value>,
) {
    let Some(reconciliation) = payload_object
        .get("reconciliation")
        .and_then(Value::as_object)
        .cloned()
    else {
        return;
    };
    let planned_operation = string_field_from_map(&reconciliation, "planned_operation");
    if !matches!(
        planned_operation.as_str(),
        "update_history" | "merge_transfer_history"
    ) {
        return;
    }
    let history_bill_id = reconciliation
        .get("history_bill_id")
        .map(value_to_i64)
        .unwrap_or_default();
    let history_bill_version = reconciliation
        .get("history_bill_version")
        .map(value_to_i64)
        .unwrap_or(1)
        .max(1);
    if history_bill_id <= 0 {
        return;
    }
    let group_key = string_field_from_map(&reconciliation, "group_key");
    let operation_id = reconciliation
        .get("operation_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            build_import_history_rewrite_operation_id(
                &planned_operation,
                history_bill_id,
                history_bill_version,
                &group_key,
            )
        });
    let session_id = string_field_from_map(preview_item, "session_id");
    let ack_token = build_import_history_rewrite_ack_token(
        &session_id,
        &operation_id,
        &planned_operation,
        history_bill_id,
        history_bill_version,
    );

    let reconciliation_target = section_object_mut(payload_object, "reconciliation");
    reconciliation_target.insert("operation_id".to_string(), Value::String(operation_id));
    reconciliation_target.insert(
        "acknowledgement_token".to_string(),
        Value::String(ack_token),
    );
    reconciliation_target.insert("destructive_ack_required".to_string(), Value::Bool(true));
    section_object_mut(payload_object, "annotation").insert(
        "history_rewrite_notice".to_string(),
        Value::String(HISTORY_REWRITE_NOTICE.to_string()),
    );
}
