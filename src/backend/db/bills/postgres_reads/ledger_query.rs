#[derive(Debug, Clone)]
pub struct PostgresLedgerQueries {
    pool: PostgresPool,
}

const LEDGER_PAGE_SIZE_LIMIT: usize = 500;
const MONTH_LEDGER_SIZE_LIMIT: usize = 100_000;

impl PostgresLedgerQueries {
    pub fn new(pool: &PostgresPool) -> Self {
        Self { pool: pool.clone() }
    }

    /// 查询当前用户的正式账本，并在 repository 内完成筛选解析、行映射与标签聚合。
    pub async fn list(
        &self,
        principal: UserId,
        query: LedgerListQuery,
    ) -> DbResult<LedgerEntryPage> {
        self.list_with_page_size_limit(principal, query, LEDGER_PAGE_SIZE_LIMIT)
            .await
    }

    /// 查询一个有明确日期边界的完整月份，并拒绝静默返回超过有界上限的部分集合。
    pub async fn list_month(
        &self,
        principal: UserId,
        mut query: LedgerListQuery,
    ) -> DbResult<LedgerEntryPage> {
        if query.date_from.is_none() || query.date_to.is_none() {
            return Err(DbError::InvalidOperation(
                "monthly ledger query requires a bounded date range".to_string(),
            ));
        }
        query.page = 1;
        query.page_size = MONTH_LEDGER_SIZE_LIMIT;
        let page = self
            .list_with_page_size_limit(principal, query, MONTH_LEDGER_SIZE_LIMIT)
            .await?;
        if page.total > i64::try_from(MONTH_LEDGER_SIZE_LIMIT).unwrap_or(i64::MAX) {
            return Err(DbError::InvalidOperation(format!(
                "monthly ledger exceeds bounded read limit: {}",
                MONTH_LEDGER_SIZE_LIMIT
            )));
        }
        Ok(page)
    }

    async fn list_with_page_size_limit(
        &self,
        principal: UserId,
        query: LedgerListQuery,
        max_page_size: usize,
    ) -> DbResult<LedgerEntryPage> {
        let user_id = i64::try_from(principal.get())
            .map_err(|_| DbError::InvalidOperation("user id exceeds PostgreSQL BIGINT".to_string()))?;
        let categories = postgres_category_filters_for_ids(
            &self.pool,
            user_id,
            &query.category_ids,
        )
        .await?;
        let filters = BillFilters {
            date_from: query.date_from,
            date_to: query.date_to,
            transaction_type: query.transaction_type,
            flow_direction: query.flow_direction,
            main_category: query.main_category,
            sub_category: query.sub_category,
            batch_id: query.batch_id,
            counterparty: query.counterparty,
            description: query.description,
            keyword: query.keyword,
            account_ids: query.account_ids,
            categories,
            tag_ids: query.tag_ids,
            amount_filter_cents: query.amount_filter_cents,
            ..BillFilters::default()
        };
        let page = query_postgres_bills_with_page_size_limit(
            &self.pool,
            user_id,
            query.page,
            query.page_size,
            max_page_size,
            &filters,
        )
        .await?;
        let bill_ids = page
            .bills
            .iter()
            .map(|record| required_ledger_i64(record, "id"))
            .collect::<DbResult<Vec<_>>>()?;
        let mut tags_by_bill = ledger_tags_by_bill_ids(&self.pool, user_id, &bill_ids).await?;
        let items = page
            .bills
            .into_iter()
            .map(|record| {
                let bill_id = required_ledger_i64(&record, "id")?;
                ledger_entry_from_record(
                    record,
                    tags_by_bill.remove(&bill_id).unwrap_or_default(),
                )
            })
            .collect::<DbResult<Vec<_>>>()?;

        Ok(LedgerEntryPage {
            items,
            total: page.total,
            page: query.page.max(1),
            page_size: query.page_size.clamp(1, max_page_size.max(1)),
        })
    }
}

async fn ledger_tags_by_bill_ids(
    pool: &PostgresPool,
    user_id: i64,
    bill_ids: &[i64],
) -> DbResult<std::collections::HashMap<i64, Vec<LedgerTag>>> {
    if bill_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows = sqlx::query(
        r#"
        SELECT bt.bill_id, t.id, t.name
        FROM bill_tags bt
        JOIN tags t ON t.user_id = bt.user_id AND t.id = bt.tag_id
        WHERE bt.user_id = $1 AND bt.bill_id = ANY($2)
        ORDER BY bt.bill_id ASC, t.display_order ASC, t.name ASC
        "#,
    )
    .bind(user_id)
    .bind(bill_ids)
    .fetch_all(pool)
    .await?;
    let mut tags_by_bill = std::collections::HashMap::<i64, Vec<LedgerTag>>::new();
    for row in rows {
        tags_by_bill
            .entry(row.try_get("bill_id")?)
            .or_default()
            .push(LedgerTag {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
            });
    }
    Ok(tags_by_bill)
}

fn ledger_entry_from_record(record: BillRecord, tags: Vec<LedgerTag>) -> DbResult<LedgerEntry> {
    let transaction_type = TransactionType::from_backend_name(&super::record_text(&record, "type"))
        .map_err(|error| DbError::InvalidOperation(error.to_string()))?;
    Ok(LedgerEntry {
        id: required_ledger_i64(&record, "id")?,
        transaction_type,
        category_id: positive_ledger_i64(&record, "category_id"),
        main_category: super::record_text(&record, "main_category"),
        sub_category: super::record_text(&record, "sub_category"),
        date: super::record_text(&record, "date"),
        amount: Money::from_cents(required_ledger_i64(&record, "amount_cents")?),
        destination_amount: Money::from_cents(required_ledger_i64(
            &record,
            "destination_amount_cents",
        )?),
        source_account_id: positive_ledger_i64(&record, "source_account_id"),
        destination_account_id: positive_ledger_i64(&record, "destination_account_id"),
        tags,
        description: super::record_text(&record, "description"),
    })
}

fn required_ledger_i64(record: &BillRecord, key: &str) -> DbResult<i64> {
    record
        .get(key)
        .and_then(value_to_i64)
        .ok_or_else(|| DbError::InvalidOperation(format!("missing integer ledger field: {key}")))
}

fn positive_ledger_i64(record: &BillRecord, key: &str) -> Option<i64> {
    record.get(key).and_then(value_to_i64).filter(|value| *value > 0)
}
