/// 返回后端账单写入合同使用的交易类型中文名。
pub fn backend_transaction_type_name(transaction_type: TransactionType) -> &'static str {
    transaction_type.backend_name()
}

/// 把后端历史值、英文别名或旧数字编码归一为前端交易类型枚举。
pub fn frontend_transaction_type_from_backend(
    raw_value: &str,
) -> Result<TransactionType, RuntimeError> {
    TransactionType::from_backend_name(raw_value)
}

/// 按支出、转账、投资的后端存储语义把来源金额转换为负数。
pub fn signed_backend_amount(transaction_type: TransactionType, source_amount: Money) -> Money {
    if matches!(
        transaction_type,
        TransactionType::Expense | TransactionType::Transfer | TransactionType::Investment
    ) && source_amount.is_positive()
    {
        source_amount
            .checked_negated()
            .expect("positive money values can always be negated")
    } else {
        source_amount
    }
}

/// 将前端创建/编辑 payload 转成后端账单 mutation 数据和标签、账户、分类元数据。
pub fn frontend_transaction_mutation_to_backend(
    frontend_data: &Value,
    _utc_offset: UtcOffsetMinutes,
) -> Result<(Map<String, Value>, BillMutationMetadata), RuntimeError> {
    let transaction_type = frontend_transaction_type_from_value(frontend_data.get("type"))?;
    let source_amount = Money::from_cents(frontend_amount_cents(
        frontend_data.get("sourceAmountCents"),
    )?);
    let destination_amount = Money::from_cents(frontend_amount_cents(
        frontend_data.get("destinationAmountCents"),
    )?);
    let amount = match transaction_type {
        Some(
            TransactionType::Expense | TransactionType::Transfer | TransactionType::Investment,
        ) if source_amount.is_positive() => source_amount.checked_negated()?,
        _ => source_amount,
    };
    let source_account_id = value_to_i64(frontend_data.get("sourceAccountId"), 0)?;
    let destination_account_id = value_to_i64(frontend_data.get("destinationAccountId"), 0)?;
    let unix_time = normalize_frontend_unix_time(frontend_data.get("time"));
    let date = if unix_time == 0 {
        String::new()
    } else {
        frontend_unix_time_to_backend_date(
            unix_time,
            UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
        )?
    };
    let description = value_string(
        frontend_data
            .get("comment")
            .or_else(|| frontend_data.get("remark")),
    )
    .unwrap_or_default();

    let mut backend_data = Map::new();
    backend_data.insert(
        "type".to_string(),
        Value::String(
            transaction_type
                .map(TransactionType::backend_name)
                .unwrap_or_default()
                .to_string(),
        ),
    );
    backend_data.insert("date".to_string(), Value::String(date));
    backend_data.insert(
        "amount_cents".to_string(),
        Value::Number(Number::from(amount.to_cents())),
    );
    backend_data.insert(
        "destination_amount_cents".to_string(),
        Value::Number(Number::from(destination_amount.to_cents())),
    );
    backend_data.insert(
        "source_account_id".to_string(),
        Value::Number(Number::from(source_account_id)),
    );
    backend_data.insert(
        "destination_account_id".to_string(),
        Value::Number(Number::from(destination_account_id)),
    );
    backend_data.insert("description".to_string(), Value::String(description));

    let metadata = BillMutationMetadata {
        category_id: metadata_string(frontend_data.get("categoryId")),
        source_account_id,
        destination_account_id,
        tag_ids: tag_ids_from_value(frontend_data.get("tagIds"))?,
        auto_invest_account: false,
    };
    Ok((backend_data, metadata))
}

/// 为手工创建交易补齐描述、交易对方和默认来源账户等后端必填字段。
pub fn apply_manual_create_defaults(
    backend_data: &mut Map<String, Value>,
    frontend_data: &Value,
    fallback_source_account_id: Option<i64>,
) -> Result<(), RuntimeError> {
    let description_fallback =
        first_non_empty_string(frontend_data, &["comment", "remark", "description"])
            .unwrap_or_default();
    let counterparty_fallback = first_non_empty_string(
        frontend_data,
        &[
            "counterparty",
            "payee",
            "merchant",
            "merchantName",
            "shopName",
            "targetAccountName",
        ],
    )
    .or_else(|| (!description_fallback.is_empty()).then_some(description_fallback.clone()))
    .or_else(|| non_empty_map_string(backend_data, "payment_method"))
    .unwrap_or_else(|| "手工录入".to_string());

    if non_empty_map_string(backend_data, "description").is_none() {
        backend_data.insert(
            "description".to_string(),
            Value::String(counterparty_fallback.clone()),
        );
    }
    if non_empty_map_string(backend_data, "counterparty").is_none() {
        backend_data.insert(
            "counterparty".to_string(),
            Value::String(counterparty_fallback),
        );
    }

    let source_account_id = map_i64(backend_data, "source_account_id")?;
    if source_account_id <= 0 {
        let fallback = fallback_source_account_id
            .filter(|value| *value > 0)
            .ok_or_else(|| RuntimeError::new(ErrorCode::InvalidInput, "No account available"))?;
        backend_data.insert(
            "source_account_id".to_string(),
            Value::Number(Number::from(fallback)),
        );
    }

    Ok(())
}

/// 应用创建时的分类合同：优先使用已解析分类，其次规则命中，最后按类型兜底。
pub fn apply_create_category_contract(
    backend_data: &mut Map<String, Value>,
    resolved_category: Option<(&str, &str)>,
    rule_matched_category: Option<(&str, &str)>,
) {
    if let Some((main_category, sub_category)) = resolved_category.or(rule_matched_category) {
        backend_data.insert(
            "main_category".to_string(),
            Value::String(main_category.to_string()),
        );
        backend_data.insert(
            "sub_category".to_string(),
            Value::String(sub_category.to_string()),
        );
        return;
    }

    if non_empty_map_string(backend_data, "main_category").is_some() {
        return;
    }

    let bill_type =
        non_empty_map_string(backend_data, "type").unwrap_or_else(|| "支出".to_string());
    let (main_category, sub_category) = default_bill_category(&bill_type);
    backend_data.insert(
        "main_category".to_string(),
        Value::String(main_category.to_string()),
    );
    backend_data.insert(
        "sub_category".to_string(),
        Value::String(sub_category.to_string()),
    );
}
