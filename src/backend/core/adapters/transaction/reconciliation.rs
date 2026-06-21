/// 返回单笔账单需要重算余额的账户集合。
pub fn sync_account_ids_for_bill(snapshot: BillAccountSyncSnapshot) -> Vec<i64> {
    collect_account_ids([snapshot])
}

/// 返回更新账单前后都需要重算余额的账户集合。
pub fn sync_account_ids_for_update(
    old_bill: BillAccountSyncSnapshot,
    new_bill: BillAccountSyncSnapshot,
) -> Vec<i64> {
    collect_account_ids([old_bill, new_bill])
}

/// 返回批量删除账单后需要重算余额的账户集合。
pub fn sync_account_ids_for_batch_delete(bills: &[BillAccountSyncSnapshot]) -> Vec<i64> {
    collect_account_ids(bills.iter().copied())
}

/// 按收入、支出、转账、投资方向重新计算指定账户余额。
pub fn calculate_account_balance_from_bills(
    account_id: i64,
    initial_balance: Money,
    bills: &[AccountBalanceBill],
    same_account_investment_pnl_correction: Money,
) -> Result<Money, RuntimeError> {
    let mut income = 0_i128;
    let mut expense = 0_i128;
    let mut transfer_out = 0_i128;
    let mut transfer_in = 0_i128;
    let mut investment_out = 0_i128;
    let mut investment_in = 0_i128;

    for bill in bills {
        if bill.source_account_id == Some(account_id) {
            match bill.transaction_type {
                TransactionType::Income => income += i128::from(bill.amount.to_cents()),
                TransactionType::Expense => expense += i128::from(bill.amount.to_cents()),
                TransactionType::Transfer => transfer_out += i128::from(bill.amount.to_cents()),
                TransactionType::Investment => investment_out += i128::from(bill.amount.to_cents()),
            }
        }
        if bill.destination_account_id == Some(account_id) {
            match bill.transaction_type {
                TransactionType::Transfer => {
                    transfer_in += i128::from(bill.destination_amount.to_cents());
                }
                TransactionType::Investment => {
                    investment_in += i128::from(bill.destination_amount.to_cents());
                }
                TransactionType::Income | TransactionType::Expense => {}
            }
        }
    }

    let balance = i128::from(initial_balance.to_cents()) + income - expense - transfer_out
        + transfer_in
        - investment_out
        + investment_in
        + i128::from(same_account_investment_pnl_correction.to_cents());
    money_from_i128_cents(balance)
}

/// 解析对账单查询参数，并把缺失或非法输入映射成稳定路由响应。
pub fn parse_reconciliation_query(
    account_id: Option<&str>,
    start_time: Option<i64>,
    end_time: Option<i64>,
    category_ids: Option<&str>,
    transaction_type_code: Option<i64>,
    keyword: Option<&str>,
) -> Result<ReconciliationQueryParams, RouteResponseContract> {
    let (Some(account_id), Some(start_time), Some(end_time)) = (account_id, start_time, end_time)
    else {
        return Err(missing_reconciliation_parameters_response());
    };

    let (start_date, end_date) = if start_time == 0 && end_time == 0 {
        (None, None)
    } else {
        let start_date = reconciliation_date_from_timestamp(start_time).ok_or_else(|| {
            reconciliation_internal_error_response("invalid reconciliation start_time")
        })?;
        let end_date = reconciliation_date_from_timestamp(end_time).ok_or_else(|| {
            reconciliation_internal_error_response("invalid reconciliation end_time")
        })?;
        (Some(start_date), Some(end_date))
    };

    let account_id_int = account_id
        .trim()
        .parse::<i64>()
        .map_err(|_| invalid_reconciliation_account_id_response(account_id))?;

    Ok(ReconciliationQueryParams {
        account_id: account_id.to_string(),
        account_id_int,
        start_time,
        end_time,
        start_date,
        end_date,
        category_ids: category_ids.map(str::to_string),
        transaction_type_code,
        keyword: keyword.map(str::to_string),
    })
}

/// 将前端分类筛选 id 映射成对账查询使用的主/子分类筛选条件。
pub fn reconciliation_category_filters(
    raw_category_ids: Option<&str>,
    categories: &[ReconciliationCategoryRecord],
) -> Vec<ReconciliationCategoryFilter> {
    let Some(raw_category_ids) = raw_category_ids.filter(|value| !value.trim().is_empty()) else {
        return Vec::new();
    };
    let mut selected_ids = BTreeSet::new();
    for raw_id in raw_category_ids.split(',') {
        let Ok(category_id) = raw_id.trim().parse::<i64>() else {
            return Vec::new();
        };
        selected_ids.insert(category_id);
    }
    if selected_ids.is_empty() {
        return Vec::new();
    }

    categories
        .iter()
        .filter(|category| selected_ids.contains(&category.id))
        .map(|category| ReconciliationCategoryFilter {
            main: category.main_category.clone(),
            sub: category.sub_category.clone(),
        })
        .collect()
}

/// 将前端交易类型编码映射为后端对账筛选类型名。
pub fn reconciliation_type_filter(transaction_type_code: Option<i64>) -> Option<&'static str> {
    match transaction_type_code {
        Some(1) => Some(TransactionType::Income.backend_name()),
        Some(2) => Some(TransactionType::Expense.backend_name()),
        Some(3) => Some(TransactionType::Transfer.backend_name()),
        Some(4) => Some(TransactionType::Investment.backend_name()),
        _ => None,
    }
}

/// 构造对账查询复用的账单筛选 JSON，保持与正式账单列表筛选一致。
pub fn build_reconciliation_filters(
    params: &ReconciliationQueryParams,
    category_filters: &[ReconciliationCategoryFilter],
) -> Value {
    let mut filters = Map::new();
    filters.insert(
        "account_ids".to_string(),
        Value::Array(vec![Value::Number(Number::from(params.account_id_int))]),
    );
    if let (Some(start_date), Some(end_date)) = (&params.start_date, &params.end_date) {
        filters.insert("start_date".to_string(), Value::String(start_date.clone()));
        filters.insert("end_date".to_string(), Value::String(end_date.clone()));
    }
    if !category_filters.is_empty() {
        filters.insert(
            "categories".to_string(),
            serde_json::to_value(category_filters)
                .expect("reconciliation category filters should serialize"),
        );
    }
    if let Some(transaction_type) = reconciliation_type_filter(params.transaction_type_code) {
        filters.insert(
            "type".to_string(),
            Value::String(transaction_type.to_string()),
        );
    }
    if let Some(keyword) = params.keyword.as_ref().filter(|value| !value.is_empty()) {
        filters.insert("keyword".to_string(), Value::String(keyword.clone()));
    }
    Value::Object(filters)
}

/// 根据查询是否指定开始日期决定对账期初余额来源。
pub fn reconciliation_opening_balance(
    params: &ReconciliationQueryParams,
    account_initial_balance: Money,
    previous_bills: &[ReconciliationOpeningBalanceSnapshot],
) -> Money {
    if params.start_date.is_some() {
        previous_bills
            .first()
            .map(|bill| bill.account_balance)
            .unwrap_or(Money::ZERO)
    } else {
        account_initial_balance
    }
}

/// 逐笔回放对账账单，计算期末余额、流入流出和每笔开收盘余额。
pub fn calculate_reconciliation_summary(
    account_id: i64,
    opening_balance: Money,
    bills: &[ReconciliationBill],
) -> Result<ReconciliationSummary, RuntimeError> {
    let mut sorted_bills = bills.iter().collect::<Vec<_>>();
    sorted_bills.sort_by(|left, right| left.date.cmp(&right.date));

    let mut total_inflows = 0_i128;
    let mut total_outflows = 0_i128;
    let mut current_balance = i128::from(opening_balance.to_cents());
    let mut balance_history = BTreeMap::new();

    for bill in sorted_bills {
        let Some(transaction_type) = bill.transaction_type else {
            continue;
        };
        let amount = abs_cents_i128(bill.amount);
        let transaction_opening_balance = current_balance;

        match transaction_type {
            TransactionType::Income => {
                total_inflows += amount;
                current_balance += amount;
            }
            TransactionType::Expense => {
                total_outflows += amount;
                current_balance -= amount;
            }
            TransactionType::Transfer | TransactionType::Investment => {
                if bill.destination_account_id == Some(account_id) {
                    total_inflows += amount;
                    current_balance += amount;
                } else if bill.source_account_id == Some(account_id) {
                    total_outflows += amount;
                    current_balance -= amount;
                } else if transaction_type == TransactionType::Transfer {
                    continue;
                }
            }
        }

        balance_history.insert(
            bill.id.clone(),
            ReconciliationBalanceEntry {
                opening: money_from_i128_cents(transaction_opening_balance)?,
                closing: money_from_i128_cents(current_balance)?,
            },
        );
    }

    Ok(ReconciliationSummary {
        opening_balance,
        closing_balance: money_from_i128_cents(current_balance)?,
        total_inflows: money_from_i128_cents(total_inflows)?,
        total_outflows: money_from_i128_cents(total_outflows)?,
        balance_history,
    })
}

/// 把余额回放结果回填到前端交易数组，并按时间倒序输出。
pub fn build_reconciliation_transactions(
    frontend_transactions: Vec<Value>,
    balance_history: &BTreeMap<String, ReconciliationBalanceEntry>,
) -> Vec<Value> {
    let mut transactions = frontend_transactions
        .into_iter()
        .filter_map(|mut transaction| {
            let transaction_id = value_string(transaction.get("id"))?;
            let balance = balance_history.get(&transaction_id)?;
            let Value::Object(transaction_map) = &mut transaction else {
                return None;
            };
            transaction_map.insert(
                "accountOpeningBalanceCents".to_string(),
                Value::Number(Number::from(balance.opening.to_cents())),
            );
            transaction_map.insert(
                "accountClosingBalanceCents".to_string(),
                Value::Number(Number::from(balance.closing.to_cents())),
            );
            Some(transaction)
        })
        .collect::<Vec<_>>();
    transactions.sort_by(|left, right| {
        let left_time = value_to_i64(left.get("time"), 0).unwrap_or(0);
        let right_time = value_to_i64(right.get("time"), 0).unwrap_or(0);
        right_time.cmp(&left_time)
    });
    transactions
}

/// 组装对账接口最终 payload，统一 cents 汇总字段和交易明细。
pub fn reconciliation_result_payload(
    params: &ReconciliationQueryParams,
    account_name: impl Into<String>,
    summary: &ReconciliationSummary,
    transactions: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let net_flow = i128::from(summary.total_inflows.to_cents())
        - i128::from(summary.total_outflows.to_cents());
    let result = ReconciliationResult {
        account_id: params.account_id.clone(),
        account_name: account_name.into(),
        start_time: params.start_time,
        end_time: params.end_time,
        opening_balance_cents: summary.opening_balance.to_cents(),
        closing_balance_cents: summary.closing_balance.to_cents(),
        total_inflows_cents: summary.total_inflows.to_cents(),
        total_outflows_cents: summary.total_outflows.to_cents(),
        net_flow_cents: money_from_i128_cents(net_flow)?.to_cents(),
        item_count: transactions.len(),
        transactions,
    };
    serde_json::to_value(result).map_err(|error| {
        RuntimeError::new(
            ErrorCode::InternalError,
            format!("failed to serialize reconciliation result: {error}"),
        )
    })
}

/// 生成对账接口成功响应。
pub fn reconciliation_success_response(
    params: &ReconciliationQueryParams,
    account_name: impl Into<String>,
    summary: &ReconciliationSummary,
    transactions: Vec<Value>,
) -> Result<RouteResponseContract, RuntimeError> {
    Ok(RouteResponseContract {
        status_code: 200,
        body: success_result_body(reconciliation_result_payload(
            params,
            account_name,
            summary,
            transactions,
        )?),
    })
}

/// 生成对账接口缺少必填参数时的错误响应。
pub fn missing_reconciliation_parameters_response() -> RouteResponseContract {
    simple_route_error_response(
        400,
        "Missing required parameters: account_id, start_time, end_time",
    )
}

/// 生成对账接口账户 id 非法时的错误响应。
pub fn invalid_reconciliation_account_id_response(
    account_id: impl ToString,
) -> RouteResponseContract {
    simple_route_error_response(
        400,
        format!("Invalid account_id: {}", account_id.to_string()),
    )
}

/// 生成对账账户不存在时的错误响应。
pub fn reconciliation_account_not_found_response() -> RouteResponseContract {
    simple_route_error_response(404, "Account not found")
}

/// 生成对账内部错误响应，并保留前端依赖的 message 文案。
pub fn reconciliation_internal_error_response(error: impl Into<String>) -> RouteResponseContract {
    let mut body = Map::new();
    body.insert("success".to_string(), Value::Bool(false));
    body.insert("error".to_string(), Value::String(error.into()));
    body.insert(
        "message".to_string(),
        Value::String("Failed to retrieve reconciliation statements".to_string()),
    );
    RouteResponseContract {
        status_code: 500,
        body: Value::Object(body),
    }
}
