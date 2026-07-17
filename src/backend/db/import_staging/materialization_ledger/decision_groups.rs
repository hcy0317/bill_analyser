/// 批量写入 dedup/transfer decision group，作为异步物化的可审计输出。
pub fn insert_import_decision_groups_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportDecisionGroupDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        if drafts.is_empty() {
            tx.commit().await?;
            return Ok(0);
        }
        let mut inserted = 0usize;
        for draft in drafts {
            validate_import_decision_group_draft_scope(
                &mut tx,
                session_db_id,
                user_id,
                draft,
            )
            .await?;
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
            .fetch_one(&mut *tx)
            .await?
            .try_get("id")?;
            sqlx::query("DELETE FROM import_decision_group_members WHERE group_id = $1")
                .bind(group_id)
                .execute(&mut *tx)
                .await?;
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
                query.build().execute(&mut *tx).await?;
            }
            inserted += 1;
        }
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id).await?;
        tx.commit().await?;
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
                r#"
                SELECT m.*,
                       COALESCE(p.version, sr.version, b.version, 1) AS member_version
                FROM import_decision_group_members m
                LEFT JOIN import_preview_rows p
                  ON p.id = m.preview_row_id
                 AND p.session_id = $2
                 AND p.user_id = $3
                LEFT JOIN import_standard_rows sr
                  ON sr.id = m.standard_row_id
                 AND sr.session_id = $2
                 AND sr.user_id = $3
                LEFT JOIN bills b
                  ON b.id = m.history_bill_id
                 AND b.user_id = $3
                WHERE m.group_id = $1
                ORDER BY m.id ASC
                "#,
            )
            .bind(group_id)
            .bind(session_db_id)
            .bind(user_id)
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
                version: row.try_get("version")?,
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

async fn validate_import_decision_group_draft_scope(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    draft: &ImportDecisionGroupDraft,
) -> DbResult<()> {
    let mut preview_ids = draft
        .members
        .iter()
        .filter_map(|member| member.preview_row_id)
        .collect::<Vec<_>>();
    preview_ids.extend(draft.base_preview_row_id);
    preview_ids.sort_unstable();
    preview_ids.dedup();
    let mut standard_row_ids = draft
        .members
        .iter()
        .filter_map(|member| member.standard_row_id)
        .collect::<Vec<_>>();
    standard_row_ids.sort_unstable();
    standard_row_ids.dedup();
    let mut history_bill_ids = draft
        .members
        .iter()
        .filter_map(|member| member.history_bill_id)
        .collect::<Vec<_>>();
    history_bill_ids.sort_unstable();
    history_bill_ids.dedup();

    if !preview_ids.is_empty() {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM import_preview_rows WHERE id=ANY($1) AND session_id=$2 AND user_id=$3",
        )
        .bind(&preview_ids)
        .bind(session_db_id)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;
        if decision_group_database_count(count, "preview member")? != preview_ids.len() {
            return Err(DbError::InvalidOperation(
                "decision group preview member scope mismatch".into(),
            ));
        }
    }
    if !standard_row_ids.is_empty() {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM import_standard_rows WHERE id=ANY($1) AND session_id=$2 AND user_id=$3",
        )
        .bind(&standard_row_ids)
        .bind(session_db_id)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;
        if decision_group_database_count(count, "standard-row member")? != standard_row_ids.len() {
            return Err(DbError::InvalidOperation(
                "decision group standard-row member scope mismatch".into(),
            ));
        }
    }
    if !history_bill_ids.is_empty() {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM bills WHERE id=ANY($1) AND user_id=$2 AND is_deleted=false",
        )
        .bind(&history_bill_ids)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;
        if decision_group_database_count(count, "history member")? != history_bill_ids.len() {
            return Err(DbError::InvalidOperation(
                "decision group history member scope mismatch".into(),
            ));
        }
    }
    Ok(())
}

fn decision_group_database_count(count: i64, subject: &str) -> DbResult<usize> {
    usize::try_from(count).map_err(|_| {
        DbError::InvalidOperation(format!(
            "invalid decision group {subject} count returned by PostgreSQL: {count}"
        ))
    })
}

/// 以 decision group 为聚合边界执行 CAS 决策；任一成员版本过期时事务零写入。
pub fn apply_import_decision_group_command(
    pool: &PostgresPool,
    user_id: UserId,
    command: &ImportDecisionGroupCommand,
) -> DbResult<ImportDecisionGroupCommandResult> {
    let command = command.clone();
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session = sqlx::query("SELECT id, COALESCE(metadata#>>'{decision_group_materialization,status}', 'pending') AS materialization_status FROM import_sessions WHERE session_key=$1 AND user_id=$2 FOR UPDATE")
            .bind(&command.session_id).bind(user_id).fetch_optional(&mut *tx).await?;
        let Some(session) = session else {
            tx.rollback().await?;
            return Ok(ImportDecisionGroupCommandResult::NotFound);
        };
        let session_db_id: i64 = session.try_get("id")?;
        let materialization_status: String = session.try_get("materialization_status")?;
        let group = sqlx::query(
            r#"SELECT g.version, g.decision_status, g.group_type
               FROM import_decision_groups g
               JOIN import_sessions s ON s.id = g.session_id
               WHERE g.id = $1 AND g.user_id = $2 AND s.session_key = $3
               FOR UPDATE"#,
        )
        .bind(command.group_id)
        .bind(user_id)
        .bind(&command.session_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(group) = group else {
            tx.rollback().await?;
            return Ok(match materialization_status.as_str() {
                "pending" => ImportDecisionGroupCommandResult::MaterializationPending,
                "failed" => ImportDecisionGroupCommandResult::MaterializationFailed,
                _ => ImportDecisionGroupCommandResult::NotFound,
            });
        };
        let group_version: i64 = group.try_get("version")?;
        let current_status: String = group.try_get("decision_status")?;
        let group_type: String = group.try_get("group_type")?;
        let decision = command.decision.trim().to_ascii_lowercase();
        let target_status = match decision.as_str() {
            "accept" | "accepted" => "accepted",
            "reject" | "rejected" => "rejected",
            "clear" | "cleared" => "pending",
            _ => {
                tx.rollback().await?;
                return Ok(ImportDecisionGroupCommandResult::Conflict);
            }
        };
        if command.operation_id.trim().is_empty() {
            tx.rollback().await?;
            return Ok(ImportDecisionGroupCommandResult::Conflict);
        }
        if let Some(operation) = sqlx::query("SELECT payload FROM import_confirm_operations WHERE user_id=$1 AND session_id=$2 AND operation_kind='decision_group' AND operation_id=$3 FOR UPDATE")
            .bind(user_id).bind(session_db_id).bind(command.operation_id.trim())
            .fetch_optional(&mut *tx).await? {
            let payload: Value = operation.try_get("payload")?;
            let same_target = payload.get("group_id").and_then(Value::as_i64) == Some(command.group_id)
                && payload.get("decision_status").and_then(Value::as_str) == Some(target_status);
            let replay = payload.get("result").cloned()
                .and_then(|value| serde_json::from_value::<ImportDecisionGroupMutation>(value).ok());
            tx.commit().await?;
            return Ok(match (same_target, replay) {
                (true, Some(result)) => ImportDecisionGroupCommandResult::Applied(result),
                _ => ImportDecisionGroupCommandResult::Conflict,
            });
        }
        let rows = sqlx::query(
            r#"SELECT p.id, p.version
               FROM import_preview_rows p
               WHERE p.session_id=$2 AND p.user_id=$3
                 AND EXISTS (
                   SELECT 1 FROM import_decision_group_members m
                   WHERE m.group_id=$1 AND m.preview_row_id=p.id
               )
               ORDER BY p.id FOR UPDATE OF p"#,
        )
        .bind(command.group_id)
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?;
        let standard_rows = sqlx::query(
            r#"SELECT sr.id FROM import_decision_group_members m
               JOIN import_standard_rows sr ON sr.id = m.standard_row_id
               WHERE m.group_id = $1 AND sr.session_id=$2 AND sr.user_id=$3
               FOR UPDATE OF sr"#,
        )
        .bind(command.group_id)
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?;
        let member_reference_counts = sqlx::query(
            r#"SELECT count(DISTINCT preview_row_id) FILTER (WHERE preview_row_id IS NOT NULL)
                         AS preview_count,
                      count(DISTINCT standard_row_id) FILTER (WHERE standard_row_id IS NOT NULL)
                         AS standard_count
               FROM import_decision_group_members
               WHERE group_id=$1"#,
        )
        .bind(command.group_id)
        .fetch_one(&mut *tx)
        .await?;
        let referenced_preview_count: i64 = member_reference_counts.try_get("preview_count")?;
        let referenced_standard_count: i64 = member_reference_counts.try_get("standard_count")?;
        let locked_standard_count = standard_rows
            .iter()
            .map(|row| row.try_get::<i64, _>("id"))
            .collect::<Result<std::collections::HashSet<_>, sqlx::Error>>()?
            .len();
        if decision_group_database_count(referenced_preview_count, "referenced preview")?
            != rows.len()
            || decision_group_database_count(referenced_standard_count, "referenced standard-row")?
                != locked_standard_count
        {
            tx.rollback().await?;
            return Ok(ImportDecisionGroupCommandResult::Conflict);
        }
        let current = rows
            .iter()
            .map(|row| ImportDecisionPreviewVersion {
                preview_row_id: row.try_get("id").unwrap_or_default(),
                version: row.try_get("version").unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        let mut expected = command.expected_preview_versions.clone();
        expected.sort_by_key(|item| item.preview_row_id);
        if group_version != command.expected_group_version || current != expected {
            tx.rollback().await?;
            return Ok(ImportDecisionGroupCommandResult::Conflict);
        }
        if current_status == target_status {
            let result = ImportDecisionGroupMutation {
                group_id: command.group_id,
                group_version,
                decision_status: current_status,
                removed_preview_ids: Vec::new(),
                upserted_preview_ids: current.iter().map(|item| item.preview_row_id).collect(),
                upserted_preview_items: Vec::new(),
            };
            sqlx::query(r#"INSERT INTO import_confirm_operations
                (session_id,user_id,operation_kind,operation_id,status,payload)
                VALUES ($1,$2,'decision_group',$3,'completed',$4)"#)
                .bind(session_db_id).bind(user_id).bind(command.operation_id.trim())
                .bind(json!({"group_id": command.group_id, "decision_status": target_status, "result": result}))
                .execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(ImportDecisionGroupCommandResult::Applied(result));
        }
        let next_version: i64 = sqlx::query_scalar(
            r#"UPDATE import_decision_groups SET decision_status = $1,
               version = version + 1, updated_at = now()
               WHERE id = $2 AND version = $3 AND user_id=$4 AND session_id=$5
               RETURNING version"#,
        )
        .bind(target_status)
        .bind(command.group_id)
        .bind(group_version)
        .bind(user_id)
        .bind(session_db_id)
        .fetch_one(&mut *tx)
        .await?;
        if group_type == "same_batch_transfer" && target_status == "rejected" {
            let result = reject_same_batch_transfer_and_dematerialize(
                &mut tx,
                session_db_id,
                user_id,
                command.group_id,
                next_version,
                &current,
            )
            .await?;
            sqlx::query(r#"INSERT INTO import_confirm_operations
                (session_id,user_id,operation_kind,operation_id,status,payload)
                VALUES ($1,$2,'decision_group',$3,'completed',$4)"#)
                .bind(session_db_id).bind(user_id).bind(command.operation_id.trim())
                .bind(json!({"group_id": command.group_id, "decision_status": target_status, "result": result}))
                .execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(ImportDecisionGroupCommandResult::Applied(result));
        }
        if matches!(
            group_type.as_str(),
            "historical_transfer" | "historical_duplicate"
        ) && target_status == "rejected"
        {
            let result = reject_historical_group_and_dematerialize(
                &mut tx,
                session_db_id,
                user_id,
                command.group_id,
                next_version,
                &current,
            )
            .await?;
            sqlx::query(r#"INSERT INTO import_confirm_operations
                (session_id,user_id,operation_kind,operation_id,status,payload)
                VALUES ($1,$2,'decision_group',$3,'completed',$4)"#)
                .bind(session_db_id).bind(user_id).bind(command.operation_id.trim())
                .bind(json!({"group_id": command.group_id, "decision_status": target_status, "result": result}))
                .execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(ImportDecisionGroupCommandResult::Applied(result));
        }
        for row in &current {
            sqlx::query(
                r#"UPDATE import_preview_rows SET
                   preview_payload = jsonb_set(jsonb_set(preview_payload,
                     '{preview_matching_feedback,transfer,state}', to_jsonb($1::text), true),
                     '{preview_matching_feedback,transfer,review_status}', to_jsonb($1::text), true),
                   version = version + 1, updated_at = now()
                   WHERE id = $2 AND version = $3 AND session_id=$4 AND user_id=$5"#,
            )
            .bind(target_status)
            .bind(row.preview_row_id)
            .bind(row.version)
            .bind(session_db_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        }
        let result = ImportDecisionGroupMutation {
            group_id: command.group_id,
            group_version: next_version,
            decision_status: target_status.to_string(),
            removed_preview_ids: Vec::new(),
            upserted_preview_ids: current.iter().map(|item| item.preview_row_id).collect(),
            upserted_preview_items: Vec::new(),
        };
        sqlx::query(r#"INSERT INTO import_confirm_operations
            (session_id,user_id,operation_kind,operation_id,status,payload)
            VALUES ($1,$2,'decision_group',$3,'completed',$4)"#)
            .bind(session_db_id).bind(user_id).bind(command.operation_id.trim())
            .bind(json!({"group_id": command.group_id, "decision_status": target_status, "result": result}))
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(ImportDecisionGroupCommandResult::Applied(result))
    })
}
