// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[derive(Debug, Default, Deserialize)]
struct BillsListQuery {
    page: Option<usize>,
    count: Option<usize>,
    page_size: Option<usize>,
    #[serde(rename = "pageSize")]
    page_size_camel: Option<usize>,
    #[serde(rename = "type")]
    transaction_type: Option<String>,
    main_category: Option<String>,
    sub_category: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    min_time: Option<i64>,
    max_time: Option<i64>,
    keyword: Option<String>,
    batch_id: Option<String>,
    counterparty: Option<String>,
    description: Option<String>,
    account_ids: Option<String>,
    #[serde(rename = "accountIds")]
    account_ids_camel: Option<String>,
    category_ids: Option<String>,
    #[serde(rename = "categoryIds")]
    category_ids_camel: Option<String>,
    tag_ids: Option<String>,
    #[serde(rename = "tagIds")]
    tag_ids_camel: Option<String>,
    amount_filter: Option<String>,
    #[serde(rename = "amountFilter")]
    amount_filter_camel: Option<String>,
}

impl BillsListQuery {
    fn page(&self) -> usize {
        self.page.unwrap_or(1).max(1)
    }

    fn page_size(&self) -> usize {
        self.page_size
            .or(self.page_size_camel)
            .or(self.count)
            .unwrap_or(20)
            .clamp(1, 500)
    }

    fn account_ids(&self) -> Vec<i64> {
        parse_csv_i64(
            self.account_ids
                .as_ref()
                .or(self.account_ids_camel.as_ref()),
        )
    }

    fn category_ids(&self) -> Vec<i64> {
        parse_csv_i64(
            self.category_ids
                .as_ref()
                .or(self.category_ids_camel.as_ref()),
        )
    }

    fn tag_ids(&self) -> Vec<i64> {
        parse_csv_i64(self.tag_ids.as_ref().or(self.tag_ids_camel.as_ref()))
    }

    fn amount_filter(&self) -> Option<String> {
        non_empty_string(
            self.amount_filter
                .as_ref()
                .or(self.amount_filter_camel.as_ref()),
        )
    }
}

#[derive(Debug, Deserialize)]
struct BillsByMonthQuery {
    year: i32,
    month: u32,
    #[serde(flatten)]
    common: BillsListQuery,
}

#[derive(Debug, Default, Deserialize)]
struct BillIdQuery {
    id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BillsExportQuery {
    format: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ReconciliationStatementsQuery {
    account_id: Option<String>,
    start_time: Option<i64>,
    end_time: Option<i64>,
    category_ids: Option<String>,
    #[serde(rename = "type")]
    transaction_type: Option<i64>,
    keyword: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RecurringCandidatesQuery {
    #[serde(rename = "toleranceDays")]
    tolerance_days: Option<i64>,
}

fn filters_from_query(
    connection: &Connection,
    user_id: UserId,
    query: &BillsListQuery,
) -> RouteResult<BillFilters> {
    let category_ids = query.category_ids();
    let categories = if category_ids.is_empty() {
        Vec::new()
    } else {
        category_filters_for_ids(connection, user_id, &category_ids)
            .map_err(|_| Box::new(db_error_response()))?
    };
    Ok(BillFilters {
        date_from: query
            .start_date
            .clone()
            .or_else(|| query.min_time.and_then(date_from_timestamp)),
        date_to: query
            .end_date
            .clone()
            .or_else(|| query.max_time.and_then(date_from_timestamp)),
        transaction_type: transaction_list_type_filter(query.transaction_type.as_deref()),
        main_category: non_empty_string(query.main_category.as_ref()),
        sub_category: non_empty_string(query.sub_category.as_ref()),
        batch_id: non_empty_string(query.batch_id.as_ref()),
        counterparty: non_empty_string(query.counterparty.as_ref()),
        description: non_empty_string(query.description.as_ref()),
        keyword: non_empty_string(query.keyword.as_ref()),
        account_ids: query.account_ids(),
        categories,
        tag_ids: query.tag_ids(),
        amount_filter: query.amount_filter(),
        ..BillFilters::default()
    })
}

async fn postgres_filters_from_query(
    pool: &PostgresPool,
    user_id: UserId,
    query: &BillsListQuery,
) -> RouteResult<BillFilters> {
    let category_ids = query.category_ids();
    let categories = if category_ids.is_empty() {
        Vec::new()
    } else {
        postgres_category_filters_for_ids(pool, user_id.get() as i64, &category_ids)
            .await
            .map_err(|_| Box::new(db_error_response()))?
    };
    Ok(BillFilters {
        date_from: query
            .start_date
            .clone()
            .or_else(|| query.min_time.and_then(date_from_timestamp)),
        date_to: query
            .end_date
            .clone()
            .or_else(|| query.max_time.and_then(date_from_timestamp)),
        transaction_type: transaction_list_type_filter(query.transaction_type.as_deref()),
        main_category: non_empty_string(query.main_category.as_ref()),
        sub_category: non_empty_string(query.sub_category.as_ref()),
        batch_id: non_empty_string(query.batch_id.as_ref()),
        counterparty: non_empty_string(query.counterparty.as_ref()),
        description: non_empty_string(query.description.as_ref()),
        keyword: non_empty_string(query.keyword.as_ref()),
        account_ids: query.account_ids(),
        categories,
        tag_ids: query.tag_ids(),
        amount_filter: query.amount_filter(),
        ..BillFilters::default()
    })
}
