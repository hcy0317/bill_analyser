/// Decode the versioned durable v1 receipt into the transport-neutral repository outcome.
///
/// The envelope remains a persistence compatibility contract until target databases have completed
/// the typed receipt rollout. Runtime callers must consume `ConfirmOutcome` instead of this JSON.
fn confirm_preview_result_from_receipt_envelope(
    envelope: &Value,
) -> DbResult<ConfirmPreviewResult> {
    let data = envelope.get("data").ok_or_else(|| {
        DbError::InvalidOperation("invalid import confirm receipt envelope".to_string())
    })?;
    let data = serde_json::from_value::<bill_analyser_core::ImportStageConfirmData>(data.clone())
        .map_err(|_| {
            DbError::InvalidOperation("invalid import confirm receipt envelope".to_string())
        })?;
    Ok(ConfirmPreviewResult {
        confirmed_count: data.imported_count,
        skipped_count: data.skipped_count,
        duplicate_count: 0,
        errors: data.errors,
    })
}

/// Encode the transport-neutral result into the immutable v1 receipt envelope.
///
/// This is the sole transitional writer for the legacy envelope stored by both metadata and typed
/// receipt projections. It is not a public HTTP response builder.
fn confirm_receipt_success_envelope(result: &ConfirmPreviewResult) -> Value {
    json!({
        "success": true,
        "data": {
            "imported_count": result.confirmed_count,
            "skipped_count": result.skipped_count + result.duplicate_count,
            "errors": result.errors,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_decoder_rejects_envelopes_without_data() {
        let error = confirm_preview_result_from_receipt_envelope(&json!({ "success": true }))
            .expect_err("a durable receipt without data must fail closed");

        assert!(matches!(
            error,
            DbError::InvalidOperation(message)
                if message == "invalid import confirm receipt envelope"
        ));
    }

    #[test]
    fn receipt_decoder_rejects_malformed_data() {
        let error = confirm_preview_result_from_receipt_envelope(&json!({
            "data": {
                "imported_count": "invalid",
                "skipped_count": 0,
                "errors": []
            }
        }))
        .expect_err("malformed durable receipt data must fail closed");

        assert!(matches!(
            error,
            DbError::InvalidOperation(message)
                if message == "invalid import confirm receipt envelope"
        ));
    }
}
