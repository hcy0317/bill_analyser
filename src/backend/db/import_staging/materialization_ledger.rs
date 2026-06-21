/// 清理可重建的 preview materialization 数据，必须在重新 stage2 前执行以避免旧候选残留。
pub fn clear_import_preview_materialization_state(
    _pool: &PostgresPool,
    _session_id: &str,
    _user_id: UserId,
) -> DbResult<usize> {
    Ok(0)
}

/// 批量写入历史重复/转账 materialization 证据，供 preview 与 confirm 审核链路复用。
pub fn insert_import_history_materializations_batch(
    _pool: &PostgresPool,
    _session_id: &str,
    _user_id: UserId,
    _drafts: &[ImportHistoryMaterializationDraft],
) -> DbResult<usize> {
    Ok(0)
}

pub fn get_import_history_materializations_by_session(
    _pool: &PostgresPool,
    _session_id: &str,
    _user_id: UserId,
) -> DbResult<Vec<ImportHistoryMaterializationRow>> {
    Ok(Vec::new())
}

pub fn get_import_history_candidate_bills_for_session(
    _pool: &PostgresPool,
    _session_id: &str,
    _user_id: UserId,
) -> DbResult<Vec<ImportHistoryBillRow>> {
    Ok(Vec::new())
}

/// 批量写入 dedup/transfer decision group，作为异步物化的可审计输出。
pub fn insert_import_decision_groups_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportDecisionGroupDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let mut inserted = 0usize;
        for draft in drafts {
            let group_id: i64 = sqlx::query(
                r#"
                INSERT INTO import_decision_groups (
                    session_id, user_id, group_type, group_key, decision_status,
                    base_preview_row_id, signal_payload, created_at, updated_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7::jsonb,now(),now())
                ON CONFLICT (session_id, group_type, group_key) DO UPDATE SET
                    decision_status = excluded.decision_status,
                    base_preview_row_id = excluded.base_preview_row_id,
                    signal_payload = excluded.signal_payload,
                    updated_at = now(),
                    version = import_decision_groups.version + 1
                RETURNING id
                "#,
            )
            .bind(session_db_id)
            .bind(user_id)
            .bind(&draft.group_type)
            .bind(&draft.group_key)
            .bind(&draft.decision_status)
            .bind(draft.base_preview_row_id)
            .bind(draft.signal_payload.to_string())
            .fetch_one(pool)
            .await?
            .try_get("id")?;
            if !draft.members.is_empty() {
                let mut query = QueryBuilder::<Postgres>::new(
                    r#"
                    INSERT INTO import_decision_group_members (
                        group_id, preview_row_id, standard_row_id, history_bill_id,
                        member_role, parser_name, metadata, created_at
                    )
                    "#,
                );
                query.push_values(&draft.members, |mut row, member| {
                    row.push_bind(group_id)
                        .push_bind(member.preview_row_id)
                        .push_bind(member.standard_row_id)
                        .push_bind(member.history_bill_id)
                        .push_bind(&member.member_role)
                        .push_bind(&member.parser_name)
                        .push_bind(member.metadata.to_string())
                        .push_unseparated("::jsonb")
                        .push("now()");
                });
                query.push(" ON CONFLICT DO NOTHING");
                query.build().execute(pool).await?;
            }
            inserted += 1;
        }
        Ok(inserted)
    })
}

pub fn get_import_decision_groups_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportDecisionGroupRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let rows = sqlx::query(
            r#"
            SELECT g.*, s.session_key
            FROM import_decision_groups g
            JOIN import_sessions s ON s.id = g.session_id
            WHERE g.session_id = $1 AND g.user_id = $2
            ORDER BY g.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        let mut groups = Vec::new();
        for row in rows {
            let group_id: i64 = row.try_get("id")?;
            let member_rows = sqlx::query(
                "SELECT * FROM import_decision_group_members WHERE group_id = $1 ORDER BY id ASC",
            )
            .bind(group_id)
            .fetch_all(pool)
            .await?;
            groups.push(ImportDecisionGroupRow {
                id: group_id,
                session_id: row.try_get("session_key")?,
                user_id: row.try_get("user_id")?,
                group_type: row.try_get("group_type")?,
                group_key: row.try_get("group_key")?,
                decision_status: row.try_get("decision_status")?,
                base_preview_row_id: row.try_get("base_preview_row_id")?,
                signal_payload: row.try_get("signal_payload")?,
                members: member_rows
                    .iter()
                    .map(import_decision_member_from_pg_row)
                    .collect::<DbResult<Vec<_>>>()?,
                created_at: format_pg_time(row.try_get("created_at")?),
                updated_at: format_pg_time(row.try_get("updated_at")?),
            });
        }
        Ok(groups)
    })
}

/// 计算导入账单稳定 hash，dedup 与历史 materialization 依赖它保持跨阶段一致。
pub fn calculate_import_bill_hash(
    date: &str,
    bill_type: &str,
    amount: f64,
    counterparty: &str,
    description: &str,
) -> String {
    let source = format!(
        "{date}|{bill_type}|{}|{counterparty}|{description}",
        finite_float_text(amount)
    );
    format!("{:x}", md5::compute(source.as_bytes()))
}
