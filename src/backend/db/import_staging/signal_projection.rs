#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImportPreviewSignalProjection {
    parser: bool,
    platform_duplicate: bool,
    transfer: bool,
    history: bool,
    learning: bool,
    llm: bool,
    version: i16,
}

const IMPORT_PREVIEW_SIGNAL_SHADOW_SAMPLE_SIZE: usize = 64;

fn import_preview_signal_projection_from_payload(
    preview_payload: &Value,
) -> DbResult<ImportPreviewSignalProjection> {
    let canonical_row = preview_payload.as_object().ok_or_else(|| {
        DbError::InvalidOperation("preview_payload must be a JSON object".to_string())
    })?;
    let state = bill_analyser_core::derive_import_preview_state_snapshot_from_canonical_row(
        canonical_row,
    );
    let version = i16::try_from(state.projection_version).map_err(|_| {
        DbError::InvalidOperation("preview signal projection version exceeds SMALLINT".to_string())
    })?;
    Ok(ImportPreviewSignalProjection {
        parser: state
            .signals
            .contains(ImportPreviewSignalFamily::Parser),
        platform_duplicate: state
            .signals
            .contains(ImportPreviewSignalFamily::PlatformDuplicate),
        transfer: state
            .signals
            .contains(ImportPreviewSignalFamily::Transfer),
        history: state
            .signals
            .contains(ImportPreviewSignalFamily::History),
        learning: state
            .signals
            .contains(ImportPreviewSignalFamily::Learning),
        llm: state.signals.contains(ImportPreviewSignalFamily::Llm),
        version,
    })
}

fn set_import_preview_signal_review_status(
    preview_payload: &mut Value,
    family: &str,
    status: &str,
) -> DbResult<()> {
    let root = preview_payload.as_object_mut().ok_or_else(|| {
        DbError::InvalidOperation("preview_payload must be a JSON object".to_string())
    })?;
    let feedback = root
        .entry("preview_matching_feedback")
        .or_insert_with(|| json!({}));
    if !feedback.is_object() {
        *feedback = json!({});
    }
    let feedback = feedback
        .as_object_mut()
        .ok_or_else(|| {
            DbError::InvalidOperation(
                "preview_matching_feedback could not be normalized to an object".to_string(),
            )
        })?;
    let family_feedback = feedback.entry(family).or_insert_with(|| json!({}));
    if !family_feedback.is_object() {
        *family_feedback = json!({});
    }
    let family_feedback = family_feedback
        .as_object_mut()
        .ok_or_else(|| {
            DbError::InvalidOperation(format!(
                "preview_matching_feedback.{family} could not be normalized to an object"
            ))
        })?;
    family_feedback.insert("state".to_string(), Value::String(status.to_string()));
    family_feedback.insert(
        "review_status".to_string(),
        Value::String(status.to_string()),
    );
    Ok(())
}

fn push_import_preview_signal_projection_assignments(
    query: &mut QueryBuilder<'static, Postgres>,
    projection: ImportPreviewSignalProjection,
) {
    query.push(", signal_parser = ");
    query.push_bind(projection.parser);
    query.push(", signal_platform_duplicate = ");
    query.push_bind(projection.platform_duplicate);
    query.push(", signal_transfer = ");
    query.push_bind(projection.transfer);
    query.push(", signal_history = ");
    query.push_bind(projection.history);
    query.push(", signal_learning = ");
    query.push_bind(projection.learning);
    query.push(", signal_llm = ");
    query.push_bind(projection.llm);
    query.push(", signal_projection_version = ");
    query.push_bind(projection.version);
}

const IMPORT_PREVIEW_SIGNAL_PROJECTION_PARITY_SQL: &str =
    r#"SELECT count(*)::bigint AS observed_rows,
              count(*) FILTER (WHERE
                p.signal_projection_version <> 1 OR
                p.signal_parser IS DISTINCT FROM legacy.parser OR
                p.signal_platform_duplicate IS DISTINCT FROM legacy.platform_duplicate OR
                p.signal_transfer IS DISTINCT FROM legacy.transfer OR
                p.signal_history IS DISTINCT FROM legacy.history OR
                p.signal_learning IS DISTINCT FROM legacy.learning OR
                p.signal_llm IS DISTINCT FROM legacy.llm
              )::bigint AS mismatch_rows
       FROM import_preview_rows p
       CROSS JOIN LATERAL jsonb_to_record(import_preview_signal_flags(p.preview_payload))
         AS legacy(parser BOOLEAN, platform_duplicate BOOLEAN, transfer BOOLEAN,
                   history BOOLEAN, learning BOOLEAN, llm BOOLEAN)
       WHERE p.id = ANY($1::bigint[])"#;

async fn import_preview_signal_projection_parity_counts(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    preview_ids: &[i64],
) -> DbResult<(i64, i64)> {
    if preview_ids.is_empty() {
        return Ok((0, 0));
    }
    let row = sqlx::query(IMPORT_PREVIEW_SIGNAL_PROJECTION_PARITY_SQL)
        .bind(preview_ids)
        .fetch_one(&mut **tx)
        .await?;
    Ok((
        row.try_get("observed_rows")?,
        row.try_get("mismatch_rows")?,
    ))
}

async fn observe_import_preview_signal_projection_parity(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    preview_ids: &[i64],
    operation: &'static str,
) -> DbResult<()> {
    if preview_ids.is_empty() {
        return Ok(());
    }
    let sampled_preview_ids =
        &preview_ids[..preview_ids.len().min(IMPORT_PREVIEW_SIGNAL_SHADOW_SAMPLE_SIZE)];
    let (observed_rows, mismatch_rows) =
        import_preview_signal_projection_parity_counts(tx, sampled_preview_ids).await?;
    let expected_rows = i64::try_from(sampled_preview_ids.len()).unwrap_or(i64::MAX);
    if observed_rows != expected_rows || mismatch_rows != 0 {
        tracing::warn!(
            target: "bill_analyser::import_signal_projection",
            operation,
            expected_rows,
            observed_rows,
            mismatch_rows,
            "import signal projection shadow parity mismatch"
        );
    } else {
        tracing::debug!(
            target: "bill_analyser::import_signal_projection",
            operation,
            observed_rows,
            "import signal projection shadow parity matched"
        );
    }
    Ok(())
}
