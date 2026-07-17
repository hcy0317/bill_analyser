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
