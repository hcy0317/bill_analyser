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
             AND sr.session_id=$2 AND sr.user_id=$3
           FOR UPDATE OF sr, m"#,
    )
    .bind(group_id)
    .bind(session_db_id)
    .bind(user_id)
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
    sqlx::query("UPDATE import_decision_groups SET decision_status='suppressed',base_preview_row_id=$1,signal_payload=jsonb_build_object('suppressed',true,'reclassification',jsonb_build_object('status','pending','attempts',0,'preview_ids',jsonb_build_array($1))) WHERE id=$2 AND session_id=$3 AND user_id=$4")
        .bind(new_id).bind(group_id).bind(session_db_id).bind(user_id).execute(&mut **tx).await?;
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
        r#"SELECT selected, preview_payload, category_id, account_id,
                  transfer_target_account_id
           FROM import_preview_rows
           WHERE id=$1 AND session_id=$2 AND user_id=$3"#,
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
             AND sr.session_id=$2 AND sr.user_id=$3
           ORDER BY CASE m.member_role WHEN 'outgoing' THEN 0 ELSE 1 END, m.id
           FOR UPDATE OF sr, m"#,
    )
    .bind(group_id)
    .bind(session_db_id)
    .bind(user_id)
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
        let merchant = member
            .try_get::<Option<String>, _>("merchant")?
            .unwrap_or_default();
        let payment_method = member
            .try_get::<Option<String>, _>("payment_method")?
            .unwrap_or_default();
        let description = member
            .try_get::<Option<String>, _>("description")?
            .unwrap_or_default();
        let parser_payload: Value = member.try_get("parser_payload")?;
        let outgoing = role == "outgoing";
        let keep_category =
            outgoing && manual_fields.get("category_id").and_then(Value::as_bool) == Some(true);
        let keep_source_account = outgoing
            && manual_fields
                .get("source_account_id")
                .and_then(Value::as_bool)
                == Some(true);
        let keep_destination_as_incoming_source = !outgoing
            && manual_fields
                .get("destination_account_id")
                .and_then(Value::as_bool)
                == Some(true);
        let category_id = if keep_category {
            old.try_get::<Option<i64>, _>("category_id")?
        } else {
            None
        };
        let account_id = if keep_source_account {
            old.try_get::<Option<i64>, _>("account_id")?
        } else if keep_destination_as_incoming_source {
            old.try_get::<Option<i64>, _>("transfer_target_account_id")?
        } else {
            None
        };
        let replacement_manual_fields = if outgoing {
            json!({
                "category_id": keep_category,
                "source_account_id": keep_source_account,
                "destination_account_id": false
            })
        } else {
            json!({
                "category_id": false,
                "source_account_id": keep_destination_as_incoming_source,
                "destination_account_id": false
            })
        };
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
                "annotation": {
                    "is_manually_annotated": keep_category
                        || keep_source_account
                        || keep_destination_as_incoming_source,
                    "manual_fields": replacement_manual_fields
                },
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
    sqlx::query("UPDATE import_decision_groups SET base_preview_row_id=$1, signal_payload=jsonb_set(jsonb_set(signal_payload, '{suppressed}', 'true'::jsonb, true), '{reclassification}', jsonb_build_object('status','pending','attempts',0,'preview_ids',to_jsonb($2::bigint[])), true) WHERE id=$3 AND session_id=$4 AND user_id=$5")
        .bind(new_ids.first().copied()).bind(&new_ids).bind(group_id).bind(session_db_id).bind(user_id).execute(&mut **tx).await?;
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
