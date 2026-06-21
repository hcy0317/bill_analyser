/// 基于正式账单历史检测周期候选并写入当前用户的周期建议表。
pub async fn detect_and_save_postgres_recurring_suggestions(
    pool: &PostgresPool,
    user_id: UserId,
    patterns: &[RecurringPattern],
) -> DbResult<RecurringSuggestionSaveSummary> {
    let user_id = user_id_i64(user_id)?;
    let mut summary = RecurringSuggestionSaveSummary {
        created: 0,
        updated: 0,
        skipped: 0,
    };

    for pattern in patterns {
        let existing = sqlx::query(
            "SELECT id, status FROM recurring_suggestions WHERE user_id = $1 AND pattern_hash = $2",
        )
        .bind(user_id)
        .bind(&pattern.pattern_hash)
        .fetch_optional(pool)
        .await?;
        if let Some(row) = existing {
            let suggestion_id: i64 = row.try_get("id")?;
            let status: String = row.try_get("status")?;
            if matches!(status.as_str(), "accepted" | "rejected") {
                summary.skipped += 1;
                continue;
            }
            sqlx::query(
                r#"
                UPDATE recurring_suggestions SET
                    name = $1,
                    description = $2,
                    type = $3,
                    amount_cents = $4,
                    source_account_id = $5,
                    destination_account_id = $6,
                    counterparty = $7,
                    frequency = $8,
                    detected_interval_days = $9,
                    confidence_score = $10,
                    sample_count = $11,
                    sample_bill_ids = $12,
                    first_occurrence = $13,
                    last_occurrence = $14,
                    suggested_next_date = $15,
                    updated_at = now(),
                    version = version + 1
                WHERE id = $16 AND user_id = $17
                "#,
            )
            .bind(&pattern.name)
            .bind(&pattern.description)
            .bind(&pattern.transaction_type)
            .bind(pattern.amount_cents)
            .bind(pattern.source_account_id)
            .bind(optional_i64_from_text(&pattern.destination_account_id))
            .bind(&pattern.counterparty)
            .bind(&pattern.frequency)
            .bind(pattern.detected_interval_days)
            .bind(pattern.confidence_score)
            .bind(i32::try_from(pattern.sample_count).unwrap_or(i32::MAX))
            .bind(json!(pattern.sample_bill_ids))
            .bind(&pattern.first_occurrence)
            .bind(&pattern.last_occurrence)
            .bind(&pattern.suggested_next_date)
            .bind(suggestion_id)
            .bind(user_id)
            .execute(pool)
            .await?;
            summary.updated += 1;
            continue;
        }

        sqlx::query(
            r#"
            INSERT INTO recurring_suggestions (
                user_id, pattern_hash, name, description, type, amount_cents,
                source_account_id, destination_account_id, counterparty,
                frequency, detected_interval_days, confidence_score, sample_count,
                sample_bill_ids, first_occurrence, last_occurrence, suggested_next_date
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16, $17
            )
            "#,
        )
        .bind(user_id)
        .bind(&pattern.pattern_hash)
        .bind(&pattern.name)
        .bind(&pattern.description)
        .bind(&pattern.transaction_type)
        .bind(pattern.amount_cents)
        .bind(pattern.source_account_id)
        .bind(optional_i64_from_text(&pattern.destination_account_id))
        .bind(&pattern.counterparty)
        .bind(&pattern.frequency)
        .bind(pattern.detected_interval_days)
        .bind(pattern.confidence_score)
        .bind(i32::try_from(pattern.sample_count).unwrap_or(i32::MAX))
        .bind(json!(pattern.sample_bill_ids))
        .bind(&pattern.first_occurrence)
        .bind(&pattern.last_occurrence)
        .bind(&pattern.suggested_next_date)
        .execute(pool)
        .await?;
        summary.created += 1;
    }

    Ok(summary)
}
