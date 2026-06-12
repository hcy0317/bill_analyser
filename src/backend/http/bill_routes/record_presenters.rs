// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。
async fn page_to_frontend_postgres(
    pool: &PostgresPool,
    user_id: UserId,
    page: usize,
    page_size: usize,
    bill_page: bill_analyser_db::BillPage,
) -> bill_analyser_db::DbResult<Value> {
    let mut items = Vec::with_capacity(bill_page.bills.len());
    for bill in bill_page.bills {
        let bill_id = record_i64(&bill, "id").unwrap_or_default();
        let tags = get_postgres_bill_tags(pool, user_id.get() as i64, bill_id).await?;
        let category_id = value_string(bill.get("category_id"));
        items.push(record_to_frontend_value_with_related(bill, tags, category_id)?);
    }
    let total = usize::try_from(bill_page.total.max(0)).unwrap_or(usize::MAX);
    Ok(json!({
        "success": true,
        "result": {
            "items": items,
            "totalCount": bill_page.total,
            "page": page,
            "pageSize": page_size,
            "total": bill_page.total,
            "page_size": page_size,
            "total_pages": total.div_ceil(page_size),
        }
    }))
}
async fn get_postgres_frontend_bill(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
) -> bill_analyser_db::DbResult<Option<Value>> {
    let Some(record) = get_postgres_bill_by_id(pool, user_id.get() as i64, bill_id).await? else {
        return Ok(None);
    };
    let tags = get_postgres_bill_tags(pool, user_id.get() as i64, bill_id).await?;
    let category_id = value_string(record.get("category_id"));
    record_to_frontend_value_with_related(record, tags, category_id).map(Some)
}
fn record_to_frontend_value_with_related(
    record: BillRecord,
    tags: Vec<Value>,
    category_id: Option<String>,
) -> bill_analyser_db::DbResult<Value> {
    let bill_id = record_i64(&record, "id").unwrap_or(0);
    let frontend_tags = tags
        .iter()
        .filter_map(frontend_tag_from_value)
        .collect::<Vec<_>>();
    let tag_ids = frontend_tags.iter().map(|tag| tag.id.clone()).collect();
    let bill = BackendTransactionView {
        id: bill_id.to_string(),
        time_sequence_id: None,
        transaction_type: frontend_transaction_type_from_backend(&record_text(&record, "type"))
            .map_err(runtime_error)?,
        category_id,
        main_category: record_text(&record, "main_category"),
        sub_category: record_text(&record, "sub_category"),
        date: record_text(&record, "date"),
        amount: money_from_record(&record, "amount_cents")?,
        destination_amount: Some(money_from_record(&record, "destination_amount_cents")?),
        source_account_id: positive_record_i64(&record, "source_account_id"),
        destination_account_id: positive_record_i64(&record, "destination_account_id"),
        utc_offset: UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
        hide_amount: false,
        tag_ids,
        tags: frontend_tags,
        category: None,
        source_account: None,
        destination_account: None,
        description: record_text(&record, "description"),
    };
    serde_json::to_value(frontend_transaction_from_backend(&bill))
        .map_err(|error| bill_analyser_db::DbError::InvalidOperation(error.to_string()))
}

#[cfg(test)]
mod record_presenter_tests {
    use super::*;

    #[test]
    fn record_to_frontend_value_with_related_preserves_cents_tags_category_and_accounts() {
        let mut record = Map::new();
        record.insert("id".to_string(), json!(42));
        record.insert("type".to_string(), json!("expense"));
        record.insert("main_category".to_string(), json!("餐饮"));
        record.insert("sub_category".to_string(), json!("咖啡"));
        record.insert("date".to_string(), json!("2026-04-02 09:00:00"));
        record.insert("amount_cents".to_string(), json!(12345));
        record.insert("destination_amount_cents".to_string(), json!(12500));
        record.insert("source_account_id".to_string(), json!(10));
        record.insert("destination_account_id".to_string(), json!(20));
        record.insert("description".to_string(), json!("B007 咖啡"));

        let value = record_to_frontend_value_with_related(
            record,
            vec![json!({"id": "7", "name": "咖啡标签"})],
            Some("5".to_string()),
        )
        .expect("frontend bill value");

        assert_eq!(value["id"], "42");
        assert_eq!(value["categoryId"], "5");
        assert_eq!(value["categoryName"], "餐饮");
        assert_eq!(value["subCategoryName"], "咖啡");
        assert_eq!(value["sourceAccountId"], "10");
        assert_eq!(value["destinationAccountId"], "20");
        assert_eq!(value["amountCents"], 12345);
        assert_eq!(value["sourceAmountCents"], 12345);
        assert_eq!(value["destinationAmountCents"], 12500);
        assert_eq!(value["tagIds"], json!(["7"]));
        assert_eq!(value["tags"], json!([{"id": "7", "name": "咖啡标签"}]));
        assert_eq!(value["comment"], "B007 咖啡");
    }
}
