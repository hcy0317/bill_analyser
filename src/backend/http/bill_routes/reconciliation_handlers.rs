// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn reconciliation_statements_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<ReconciliationStatementsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "reconciliation_statements_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
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


        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match reconciliation_statement_payload_postgres(runtime.pool(), user_id, &params)
            .await
        {
            Ok(Some(result)) => success_result(StatusCode::OK, result),
            Ok(None) => route_contract_response(reconciliation_account_not_found_response()),
            Err(error) => route_contract_response(reconciliation_internal_error_response(
                error.to_string(),
            )),
        };
}

const RECONCILIATION_QUERY_LIMIT: usize = 10_000;
const RECONCILIATION_QUERY_PAGE_SIZE: usize = 500;

async fn reconciliation_statement_payload_postgres(
    pool: &PostgresPool,
    user_id: UserId,
    params: &ReconciliationQueryParams,
) -> bill_analyser_db::DbResult<Option<Value>> {
    let Some(account) = get_postgres_reconciliation_account(
        pool,
        user_id.get() as i64,
        params.account_id_int,
    )
    .await?
    else {
        return Ok(None);
    };
    let categories = list_postgres_reconciliation_categories(pool, user_id.get() as i64).await?;
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
    let bills = query_postgres_reconciliation_bill_records(pool, user_id, &filters).await?;
    let opening_snapshots = load_postgres_reconciliation_opening_snapshots(
        pool,
        user_id,
        params,
        account.initial_balance,
    )
    .await?;
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
        let bill_id = record_i64(&bill, "id").unwrap_or_default();
        if summary.balance_history.contains_key(&bill_id.to_string()) {
            let tags = get_postgres_bill_tags(pool, user_id.get() as i64, bill_id).await?;
            let category_id = value_string(bill.get("category_id"));
            frontend_transactions.push(record_to_frontend_value_with_related(
                bill,
                tags,
                category_id,
            )?);
        }
    }
    let transactions =
        build_reconciliation_transactions(frontend_transactions, &summary.balance_history);
    reconciliation_result_payload(params, account.name, &summary, transactions)
        .map(Some)
        .map_err(runtime_error)
}

async fn query_postgres_reconciliation_bill_records(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &BillFilters,
) -> bill_analyser_db::DbResult<Vec<BillRecord>> {
    let mut bills = Vec::new();
    let mut page = 1;
    while bills.len() < RECONCILIATION_QUERY_LIMIT {
        let remaining = RECONCILIATION_QUERY_LIMIT - bills.len();
        let page_size = remaining.min(RECONCILIATION_QUERY_PAGE_SIZE);
        let bill_page =
            query_postgres_bills(pool, user_id.get() as i64, page, page_size, filters).await?;
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

async fn load_postgres_reconciliation_opening_snapshots(
    pool: &PostgresPool,
    user_id: UserId,
    params: &ReconciliationQueryParams,
    initial_balance: Money,
) -> bill_analyser_db::DbResult<Vec<ReconciliationOpeningBalanceSnapshot>> {
    let Some(start_date) = params.start_date.clone() else {
        return Ok(Vec::new());
    };
    let previous_bills = query_postgres_reconciliation_bill_records(
        pool,
        user_id,
        &BillFilters {
            date_before: Some(start_date),
            account_ids: vec![params.account_id_int],
            ..BillFilters::default()
        },
    )
    .await?;
    if previous_bills.is_empty() {
        return Ok(Vec::new());
    }
    let reconciliation_bills = previous_bills
        .iter()
        .map(record_to_reconciliation_bill)
        .collect::<bill_analyser_db::DbResult<Vec<_>>>()?;
    let summary = calculate_reconciliation_summary(
        params.account_id_int,
        initial_balance,
        &reconciliation_bills,
    )
    .map_err(runtime_error)?;
    Ok(vec![ReconciliationOpeningBalanceSnapshot {
        account_balance: summary.closing_balance,
    }])
}
fn record_to_reconciliation_bill(
    record: &BillRecord,
) -> bill_analyser_db::DbResult<ReconciliationBill> {
    Ok(ReconciliationBill {
        id: record_i64(record, "id").unwrap_or_default().to_string(),
        date: record_text(record, "date"),
        transaction_type: frontend_transaction_type_from_backend(&record_text(record, "type")).ok(),
        amount: money_from_record(record, "amount_cents")?,
        source_account_id: positive_record_i64(record, "source_account_id"),
        destination_account_id: positive_record_i64(record, "destination_account_id"),
    })
}
