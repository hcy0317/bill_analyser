/// 清理可重建的 preview materialization 数据，必须在重新 stage2 前执行以避免旧候选残留。
pub fn clear_import_preview_materialization_state(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<usize> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let feedback_events = sqlx::query(
            r#"
            UPDATE import_learning_feedback_events
            SET suggestion_id = NULL
            WHERE user_id = $1
              AND suggestion_id IN (
                  SELECT id FROM import_learning_suggestions
                  WHERE user_id = $1 AND session_id = $2
              )
            "#,
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let learning = sqlx::query(
            "DELETE FROM import_learning_suggestions WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let matching = sqlx::query(
            "DELETE FROM preview_matching_feedback WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let confirm = sqlx::query(
            "DELETE FROM import_confirm_operations WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let group_members = sqlx::query(
            r#"
            DELETE FROM import_decision_group_members members
            USING import_decision_groups groups
            WHERE members.group_id = groups.id
              AND groups.user_id = $1
              AND groups.session_id = $2
            "#,
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let groups = sqlx::query(
            "DELETE FROM import_decision_groups WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let history = sqlx::query(
            "DELETE FROM import_history_materializations WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let previews = sqlx::query(
            "DELETE FROM import_preview_rows WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id).await?;
        tx.commit().await?;
        Ok(usize::try_from(
            feedback_events
                + learning
                + matching
                + confirm
                + group_members
                + groups
                + history
                + previews,
        )
        .unwrap_or(usize::MAX))
    })
}

/// 批量写入历史重复/转账 materialization 证据，供 preview 与 confirm 审核链路复用。
pub fn insert_import_history_materializations_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportHistoryMaterializationDraft],
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
            let payload = canonical_history_materialization_payload(&draft.materialized_payload)?;
            let row = sqlx::query(
                r#"
                INSERT INTO import_history_materializations (
                    session_id, user_id, history_bill_id, history_bill_version,
                    materialized_payload, rewrite_reason, created_at
                )
                SELECT $1, $2, bill.id, bill.version, $5::jsonb, $6, now()
                FROM bills bill
                WHERE bill.user_id = $2
                  AND bill.id = $3
                  AND bill.version = $4
                  AND bill.is_deleted = false
                ON CONFLICT (session_id, history_bill_id) DO UPDATE SET
                    history_bill_version = excluded.history_bill_version,
                    materialized_payload = excluded.materialized_payload,
                    rewrite_reason = excluded.rewrite_reason
                RETURNING id
                "#,
            )
            .bind(session_db_id)
            .bind(user_id)
            .bind(draft.history_bill_id)
            .bind(draft.history_bill_version)
            .bind(payload.to_string())
            .bind(&draft.rewrite_reason)
            .fetch_optional(&mut *tx)
            .await?;
            if row.is_none() {
                return Err(DbError::InvalidOperation(format!(
                    "history materialization CAS mismatch: bill {} version {}",
                    draft.history_bill_id, draft.history_bill_version
                )));
            }
            inserted += 1;
        }
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id).await?;
        tx.commit().await?;
        Ok(inserted)
    })
}

pub fn get_import_history_materializations_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportHistoryMaterializationRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let rows = sqlx::query(
            r#"
            SELECT materialization.*, session.session_key
            FROM import_history_materializations materialization
            JOIN import_sessions session ON session.id = materialization.session_id
            WHERE materialization.session_id = $1 AND materialization.user_id = $2
            ORDER BY materialization.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(ImportHistoryMaterializationRow {
                    id: row.try_get("id")?,
                    session_id: row.try_get("session_key")?,
                    user_id: row.try_get("user_id")?,
                    history_bill_id: row.try_get("history_bill_id")?,
                    history_bill_version: row.try_get("history_bill_version")?,
                    materialized_payload: row.try_get("materialized_payload")?,
                    rewrite_reason: row.try_get("rewrite_reason")?,
                    created_at: format_pg_time(row.try_get("created_at")?),
                })
            })
            .collect()
    })
}

pub fn get_import_history_candidate_bills_for_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportHistoryBillRow>> {
    block_on_db(async move {
        let _session_db_id = active_session_db_id(pool, session_id, user_id_i64(user_id)?).await?;
        let user_id = user_id_i64(user_id)?;
        let rows = sqlx::query(
            r#"
            SELECT bill.*, category.path AS category_path, account.name AS account_name,
                   target.name AS target_account_name
            FROM bills bill
            LEFT JOIN categories category
              ON category.id = bill.category_id AND category.user_id = bill.user_id
            LEFT JOIN accounts account
              ON account.id = COALESCE(bill.source_account_id, bill.account_id)
             AND account.user_id = bill.user_id
            LEFT JOIN accounts target
              ON target.id = COALESCE(bill.target_account_id, bill.transfer_target_account_id)
             AND target.user_id = bill.user_id
            WHERE bill.user_id = $1 AND bill.is_deleted = false
            ORDER BY bill.occurred_at ASC, bill.id ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        rows.iter().map(history_bill_from_pg_row).collect()
    })
}

fn canonical_history_materialization_payload(payload: &Value) -> DbResult<Value> {
    let mut payload = payload.clone();
    let Some(object) = payload.as_object_mut() else {
        return Err(DbError::InvalidOperation(
            "history materialization payload must be an object".to_string(),
        ));
    };
    let operation_key = if object.contains_key("planned_operation") {
        "planned_operation"
    } else if object.contains_key("operation") {
        "operation"
    } else {
        return Err(DbError::InvalidOperation(
            "history materialization operation is required".to_string(),
        ));
    };
    let raw_operation = object
        .get(operation_key)
        .and_then(Value::as_str)
        .unwrap_or_default();
    let operation = normalize_history_operation(raw_operation).ok_or_else(|| {
        DbError::InvalidOperation("invalid history materialization operation".to_string())
    })?;
    object.insert(
        operation_key.to_string(),
        Value::String(operation.as_str().to_string()),
    );
    Ok(payload)
}

fn history_bill_from_pg_row(row: &PgRow) -> DbResult<ImportHistoryBillRow> {
    let history_bill_id = row.try_get::<i64, _>("id")?;
    let history_bill_version = row.try_get::<i64, _>("version")?;
    let amount_cents = row.try_get::<i64, _>("amount_cents")?;
    let direction = row.try_get::<String, _>("direction")?;
    let signed_amount_cents = if direction == "expense" {
        -amount_cents.abs()
    } else {
        amount_cents.abs()
    };
    let category_path = row
        .try_get::<Option<String>, _>("category_path")?
        .unwrap_or_default();
    let (main_category, sub_category) = category_path
        .split_once('/')
        .map(|(main, sub)| (main.to_string(), sub.to_string()))
        .unwrap_or_else(|| (category_path.clone(), String::new()));
    let source_account_id = row
        .try_get::<Option<i64>, _>("source_account_id")?
        .or(row.try_get::<Option<i64>, _>("account_id")?);
    let destination_account_id = row
        .try_get::<Option<i64>, _>("target_account_id")?
        .or(row.try_get::<Option<i64>, _>("transfer_target_account_id")?);
    let snapshot = json!({
        "id": history_bill_id,
        "version": history_bill_version,
        "occurred_at": format_pg_time(row.try_get("occurred_at")?),
        "amount_cents": amount_cents,
        "direction": direction,
        "transaction_type": row.try_get::<String, _>("transaction_type")?,
        "source_account_id": source_account_id,
        "destination_account_id": destination_account_id,
        "category_id": row.try_get::<Option<i64>, _>("category_id")?,
        "merchant": row.try_get::<Option<String>, _>("merchant")?,
        "payment_method": row.try_get::<Option<String>, _>("payment_method")?,
        "description": row.try_get::<Option<String>, _>("description")?,
    });
    Ok(ImportHistoryBillRow {
        history_bill_id,
        history_bill_version,
        bill: DedupBill {
            id: Some(history_bill_id.to_string()),
            date: snapshot["occurred_at"].as_str().unwrap_or_default().to_string(),
            amount: Money::from_cents(signed_amount_cents),
            transaction_type: snapshot["transaction_type"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            source_account_id: source_account_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            destination_account_id: destination_account_id.map(|value| value.to_string()),
            parser_id: row
                .try_get::<Option<String>, _>("parser_name")?
                .unwrap_or_else(|| "history_db".to_string()),
            source: "postgres".to_string(),
            counterparty: row
                .try_get::<Option<String>, _>("merchant")?
                .unwrap_or_default(),
            payment_method: row
                .try_get::<Option<String>, _>("payment_method")?
                .unwrap_or_default(),
            description: row
                .try_get::<Option<String>, _>("description")?
                .unwrap_or_default(),
            main_category,
            sub_category,
            account_name: row
                .try_get::<Option<String>, _>("account_name")?
                .unwrap_or_default(),
            destination_account_name: row
                .try_get::<Option<String>, _>("target_account_name")?
                .unwrap_or_default(),
            ..DedupBill::default()
        },
        snapshot,
    })
}

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
               WHERE EXISTS (
                   SELECT 1 FROM import_decision_group_members m
                   WHERE m.group_id=$1 AND m.preview_row_id=p.id
               )
               ORDER BY p.id FOR UPDATE OF p"#,
        )
        .bind(command.group_id)
        .fetch_all(&mut *tx)
        .await?;
        sqlx::query(
            r#"SELECT sr.id FROM import_decision_group_members m
               JOIN import_standard_rows sr ON sr.id = m.standard_row_id
               WHERE m.group_id = $1 FOR UPDATE OF sr"#,
        )
        .bind(command.group_id)
        .fetch_all(&mut *tx)
        .await?;
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
               WHERE id = $2 AND version = $3 RETURNING version"#,
        )
        .bind(target_status)
        .bind(command.group_id)
        .bind(group_version)
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
                   WHERE id = $2 AND version = $3"#,
            )
            .bind(target_status)
            .bind(row.preview_row_id)
            .bind(row.version)
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

async fn reject_historical_group_and_dematerialize(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    group_id: i64,
    group_version: i64,
    current: &[ImportDecisionPreviewVersion],
) -> DbResult<ImportDecisionGroupMutation> {
    if current.len() != 1 {
        return Err(DbError::InvalidOperation(
            "historical rejection requires one materialized preview".into(),
        ));
    }
    let old_id = current[0].preview_row_id;
    let old: PgRow = sqlx::query(
        "SELECT selected FROM import_preview_rows WHERE id=$1 AND session_id=$2 AND user_id=$3",
    )
    .bind(old_id)
    .bind(session_db_id)
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;
    let member = sqlx::query(
        r#"SELECT m.id AS member_id, sr.id AS standard_row_id, sr.occurred_at, sr.amount_cents,
                  sr.direction, sr.transaction_type, sr.merchant, sr.payment_method,
                  sr.description, sr.parser_payload, sr.standard_payload
           FROM import_decision_group_members m
           JOIN import_standard_rows sr ON sr.id=m.standard_row_id
           WHERE m.group_id=$1 AND m.history_bill_id IS NULL
           FOR UPDATE OF sr, m"#,
    )
    .bind(group_id)
    .fetch_one(&mut **tx)
    .await?;
    let standard_row_id: i64 = member.try_get("standard_row_id")?;
    let occurred_at: DateTime<Utc> = member.try_get("occurred_at")?;
    let amount_cents: i64 = member.try_get("amount_cents")?;
    let direction: String = member.try_get("direction")?;
    let preview_type = if direction == "income" {
        "收入"
    } else {
        "支出"
    };
    let payload = json!({
        "preview_date": occurred_at.to_rfc3339(), "preview_type": preview_type,
        "preview_amount_cents": amount_cents.abs(), "preview_destination_amount_cents": 0,
        "category_id": null, "preview_main_category": "", "preview_sub_category": "",
        "preview_source_account_id": null, "preview_destination_account_id": null,
        "preview_counterparty": member.try_get::<Option<String>, _>("merchant")?.unwrap_or_default(),
        "preview_payment_method": member.try_get::<Option<String>, _>("payment_method")?.unwrap_or_default(),
        "preview_description": member.try_get::<Option<String>, _>("description")?.unwrap_or_default(),
        "preview_parser_id": member.try_get::<Value, _>("parser_payload")?.get("parser_id").and_then(Value::as_str).unwrap_or("auto"),
        "preview_selected": old.try_get::<bool, _>("selected")?, "dedup_type": null,
        "dedup_source_ids": [standard_row_id],
        "preview_matching_feedback": {"annotation": {"status": "pending_reclassification"}}
    });
    let new_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO import_preview_rows
           (session_id,user_id,base_standard_row_id,page_sort_key,operation_kind,selected,signal_summary,
            merged_source_ids,occurred_at,amount_cents,direction,transaction_type,merchant,payment_method,
            description,preview_payload,created_at,updated_at)
           VALUES($1,$2,$3,$4,'insert',$5,'[]',$6,$7,$8,$9,$10,$11,$12,$13,$14,now(),now()) RETURNING id"#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .bind(standard_row_id)
    .bind(format!("{}:{}", occurred_at.to_rfc3339(), standard_row_id))
    .bind(old.try_get::<bool, _>("selected")?)
    .bind(vec![standard_row_id])
    .bind(occurred_at)
    .bind(amount_cents.abs())
    .bind(&direction)
    .bind(member.try_get::<String, _>("transaction_type")?)
    .bind(member.try_get::<Option<String>, _>("merchant")?)
    .bind(member.try_get::<Option<String>, _>("payment_method")?)
    .bind(member.try_get::<Option<String>, _>("description")?)
    .bind(&payload)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query("UPDATE import_decision_group_members SET preview_row_id=CASE WHEN history_bill_id IS NULL THEN $1 ELSE NULL END WHERE group_id=$2")
        .bind(new_id).bind(group_id).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM import_preview_rows WHERE id=$1 AND session_id=$2 AND user_id=$3")
        .bind(old_id).bind(session_db_id).bind(user_id).execute(&mut **tx).await?;
    sqlx::query("UPDATE import_decision_groups SET decision_status='suppressed',base_preview_row_id=$1,signal_payload=jsonb_build_object('suppressed',true,'reclassification',jsonb_build_object('status','pending','attempts',0,'preview_ids',jsonb_build_array($1))) WHERE id=$2")
        .bind(new_id).bind(group_id).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM import_history_materializations WHERE session_id=$1 AND user_id=$2 AND history_bill_id IN (SELECT history_bill_id FROM import_decision_group_members WHERE group_id=$3 AND history_bill_id IS NOT NULL)")
        .bind(session_db_id).bind(user_id).bind(group_id).execute(&mut **tx).await?;
    let mut item = payload;
    item.as_object_mut().unwrap().insert("id".into(), json!(new_id));
    item.as_object_mut()
        .unwrap()
        .insert("version".into(), json!(1));
    Ok(ImportDecisionGroupMutation {
        group_id,
        group_version,
        decision_status: "suppressed".into(),
        removed_preview_ids: vec![old_id],
        upserted_preview_ids: vec![new_id],
        upserted_preview_items: vec![item],
    })
}

async fn reject_same_batch_transfer_and_dematerialize(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    group_id: i64,
    group_version: i64,
    current: &[ImportDecisionPreviewVersion],
) -> DbResult<ImportDecisionGroupMutation> {
    if current.len() != 1 {
        return Err(DbError::InvalidOperation(
            "same-batch transfer rejection requires exactly one materialized preview".into(),
        ));
    }
    let old_preview_id = current[0].preview_row_id;
    let old = sqlx::query(
        "SELECT selected, preview_payload FROM import_preview_rows WHERE id=$1 AND session_id=$2 AND user_id=$3",
    )
    .bind(old_preview_id)
    .bind(session_db_id)
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;
    let selected: bool = old.try_get("selected")?;
    let old_payload: Value = old.try_get("preview_payload")?;
    let manual_fields = old_payload
        .pointer("/preview_matching_feedback/annotation/manual_fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let members = sqlx::query(
        r#"SELECT m.id AS member_id, m.member_role, sr.id AS standard_row_id,
                  sr.occurred_at, sr.amount_cents, sr.direction, sr.transaction_type,
                  sr.merchant, sr.payment_method, sr.description, sr.parser_payload
           FROM import_decision_group_members m
           JOIN import_standard_rows sr ON sr.id=m.standard_row_id
           WHERE m.group_id=$1 AND m.member_role IN ('outgoing','incoming')
           ORDER BY CASE m.member_role WHEN 'outgoing' THEN 0 ELSE 1 END, m.id
           FOR UPDATE OF sr, m"#,
    )
    .bind(group_id)
    .fetch_all(&mut **tx)
    .await?;
    if members.len() != 2 {
        return Err(DbError::InvalidOperation(
            "same-batch transfer rejection requires outgoing and incoming standard rows".into(),
        ));
    }
    let mut new_ids = Vec::with_capacity(2);
    let mut items = Vec::with_capacity(2);
    for member in members {
        let role: String = member.try_get("member_role")?;
        let standard_row_id: i64 = member.try_get("standard_row_id")?;
        let occurred_at: DateTime<Utc> = member.try_get("occurred_at")?;
        let amount_cents: i64 = member.try_get("amount_cents")?;
        let direction: String = member.try_get("direction")?;
        let transaction_type: String = member.try_get("transaction_type")?;
        let merchant: String = member.try_get("merchant")?;
        let payment_method: String = member.try_get("payment_method")?;
        let description: String = member.try_get("description")?;
        let parser_payload: Value = member.try_get("parser_payload")?;
        let outgoing = role == "outgoing";
        let keep_category =
            outgoing && manual_fields.get("category_id").and_then(Value::as_bool) == Some(true);
        let keep_account = outgoing
            && manual_fields
                .get("source_account_id")
                .and_then(Value::as_bool)
                == Some(true);
        let category_id = keep_category
            .then(|| old_payload.get("category_id").and_then(Value::as_i64))
            .flatten();
        let account_id = keep_account
            .then(|| {
                old_payload
                    .get("preview_source_account_id")
                    .and_then(Value::as_i64)
            })
            .flatten();
        let preview_type = if direction == "income" {
            "收入"
        } else {
            "支出"
        };
        let preview_payload = json!({
            "preview_date": occurred_at.to_rfc3339(),
            "preview_type": preview_type,
            "preview_amount_cents": amount_cents.abs(),
            "preview_destination_amount_cents": 0,
            "category_id": category_id,
            "categoryId": category_id,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": account_id,
            "preview_destination_account_id": null,
            "preview_counterparty": merchant,
            "preview_payment_method": payment_method,
            "preview_description": description,
            "preview_parser_id": parser_payload.get("parser_id").and_then(Value::as_str).unwrap_or("auto"),
            "preview_parser_tags": parser_payload.get("parser_tags").cloned().unwrap_or_else(|| json!([])),
            "preview_selected": selected,
            "dedup_type": null,
            "dedup_source_ids": [standard_row_id],
            "preview_matching_feedback": {
                "annotation": {"manual_fields": if outgoing { manual_fields.clone() } else { json!({}) }},
                "transfer": {"state": "rejected", "review_status": "rejected", "suppressed_group_id": group_id}
            }
        });
        let id: i64 = sqlx::query_scalar(
            r#"INSERT INTO import_preview_rows
               (session_id,user_id,page_sort_key,operation_kind,selected,signal_summary,
                merged_source_ids,occurred_at,amount_cents,direction,transaction_type,
                account_id,transfer_target_account_id,category_id,merchant,payment_method,
                description,preview_payload,created_at,updated_at)
               VALUES($1,$2,$3,'insert',$4,'[]'::jsonb,$5,$6,$7,$8,$9,$10,NULL,$11,$12,$13,$14,$15,now(),now())
               RETURNING id"#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .bind(format!("{}:{}", occurred_at.to_rfc3339(), merchant))
        .bind(selected)
        .bind(vec![standard_row_id])
        .bind(occurred_at)
        .bind(amount_cents.abs())
        .bind(direction)
        .bind(transaction_type)
        .bind(account_id)
        .bind(category_id)
        .bind(&merchant)
        .bind(&payment_method)
        .bind(&description)
        .bind(&preview_payload)
        .fetch_one(&mut **tx)
        .await?;
        sqlx::query("UPDATE import_decision_group_members SET preview_row_id=$1 WHERE id=$2")
            .bind(id)
            .bind(member.try_get::<i64, _>("member_id")?)
            .execute(&mut **tx)
            .await?;
        new_ids.push(id);
        let mut response_item = preview_payload;
        if let Some(object) = response_item.as_object_mut() {
            object.insert("id".into(), json!(id));
            object.insert("version".into(), json!(1));
            object.insert("preview_selected".into(), json!(selected));
        }
        items.push(response_item);
    }
    sqlx::query("DELETE FROM import_preview_rows WHERE id=$1 AND session_id=$2 AND user_id=$3")
        .bind(old_preview_id)
        .bind(session_db_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE import_decision_groups SET base_preview_row_id=$1, signal_payload=jsonb_set(jsonb_set(signal_payload, '{suppressed}', 'true'::jsonb, true), '{reclassification}', jsonb_build_object('status','pending','attempts',0,'preview_ids',to_jsonb($2::bigint[])), true) WHERE id=$3")
        .bind(new_ids.first().copied()).bind(&new_ids).bind(group_id).execute(&mut **tx).await?;
    sqlx::query("UPDATE import_sessions SET total_preview=(SELECT count(*) FROM import_preview_rows WHERE session_id=$1 AND user_id=$2), version=version+1, updated_at=now() WHERE id=$1 AND user_id=$2")
        .bind(session_db_id).bind(user_id).execute(&mut **tx).await?;
    Ok(ImportDecisionGroupMutation {
        group_id,
        group_version,
        decision_status: "rejected".into(),
        removed_preview_ids: vec![old_preview_id],
        upserted_preview_ids: new_ids,
        upserted_preview_items: items,
    })
}

pub fn claim_import_group_reclassification(
    pool: &PostgresPool,
    user_id: UserId,
    group_id: i64,
) -> DbResult<Option<String>> {
    block_on_db(async move {
        let row = sqlx::query(r#"UPDATE import_decision_groups SET signal_payload=jsonb_set(signal_payload,'{reclassification}',COALESCE(signal_payload->'reclassification','{}'::jsonb)||jsonb_build_object('status','running','attempts',COALESCE((signal_payload#>>'{reclassification,attempts}')::int,0)+1,'error',NULL,'owner_token',md5(random()::text||clock_timestamp()::text||id::text),'lease_expires_at',(clock_timestamp()+interval '5 minutes')),true),updated_at=now() WHERE id=$1 AND user_id=$2 AND (COALESCE(signal_payload#>>'{reclassification,status}','pending') IN ('pending','failed') OR (signal_payload#>>'{reclassification,status}'='running' AND COALESCE((signal_payload#>>'{reclassification,lease_expires_at}')::timestamptz,'epoch'::timestamptz)<=clock_timestamp())) RETURNING signal_payload#>>'{reclassification,owner_token}' AS owner_token"#)
            .bind(group_id).bind(user_id_i64(user_id)?).fetch_optional(pool).await?;
        row.map(|row| row.try_get("owner_token"))
            .transpose()
            .map_err(Into::into)
    })
}

pub fn get_import_group_reclassification_state(
    pool: &PostgresPool,
    user_id: UserId,
    group_id: i64,
    operation_id: &str,
) -> DbResult<(String, Vec<Value>)> {
    let operation_id = operation_id.to_string();
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let row = sqlx::query(
            r#"SELECT COALESCE(g.signal_payload#>>'{reclassification,status}','pending') AS status,
                      COALESCE(o.payload#>'{result,upserted_preview_items}','[]'::jsonb) AS items
               FROM import_decision_groups g
               LEFT JOIN import_confirm_operations o
                 ON o.user_id=g.user_id AND o.session_id=g.session_id
                AND o.operation_kind='decision_group' AND o.operation_id=$3
                AND o.payload->>'group_id'=g.id::text
               WHERE g.id=$1 AND g.user_id=$2"#,
        )
        .bind(group_id)
        .bind(user_id)
        .bind(operation_id.trim())
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            return Err(DbError::InvalidOperation(
                "decision group reclassification state missing".into(),
            ));
        };
        let status: String = row.try_get("status")?;
        let items: Value = row.try_get("items")?;
        Ok((status, items.as_array().cloned().unwrap_or_default()))
    })
}

#[allow(clippy::too_many_arguments)]
pub fn finish_import_group_reclassification(
    pool: &PostgresPool,
    user_id: UserId,
    group_id: i64,
    operation_id: &str,
    owner_token: &str,
    status: &str,
    error: Option<&str>,
    items: &[Value],
) -> DbResult<()> {
    if status == "completed" && items.is_empty() {
        return Err(DbError::InvalidOperation(
            "decision group reclassification produced no preview items".into(),
        ));
    }
    let operation_id = operation_id.to_string();
    let owner_token = owner_token.to_string();
    let status = status.to_string();
    let error = error.map(str::to_string);
    let items = items.to_vec();
    block_on_db(async move {
        let mut tx = pool.begin().await?;
        let user_id = user_id_i64(user_id)?;
        let group_updated = sqlx::query(r#"UPDATE import_decision_groups SET signal_payload=jsonb_set(signal_payload,'{reclassification}',COALESCE(signal_payload->'reclassification','{}'::jsonb)||jsonb_build_object('status',$1::text,'error',$2::text,'lease_expires_at',NULL),true),updated_at=now() WHERE id=$3 AND user_id=$4 AND signal_payload#>>'{reclassification,status}'='running' AND signal_payload#>>'{reclassification,owner_token}'=$5"#)
            .bind(&status).bind(&error).bind(group_id).bind(user_id).bind(&owner_token).execute(&mut *tx).await?.rows_affected();
        if group_updated != 1 {
            tx.rollback().await?;
            return Err(DbError::InvalidOperation(
                "decision group reclassification lease conflict".into(),
            ));
        }
        if status == "completed" {
            let ledger_updated = sqlx::query(r#"UPDATE import_confirm_operations SET status='completed', payload=jsonb_set(payload,'{result,upserted_preview_items}',$1::jsonb,true) WHERE user_id=$2 AND operation_kind='decision_group' AND operation_id=$3 AND payload->>'group_id'=$4::text"#)
                .bind(json!(items)).bind(user_id).bind(&operation_id).bind(group_id).execute(&mut *tx).await?.rows_affected();
            if ledger_updated != 1 {
                tx.rollback().await?;
                return Err(DbError::InvalidOperation("decision group reclassification operation ledger conflict".into()));
            }
        } else {
            let ledger_updated = sqlx::query(r#"UPDATE import_confirm_operations SET status='failed', payload=jsonb_set(payload,'{reclassification_error}',to_jsonb($1::text),true) WHERE user_id=$2 AND operation_kind='decision_group' AND operation_id=$3 AND payload->>'group_id'=$4::text"#)
                .bind(error.as_deref().unwrap_or("unknown error")).bind(user_id).bind(&operation_id).bind(group_id).execute(&mut *tx).await?.rows_affected();
            if ledger_updated != 1 {
                tx.rollback().await?;
                return Err(DbError::InvalidOperation("decision group reclassification operation ledger conflict".into()));
            }
        }
        tx.commit().await?;
        Ok(())
    })
}

pub fn set_import_decision_materialization_status(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    status: &str,
) -> DbResult<()> {
    block_on_db(async move {
        sqlx::query("UPDATE import_sessions SET metadata=jsonb_set(metadata, '{decision_group_materialization}', jsonb_build_object('status',$1::text), true), updated_at=now() WHERE session_key=$2 AND user_id=$3")
            .bind(status).bind(session_id).bind(user_id_i64(user_id)?)
            .execute(pool).await?;
        Ok(())
    })
}

pub fn set_import_decision_materialization_failed(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    error: &str,
) -> DbResult<()> {
    block_on_db(async move {
        sqlx::query("UPDATE import_sessions SET metadata=jsonb_set(metadata, '{decision_group_materialization}', jsonb_build_object('status','failed','error',$1::text), true), updated_at=now() WHERE session_key=$2 AND user_id=$3")
            .bind(error).bind(session_id).bind(user_id_i64(user_id)?)
            .execute(pool).await?;
        Ok(())
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
