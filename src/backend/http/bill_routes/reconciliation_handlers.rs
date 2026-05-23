// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

async fn reconciliation_statements_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<ReconciliationStatementsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let params = match parse_reconciliation_query(
        query.account_id.as_deref(),
        query.start_time,
        query.end_time,
        query.category_ids.as_deref(),
        query.transaction_type,
        query.keyword.as_deref(),
    ) {
        Ok(value) => value,
        Err(response) => return route_contract_response(response),
    };

    match reconciliation_statement_payload(runtime.connection(), user_id, &params) {
        Ok(Some(result)) => success_result(StatusCode::OK, result),
        Ok(None) => route_contract_response(reconciliation_account_not_found_response()),
        Err(error) => {
            route_contract_response(reconciliation_internal_error_response(error.to_string()))
        }
    }
}

struct ReconciliationAccountRow {
    name: String,
    initial_balance: Money,
}

const RECONCILIATION_QUERY_LIMIT: usize = 10_000;
const RECONCILIATION_QUERY_PAGE_SIZE: usize = 500;

fn reconciliation_statement_payload(
    connection: &Connection,
    user_id: UserId,
    params: &ReconciliationQueryParams,
) -> bill_analyser_db::DbResult<Option<Value>> {
    let Some(account) = load_reconciliation_account(connection, user_id, params.account_id_int)?
    else {
        return Ok(None);
    };
    let categories = load_reconciliation_category_records(connection, user_id)?;
    let category_filters =
        reconciliation_category_filters(params.category_ids.as_deref(), &categories);
    let filters = BillFilters {
        date_from: params.start_date.clone(),
        date_to: params.end_date.clone(),
        transaction_type: reconciliation_type_filter(params.transaction_type_code)
            .map(ToString::to_string),
        keyword: non_empty_string(params.keyword.as_ref()),
        account_ids: vec![params.account_id_int],
        categories: category_filters
            .iter()
            .map(|category| BillCategoryFilter {
                main: category.main.clone(),
                sub: non_empty_string(Some(&category.sub)),
            })
            .collect(),
        ..BillFilters::default()
    };
    let bills = query_reconciliation_bill_records(connection, user_id, &filters)?;
    let opening_snapshots = load_reconciliation_opening_snapshots(connection, user_id, params)?;
    let opening_balance =
        reconciliation_opening_balance(params, account.initial_balance, &opening_snapshots);
    let reconciliation_bills = bills
        .iter()
        .map(record_to_reconciliation_bill)
        .collect::<bill_analyser_db::DbResult<Vec<_>>>()?;
    let summary = calculate_reconciliation_summary(
        params.account_id_int,
        opening_balance,
        &reconciliation_bills,
    )
    .map_err(runtime_error)?;
    let mut frontend_transactions = Vec::with_capacity(summary.balance_history.len());
    for bill in bills {
        let bill_id = record_i64(&bill, "id").unwrap_or_default().to_string();
        if summary.balance_history.contains_key(&bill_id) {
            frontend_transactions.push(record_to_frontend_value(connection, user_id, bill)?);
        }
    }
    let transactions =
        build_reconciliation_transactions(frontend_transactions, &summary.balance_history);
    reconciliation_result_payload(params, account.name, &summary, transactions)
        .map(Some)
        .map_err(runtime_error)
}

fn load_reconciliation_account(
    connection: &Connection,
    user_id: UserId,
    account_id: i64,
) -> bill_analyser_db::DbResult<Option<ReconciliationAccountRow>> {
    let row = connection
        .query_row(
            "SELECT name, initial_balance FROM accounts WHERE id = ?1 AND user_id = ?2",
            params![account_id, user_id.get() as i64],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                ))
            },
        )
        .optional()?;
    row.map(|(name, initial_balance)| {
        Ok(ReconciliationAccountRow {
            name,
            initial_balance: Money::from_yuan_str(&python_float_text(initial_balance))
                .map_err(runtime_error)?,
        })
    })
    .transpose()
}

fn load_reconciliation_category_records(
    connection: &Connection,
    user_id: UserId,
) -> bill_analyser_db::DbResult<Vec<ReconciliationCategoryRecord>> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "SELECT id, main_category, sub_category FROM categories WHERE user_id = ?1 ORDER BY id ASC",
    )?;
    let rows = statement.query_map(params![user_id.get() as i64], |row| {
        Ok(ReconciliationCategoryRecord {
            id: row.get::<_, i64>(0)?,
            main_category: row.get::<_, String>(1)?,
            sub_category: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        })
    })?;
    let mut categories = Vec::new();
    for row in rows {
        categories.push(row?);
    }
    Ok(categories)
}

fn query_reconciliation_bill_records(
    connection: &Connection,
    user_id: UserId,
    filters: &BillFilters,
) -> bill_analyser_db::DbResult<Vec<BillRecord>> {
    let mut bills = Vec::new();
    let mut page = 1;
    while bills.len() < RECONCILIATION_QUERY_LIMIT {
        let remaining = RECONCILIATION_QUERY_LIMIT - bills.len();
        let page_size = remaining.min(RECONCILIATION_QUERY_PAGE_SIZE);
        let bill_page = query_bills(connection, user_id, page, page_size, filters)?;
        let fetched = bill_page.bills.len();
        let total = usize::try_from(bill_page.total.max(0)).unwrap_or(usize::MAX);
        bills.extend(bill_page.bills);
        if fetched == 0 || fetched < page_size || bills.len() >= total {
            break;
        }
        page += 1;
    }
    Ok(bills)
}

fn load_reconciliation_opening_snapshots(
    connection: &Connection,
    user_id: UserId,
    params: &ReconciliationQueryParams,
) -> bill_analyser_db::DbResult<Vec<ReconciliationOpeningBalanceSnapshot>> {
    let Some(start_date) = params.start_date.clone() else {
        return Ok(Vec::new());
    };
    let previous_page = query_bills(
        connection,
        user_id,
        1,
        1,
        &BillFilters {
            date_to: Some(start_date),
            account_ids: vec![params.account_id_int],
            ..BillFilters::default()
        },
    )?;
    let snapshot = previous_page
        .bills
        .first()
        .map(|record| optional_money_from_record(record, "account_balance"))
        .transpose()?
        .flatten()
        .map(|account_balance| ReconciliationOpeningBalanceSnapshot { account_balance });
    Ok(snapshot.into_iter().collect())
}

fn record_to_reconciliation_bill(
    record: &BillRecord,
) -> bill_analyser_db::DbResult<ReconciliationBill> {
    Ok(ReconciliationBill {
        id: record_i64(record, "id").unwrap_or_default().to_string(),
        date: record_text(record, "date"),
        transaction_type: frontend_transaction_type_from_backend(&record_text(record, "type")).ok(),
        amount: money_from_record(record, "amount")?,
        source_account_id: positive_record_i64(record, "source_account_id"),
        destination_account_id: positive_record_i64(record, "destination_account_id"),
    })
}
