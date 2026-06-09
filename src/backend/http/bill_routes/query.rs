// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
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

#[cfg(test)]
mod bill_query_tests {
    use super::*;

    #[test]
    fn bills_list_query_pins_paging_filter_aliases_and_amount_filter() {
        let query = BillsListQuery {
            page: Some(0),
            count: Some(25),
            page_size: None,
            page_size_camel: Some(50),
            account_ids: Some(" 2,0,bad,3,2".to_string()),
            account_ids_camel: Some("9".to_string()),
            category_ids: None,
            category_ids_camel: Some(" 4,5,invalid ".to_string()),
            tag_ids: Some("7,,0,8".to_string()),
            amount_filter: None,
            amount_filter_camel: Some(" between:10:20 ".to_string()),
            ..BillsListQuery::default()
        };

        assert_eq!(query.page(), 1);
        assert_eq!(query.page_size(), 50, "pageSize takes precedence over count");
        assert_eq!(query.account_ids(), vec![2, 3, 2]);
        assert_eq!(query.category_ids(), vec![4, 5]);
        assert_eq!(query.tag_ids(), vec![7, 8]);
        assert_eq!(query.amount_filter().as_deref(), Some("between:10:20"));

        let clamped = BillsListQuery {
            page: Some(4),
            count: Some(100),
            page_size: Some(999),
            page_size_camel: Some(10),
            amount_filter: Some(" gte:200 ".to_string()),
            amount_filter_camel: Some("ignored".to_string()),
            ..BillsListQuery::default()
        };
        assert_eq!(clamped.page(), 4);
        assert_eq!(clamped.page_size(), 500);
        assert_eq!(clamped.amount_filter().as_deref(), Some("gte:200"));
    }
}
