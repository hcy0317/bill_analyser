const PREVIEW_FILTER_NONE_VALUE: &str = "__none__";
const PREVIEW_FILTER_INVALID_VALUE: &str = "__invalid__";

pub fn insert_preview_bill(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    draft: &ImportPreviewDraft,
) -> DbResult<i64> {
    insert_preview_bill_on_connection(connection, session_id, user_id, draft)?;
    Ok(connection.last_insert_rowid())
}

pub fn insert_preview_bills_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportPreviewDraft],
) -> DbResult<usize> {
    if drafts.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let created_at = now_text();
        let user_id = user_id_i64(user_id)?;
        let mut statement = tx.prepare(INSERT_PREVIEW_BILL_SQL)?;
        for draft in drafts {
            insert_preview_bill_with_statement(&mut statement, session_id, user_id, draft, &created_at)?;
        }
        Ok(drafts.len())
    })
}

pub fn get_preview_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<Vec<ImportPreviewRow>> {
    let mut query =
        "SELECT * FROM bills_preview WHERE session_id = ?1 AND user_id = ?2".to_string();
    if selected_only {
        query.push_str(" AND preview_selected = 1");
    }
    query.push_str(" ORDER BY preview_date ASC, id ASC");

    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map(params![session_id, user_id_i64(user_id)?], preview_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn count_preview_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<i64> {
    let selected_clause = if selected_only {
        " AND preview_selected = 1"
    } else {
        ""
    };
    connection
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM bills_preview WHERE session_id = ?1 AND user_id = ?2{selected_clause}"
            ),
            params![session_id, user_id_i64(user_id)?],
            |row| row.get(0),
        )
        .map_err(DbError::from)
}

pub fn get_preview_page_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    page: i64,
    page_size: i64,
    selected_only: bool,
) -> DbResult<(Vec<ImportPreviewRow>, i64)> {
    let normalized_page = page.max(1);
    let normalized_page_size = page_size.max(1);
    let offset = (normalized_page - 1) * normalized_page_size;
    let selected_clause = if selected_only {
        " AND preview_selected = 1"
    } else {
        ""
    };
    let total: i64 = connection.query_row(
        &format!(
            "SELECT COUNT(*) FROM bills_preview WHERE session_id = ?1 AND user_id = ?2{selected_clause}"
        ),
        params![session_id, user_id_i64(user_id)?],
        |row| row.get(0),
    )?;
    let mut statement = connection.prepare(&format!(
        "SELECT * FROM bills_preview WHERE session_id = ?1 AND user_id = ?2{selected_clause} \
         ORDER BY preview_date ASC, id ASC LIMIT ?3 OFFSET ?4"
    ))?;
    let rows = statement.query_map(
        params![
            session_id,
            user_id_i64(user_id)?,
            normalized_page_size,
            offset
        ],
        preview_from_row,
    )?;
    Ok((rows.collect::<Result<Vec<_>, _>>()?, total))
}

pub fn query_preview_page_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    request: &ImportPreviewPageRequest,
) -> DbResult<ImportPreviewPageResult> {
    if !request.preview_ids.is_empty() {
        let mut rows = get_preview_by_ids(connection, session_id, &request.preview_ids, user_id)?;
        if !request.sort_by.is_empty() {
            sort_preview_rows(rows.as_mut_slice(), &request.sort_by, &request.sort_direction);
        }
        let rows = apply_preview_filters(rows, &request.filters);
        let metadata = build_preview_metadata(&rows);
        return Ok(ImportPreviewPageResult {
            total: rows.len(),
            rows,
            page: request.page.max(1),
            page_size: request.page_size.max(1),
            metadata,
        });
    }

    let page = request.page.max(1);
    let page_size = request.page_size.max(1);
    let total = count_preview_rows_by_query(connection, session_id, user_id, &request.filters)?;
    let metadata = build_preview_metadata_by_query(
        connection,
        session_id,
        user_id,
        &request.filters,
        total,
    )?;
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    let rows = get_preview_rows_by_query(
        connection,
        session_id,
        user_id,
        &request.filters,
        Some((&request.sort_by, &request.sort_direction)),
        Some(page_size),
        Some(offset),
    )?;
    Ok(ImportPreviewPageResult {
        rows,
        total,
        page,
        page_size,
        metadata,
    })
}

fn count_preview_rows_by_query(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    filters: &ImportPreviewQueryFilters,
) -> DbResult<usize> {
    let query = build_preview_sql_query("COUNT(*)", session_id, user_id, filters, None, None, None)?;
    let count: i64 = connection.query_row(
        query.sql.as_str(),
        params_from_iter(query.params.iter()),
        |row| row.get(0),
    )?;
    Ok(usize::try_from(count.max(0)).unwrap_or(usize::MAX))
}

fn get_preview_rows_by_query(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    filters: &ImportPreviewQueryFilters,
    sort: Option<(&str, &str)>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> DbResult<Vec<ImportPreviewRow>> {
    let query = build_preview_sql_query("*", session_id, user_id, filters, sort, limit, offset)?;
    let mut statement = connection.prepare(query.sql.as_str())?;
    let rows = statement.query_map(params_from_iter(query.params.iter()), preview_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

struct PreviewSqlQuery {
    sql: String,
    params: Vec<SqlValue>,
}

struct PreviewSqlBuildOptions<'a> {
    select: &'a str,
    from: &'a str,
    session_id: &'a str,
    user_id: UserId,
    filters: &'a ImportPreviewQueryFilters,
    sort: Option<(&'a str, &'a str)>,
    limit: Option<usize>,
    offset: Option<usize>,
}

fn build_preview_sql_query(
    select: &str,
    session_id: &str,
    user_id: UserId,
    filters: &ImportPreviewQueryFilters,
    sort: Option<(&str, &str)>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> DbResult<PreviewSqlQuery> {
    build_preview_sql_query_from(PreviewSqlBuildOptions {
        select,
        from: "bills_preview",
        session_id,
        user_id,
        filters,
        sort,
        limit,
        offset,
    })
}

fn build_preview_sql_query_from(options: PreviewSqlBuildOptions<'_>) -> DbResult<PreviewSqlQuery> {
    let mut clauses = vec!["session_id = ?".to_string(), "user_id = ?".to_string()];
    let mut params = vec![
        SqlValue::Text(options.session_id.to_string()),
        SqlValue::Integer(user_id_i64(options.user_id)?),
    ];
    append_preview_sql_filters(&mut clauses, &mut params, options.filters);

    let mut sql = format!(
        "SELECT {select} FROM {from} WHERE {}",
        clauses.join(" AND "),
        select = options.select,
        from = options.from,
    );
    if let Some((sort_by, sort_direction)) = options.sort {
        sql.push_str(" ORDER BY ");
        sql.push_str(preview_sql_sort_column(sort_by));
        if sort_direction.eq_ignore_ascii_case("desc") {
            sql.push_str(" DESC");
        } else {
            sql.push_str(" ASC");
        }
        sql.push_str(", id ASC");
    }
    if let Some(limit) = options.limit {
        sql.push_str(" LIMIT ?");
        params.push(SqlValue::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
    }
    if let Some(offset) = options.offset {
        sql.push_str(" OFFSET ?");
        params.push(SqlValue::Integer(i64::try_from(offset).unwrap_or(i64::MAX)));
    }
    Ok(PreviewSqlQuery { sql, params })
}

fn append_preview_sql_filters(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    filters: &ImportPreviewQueryFilters,
) {
    if filters.selected_only {
        clauses.push("preview_selected = 1".to_string());
    }
    if let Some(value) = filters.min_datetime.as_deref().filter(|value| !value.trim().is_empty()) {
        clauses.push("REPLACE(REPLACE(TRIM(preview_date), '/', '-'), 'T', ' ') >= ?".to_string());
        params.push(SqlValue::Text(normalized_preview_date_key(value)));
    }
    if let Some(value) = filters.max_datetime.as_deref().filter(|value| !value.trim().is_empty()) {
        clauses.push("REPLACE(REPLACE(TRIM(preview_date), '/', '-'), 'T', ' ') <= ?".to_string());
        params.push(SqlValue::Text(normalized_preview_date_key(value)));
    }
    append_preview_type_sql_filter(clauses, params, filters.transaction_type.as_deref());
    append_category_sql_filter(clauses, params, filters.category.as_deref());
    append_account_sql_filter(clauses, params, filters.account.as_deref());
    append_tag_sql_filter(clauses, params, filters.tag.as_deref());
    append_description_sql_filter(clauses, params, filters.description.as_deref());
    append_signal_sql_filter(clauses, params, filters.signal.as_deref());
    append_annotation_sql_filter(clauses, params, filters.annotation.as_deref());
}

fn append_preview_type_sql_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    value: Option<&str>,
) {
    let Some(filter) = value.map(normalized_text).filter(|value| !value.is_empty()) else {
        return;
    };
    if filter == PREVIEW_FILTER_NONE_VALUE || filter == PREVIEW_FILTER_INVALID_VALUE {
        clauses.push("COALESCE(TRIM(preview_type), '') = ''".to_string());
        return;
    }
    if let Some(type_code) = preview_type_code(&filter) {
        clauses.push(preview_type_code_sql_clause());
        params.push(SqlValue::Integer(type_code));
    } else {
        clauses.push("LOWER(COALESCE(preview_type, '')) LIKE ?".to_string());
        params.push(SqlValue::Text(sql_like_contains(&filter)));
    }
}

fn append_category_sql_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    value: Option<&str>,
) {
    let Some(filter) = value.map(normalized_text).filter(|value| !value.is_empty()) else {
        return;
    };
    if filter == PREVIEW_FILTER_NONE_VALUE {
        clauses.push(
            "(COALESCE(TRIM(preview_main_category), '') = '' AND COALESCE(TRIM(preview_sub_category), '') = '')"
                .to_string(),
        );
        return;
    }
    if filter == PREVIEW_FILTER_INVALID_VALUE {
        clauses.push(category_issue_sql_clause());
        return;
    }
    clauses.push(
        "(LOWER(COALESCE(preview_main_category, '')) LIKE ? OR LOWER(COALESCE(preview_sub_category, '')) LIKE ?)"
            .to_string(),
    );
    let like = SqlValue::Text(sql_like_contains(&filter));
    params.push(like.clone());
    params.push(like);
}

fn append_account_sql_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    value: Option<&str>,
) {
    let Some(filter) = value.map(normalized_text).filter(|value| !value.is_empty()) else {
        return;
    };
    if filter == PREVIEW_FILTER_NONE_VALUE {
        clauses.push(
            "(preview_source_account_id IS NULL AND preview_destination_account_id IS NULL)"
                .to_string(),
        );
        return;
    }
    if filter == PREVIEW_FILTER_INVALID_VALUE {
        clauses.push(account_issue_sql_clause());
        return;
    }
    if let Ok(account_id) = filter.parse::<i64>() {
        clauses.push(
            "(preview_source_account_id = ? OR preview_destination_account_id = ?)".to_string(),
        );
        params.push(SqlValue::Integer(account_id));
        params.push(SqlValue::Integer(account_id));
    }
}

fn append_tag_sql_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    value: Option<&str>,
) {
    let Some(filter) = value.map(normalized_text).filter(|value| !value.is_empty()) else {
        return;
    };
    if filter == PREVIEW_FILTER_NONE_VALUE || filter == PREVIEW_FILTER_INVALID_VALUE {
        clauses.push(
            "(COALESCE(TRIM(preview_parser_tags_json), '') = '' OR TRIM(preview_parser_tags_json) = '[]')"
                .to_string(),
        );
        return;
    }
    clauses.push("LOWER(COALESCE(preview_parser_tags_json, '')) LIKE ?".to_string());
    params.push(SqlValue::Text(sql_like_contains(&filter)));
}

fn append_description_sql_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    value: Option<&str>,
) {
    let Some(filter) = value.map(normalized_text).filter(|value| !value.is_empty()) else {
        return;
    };
    if filter == PREVIEW_FILTER_NONE_VALUE {
        clauses.push(
            "(COALESCE(TRIM(preview_description), '') = '' AND COALESCE(TRIM(preview_counterparty), '') = '' AND COALESCE(TRIM(preview_payment_method), '') = '')"
                .to_string(),
        );
        return;
    }
    clauses.push(
        "(LOWER(COALESCE(preview_description, '')) LIKE ? OR LOWER(COALESCE(preview_counterparty, '')) LIKE ? OR LOWER(COALESCE(preview_payment_method, '')) LIKE ?)"
            .to_string(),
    );
    let like = SqlValue::Text(sql_like_contains(&filter));
    params.push(like.clone());
    params.push(like.clone());
    params.push(like);
}

fn append_signal_sql_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    value: Option<&str>,
) {
    let Some(signal) = value.map(normalized_text).filter(|value| !value.is_empty()) else {
        return;
    };

    match signal.as_str() {
        "parser" => {
            clauses.push(format!(
                "(COALESCE(TRIM(preview_parser_id), '') <> '' OR EXISTS (SELECT 1 FROM json_each({}) AS signal_tag WHERE TRIM(CAST(signal_tag.value AS TEXT)) <> ''))",
                parser_tags_json_sql_expression()
            ));
        }
        "platform_duplicate" => {
            clauses.push("LOWER(COALESCE(dedup_type, '')) = 'platform_bank'".to_string());
        }
        "transfer" => {
            clauses.push(format!(
                "(LOWER(COALESCE(dedup_type, '')) LIKE '%transfer%' OR json_type({}, '$.transfer') IS NOT NULL)",
                matching_feedback_json_sql_expression()
            ));
        }
        "learning" => {
            clauses.push(format!(
                "json_type({}, '$.learning') IS NOT NULL",
                matching_feedback_json_sql_expression()
            ));
        }
        _ => {
            clauses.push(format!(
                "(LOWER(COALESCE(dedup_type, '')) LIKE ? \
                  OR LOWER(COALESCE(preview_parser_id, '')) LIKE ? \
                  OR EXISTS (SELECT 1 FROM json_each({tags_json}) AS signal_tag WHERE LOWER(CAST(signal_tag.value AS TEXT)) LIKE ?) \
                  OR EXISTS (
                    SELECT 1 FROM json_each({feedback_json}) AS signal_feedback
                    WHERE LOWER(CAST(signal_feedback.key AS TEXT)) LIKE ?
                       OR LOWER(CAST(signal_feedback.value AS TEXT)) LIKE ?
                       OR EXISTS (
                            SELECT 1 FROM json_each(CASE WHEN json_valid(signal_feedback.value) THEN signal_feedback.value ELSE '[]' END) AS signal_nested
                            WHERE LOWER(CAST(signal_nested.value AS TEXT)) LIKE ?
                       )
                  ))",
                tags_json = parser_tags_json_sql_expression(),
                feedback_json = matching_feedback_json_sql_expression()
            ));
            let like = SqlValue::Text(sql_like_contains(&signal));
            for _ in 0..6 {
                params.push(like.clone());
            }
        }
    }
}

fn append_annotation_sql_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    value: Option<&str>,
) {
    let Some(annotation) = value.map(normalized_text).filter(|value| !value.is_empty()) else {
        return;
    };

    if annotation == "needs-review" {
        clauses.push(preview_annotation_issue_sql_clause());
        return;
    }
    if annotation == "no-issues" {
        clauses.push(format!("NOT {}", preview_annotation_issue_sql_clause()));
        return;
    }

    clauses.push(format!(
        "EXISTS (
            SELECT 1 FROM json_each({feedback_json}) AS annotation_feedback
            WHERE annotation_feedback.key = 'annotation'
              AND (
                LOWER(CAST(annotation_feedback.value AS TEXT)) LIKE ?
                OR EXISTS (
                    SELECT 1 FROM json_each(CASE WHEN json_valid(annotation_feedback.value) THEN annotation_feedback.value ELSE '[]' END) AS annotation_nested
                    WHERE LOWER(CAST(annotation_nested.value AS TEXT)) LIKE ?
                )
              )
        )",
        feedback_json = matching_feedback_json_sql_expression()
    ));
    let like = SqlValue::Text(sql_like_contains(&annotation));
    params.push(like.clone());
    params.push(like);
}

fn sql_like_contains(value: &str) -> String {
    format!("%{value}%")
}

fn preview_sql_sort_column(sort_by: &str) -> &'static str {
    match sort_by {
        "type" => "preview_type",
        "sourceAmount" => "preview_amount",
        "counterparty" => "preview_counterparty",
        "paymentMethod" => "preview_payment_method",
        "comment" => "preview_description",
        "time" | "" => "preview_date",
        _ => "preview_date",
    }
}

fn preview_type_code_sql_clause() -> String {
    format!("{} = ?", preview_type_code_sql_expression())
}

fn category_issue_sql_clause() -> String {
    "(COALESCE(TRIM(preview_main_category), '') = '' AND COALESCE(TRIM(preview_sub_category), '') = '')"
        .to_string()
}

fn source_account_issue_sql_clause() -> String {
    "preview_source_account_id IS NULL".to_string()
}

fn destination_account_issue_sql_clause() -> String {
    format!(
        "({} IN (4, 5) AND preview_destination_account_id IS NULL)",
        preview_type_code_sql_expression(),
    )
}

fn transfer_account_review_sql_clause() -> String {
    format!(
        "({} IN (4, 5) AND preview_source_account_id IS NOT NULL AND preview_destination_account_id IS NOT NULL AND preview_source_account_id = preview_destination_account_id)",
        preview_type_code_sql_expression(),
    )
}

fn account_issue_sql_clause() -> String {
    format!(
        "({} OR {} OR {})",
        source_account_issue_sql_clause(),
        destination_account_issue_sql_clause(),
        transfer_account_review_sql_clause(),
    )
}

fn parser_tags_json_sql_expression() -> &'static str {
    "CASE WHEN json_valid(COALESCE(preview_parser_tags_json, '')) THEN preview_parser_tags_json ELSE '[]' END"
}

fn matching_feedback_json_sql_expression() -> &'static str {
    "CASE WHEN json_valid(COALESCE(preview_matching_feedback_json, '')) THEN preview_matching_feedback_json ELSE '{}' END"
}

fn feedback_annotation_type_sql_expression() -> String {
    let feedback_json = matching_feedback_json_sql_expression();
    format!(
        "LOWER(TRIM(COALESCE(CASE \
            WHEN json_type({feedback_json}, '$.annotation') = 'text' \
            THEN json_extract({feedback_json}, '$.annotation') \
            ELSE CAST(json_extract({feedback_json}, '$.annotation.type') AS TEXT) \
        END, '')))"
    )
}

fn feedback_annotation_issue_sql_clause() -> String {
    let feedback_json = matching_feedback_json_sql_expression();
    let annotation_type = feedback_annotation_type_sql_expression();
    let category_issue = category_issue_sql_clause();
    let source_account_issue = source_account_issue_sql_clause();
    let destination_account_issue = destination_account_issue_sql_clause();
    let account_issue = account_issue_sql_clause();
    let raw_annotation_issue = format!(
        "((json_type({feedback_json}, '$.annotation') = 'text' AND TRIM(COALESCE(json_extract({feedback_json}, '$.annotation'), '')) <> '') \
          OR TRIM(COALESCE(CAST(json_extract({feedback_json}, '$.annotation.status') AS TEXT), '')) <> '' \
          OR TRIM(COALESCE(CAST(json_extract({feedback_json}, '$.annotation.review_status') AS TEXT), '')) <> '' \
          OR TRIM(COALESCE(CAST(json_extract({feedback_json}, '$.annotation.level') AS TEXT), '')) <> '' \
          OR TRIM(COALESCE(CAST(json_extract({feedback_json}, '$.annotation.type') AS TEXT), '')) <> '')"
    );
    format!(
        "({raw_annotation_issue} AND CASE \
            WHEN {annotation_type} IN ('category', 'category_missing', 'missing_category', 'missing-category', 'missing_classification') THEN {category_issue} \
            WHEN {annotation_type} IN ('account', 'missing_account', 'source_account', 'source_account_missing', 'missing_source_account', 'missing-source-account') THEN {source_account_issue} \
            WHEN {annotation_type} IN ('destination_account', 'destination_account_missing', 'missing_destination_account', 'missing-destination-account') THEN {destination_account_issue} \
            WHEN {annotation_type} IN ('transfer_account_direction', 'transfer_accounts', 'review_transfer_accounts', 'same_transfer_accounts') THEN {account_issue} \
            ELSE 1 \
        END)"
    )
}

fn preview_annotation_issue_sql_clause() -> String {
    format!(
        "({} OR {} OR {})",
        category_issue_sql_clause(),
        account_issue_sql_clause(),
        feedback_annotation_issue_sql_clause(),
    )
}

fn preview_type_code_sql_expression() -> &'static str {
    "CASE LOWER(TRIM(preview_type)) WHEN '收入' THEN 2 WHEN 'income' THEN 2 WHEN '2' THEN 2 WHEN '支出' THEN 3 WHEN 'expense' THEN 3 WHEN '3' THEN 3 WHEN '转账' THEN 4 WHEN 'transfer' THEN 4 WHEN '4' THEN 4 WHEN '投资' THEN 5 WHEN 'investment' THEN 5 WHEN '5' THEN 5 ELSE NULL END"
}

fn apply_preview_filters(
    rows: Vec<ImportPreviewRow>,
    filters: &ImportPreviewQueryFilters,
) -> Vec<ImportPreviewRow> {
    rows.into_iter()
        .filter(|row| preview_row_matches_filters(row, filters))
        .collect()
}

fn preview_row_matches_filters(
    row: &ImportPreviewRow,
    filters: &ImportPreviewQueryFilters,
) -> bool {
    let date_key = normalized_preview_date_key(&row.preview_date);
    if filters
        .min_datetime
        .as_deref()
        .is_some_and(|value| date_key < normalized_preview_date_key(value))
    {
        return false;
    }
    if filters
        .max_datetime
        .as_deref()
        .is_some_and(|value| date_key > normalized_preview_date_key(value))
    {
        return false;
    }
    if !preview_type_filter_matches(filters.transaction_type.as_deref(), &row.preview_type) {
        return false;
    }
    if !category_filter_matches(filters.category.as_deref(), row) {
        return false;
    }
    if !account_filter_matches(filters.account.as_deref(), row) {
        return false;
    }
    if !tag_filter_matches(filters.tag.as_deref(), row) {
        return false;
    }
    if !description_filter_matches(filters.description.as_deref(), row) {
        return false;
    }
    if filters
        .signal
        .as_deref()
        .is_some_and(|signal| !preview_row_matches_signal(row, signal))
    {
        return false;
    }
    if filters
        .annotation
        .as_deref()
        .is_some_and(|annotation| !preview_row_matches_annotation(row, annotation))
    {
        return false;
    }
    true
}

fn preview_type_filter_matches(filter: Option<&str>, preview_type: &str) -> bool {
    let Some(filter) = filter.map(normalized_text).filter(|value| !value.is_empty()) else {
        return true;
    };
    if filter == PREVIEW_FILTER_NONE_VALUE || filter == PREVIEW_FILTER_INVALID_VALUE {
        return preview_type.trim().is_empty();
    }
    preview_type_code(&filter)
        .zip(preview_type_code(preview_type))
        .is_some_and(|(filter_type, row_type)| filter_type == row_type)
        || normalized_text(preview_type).contains(&filter)
}

fn category_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(normalized_text).filter(|value| !value.is_empty()) else {
        return true;
    };
    let has_category =
        !row.preview_main_category.trim().is_empty() || !row.preview_sub_category.trim().is_empty();
    if filter == PREVIEW_FILTER_NONE_VALUE {
        return !has_category;
    }
    if filter == PREVIEW_FILTER_INVALID_VALUE {
        return preview_row_has_category_issues(row);
    }
    text_filter_matches(
        Some(filter.as_str()),
        [&row.preview_main_category, &row.preview_sub_category],
    )
}

fn account_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(normalized_text).filter(|value| !value.is_empty()) else {
        return true;
    };
    let source_account = row
        .preview_source_account_id
        .map(|value| value.to_string())
        .unwrap_or_default();
    let destination_account = row
        .preview_destination_account_id
        .map(|value| value.to_string())
        .unwrap_or_default();
    if filter == PREVIEW_FILTER_NONE_VALUE {
        return source_account.is_empty() && destination_account.is_empty();
    }
    if filter == PREVIEW_FILTER_INVALID_VALUE {
        return preview_row_has_account_issues(row);
    }
    text_filter_matches(Some(filter.as_str()), [&source_account, &destination_account])
}

fn tag_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(normalized_text).filter(|value| !value.is_empty()) else {
        return true;
    };
    if filter == PREVIEW_FILTER_NONE_VALUE || filter == PREVIEW_FILTER_INVALID_VALUE {
        return row
            .preview_parser_tags
            .iter()
            .all(|value| value.trim().is_empty());
    }
    row.preview_parser_tags
        .iter()
        .any(|value| normalized_text(value).contains(&filter))
}

fn description_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(normalized_text).filter(|value| !value.is_empty()) else {
        return true;
    };
    let values = [
        &row.preview_description,
        &row.preview_counterparty,
        &row.preview_payment_method,
    ];
    if filter == PREVIEW_FILTER_NONE_VALUE {
        return values.iter().all(|value| value.trim().is_empty());
    }
    text_filter_matches(Some(filter.as_str()), values)
}

fn preview_type_code(preview_type: &str) -> Option<i64> {
    match preview_type.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => Some(2),
        "支出" | "expense" | "3" => Some(3),
        "转账" | "transfer" | "4" => Some(4),
        "投资" | "investment" | "5" => Some(5),
        _ => None,
    }
}

fn preview_row_has_category_issues(row: &ImportPreviewRow) -> bool {
    preview_type_code(&row.preview_type) != Some(1)
        && preview_type_code(&row.preview_type) != Some(6)
        && row.preview_main_category.trim().is_empty()
        && row.preview_sub_category.trim().is_empty()
}

fn preview_row_has_account_issues(row: &ImportPreviewRow) -> bool {
    preview_row_has_source_account_issues(row)
        || preview_row_has_destination_account_issues(row)
        || preview_row_has_transfer_account_review_issues(row)
}

fn preview_row_has_source_account_issues(row: &ImportPreviewRow) -> bool {
    row.preview_source_account_id.is_none()
}

fn preview_row_has_destination_account_issues(row: &ImportPreviewRow) -> bool {
    let requires_destination = matches!(preview_type_code(&row.preview_type), Some(4) | Some(5));
    requires_destination && row.preview_destination_account_id.is_none()
}

fn preview_row_has_transfer_account_review_issues(row: &ImportPreviewRow) -> bool {
    let requires_destination = matches!(preview_type_code(&row.preview_type), Some(4) | Some(5));
    requires_destination
        && row.preview_source_account_id.is_some()
        && row.preview_destination_account_id.is_some()
        && row.preview_source_account_id == row.preview_destination_account_id
}

fn preview_row_has_annotation_issues(row: &ImportPreviewRow) -> bool {
    preview_row_has_category_issues(row) || preview_row_has_account_issues(row)
}

fn text_filter_matches<'a>(
    filter: Option<&str>,
    values: impl IntoIterator<Item = &'a String>,
) -> bool {
    let Some(filter) = filter.map(normalized_text).filter(|value| !value.is_empty()) else {
        return true;
    };
    values
        .into_iter()
        .any(|value| normalized_text(value).contains(&filter))
}

fn normalized_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalized_preview_date_key(value: &str) -> String {
    value.trim().replace('/', "-").replace('T', " ")
}

fn preview_row_matches_signal(row: &ImportPreviewRow, signal: &str) -> bool {
    let signal = normalized_text(signal);
    if signal.is_empty() {
        return true;
    }
    if signal == "parser" {
        return !row.preview_parser_id.trim().is_empty()
            || row
                .preview_parser_tags
                .iter()
                .any(|tag| !tag.trim().is_empty());
    }
    if signal == "platform_duplicate" {
        return normalized_text(&row.dedup_type) == "platform_bank";
    }
    if signal == "transfer" {
        return normalized_text(&row.dedup_type).contains("transfer")
            || row.preview_matching_feedback.get("transfer").is_some();
    }
    if signal == "learning" {
        return row.preview_matching_feedback.get("learning").is_some();
    }
    if normalized_text(&row.dedup_type).contains(&signal)
        || normalized_text(&row.preview_parser_id).contains(&signal)
        || row
            .preview_parser_tags
            .iter()
            .any(|tag| normalized_text(tag).contains(&signal))
    {
        return true;
    }
    let Some(feedback) = row.preview_matching_feedback.as_object() else {
        return false;
    };
    feedback.iter().any(|(key, value)| {
        normalized_text(key).contains(&signal)
            || value
                .as_str()
                .is_some_and(|text| normalized_text(text).contains(&signal))
            || value.as_object().is_some_and(|object| {
                object.values().any(|nested| {
                    nested
                        .as_str()
                        .is_some_and(|text| normalized_text(text).contains(&signal))
                })
            })
    })
}

fn preview_row_matches_annotation(row: &ImportPreviewRow, annotation: &str) -> bool {
    let annotation = normalized_text(annotation);
    if annotation.is_empty() {
        return true;
    }
    if annotation == "needs-review" {
        return preview_row_has_annotation_issues(row) || preview_row_has_feedback_annotation_issue(row);
    }
    if annotation == "no-issues" {
        return !preview_row_has_annotation_issues(row) && !preview_row_has_feedback_annotation_issue(row);
    }
    let Some(value) = row.preview_matching_feedback.get("annotation") else {
        return false;
    };
    value
        .as_str()
        .is_some_and(|text| normalized_text(text).contains(&annotation))
        || value.as_object().is_some_and(|object| {
            object.values().any(|nested| {
                nested
                    .as_str()
                    .is_some_and(|text| normalized_text(text).contains(&annotation))
            })
        })
}

fn sort_preview_rows(rows: &mut [ImportPreviewRow], sort_by: &str, sort_direction: &str) {
    rows.sort_by_key(|row| row.id);
    let descending = sort_direction.eq_ignore_ascii_case("desc");
    match sort_by {
        "time" | "" => rows.sort_by(|left, right| {
            compare_string(&left.preview_date, &right.preview_date, descending)
                .then_with(|| left.id.cmp(&right.id))
        }),
        "type" => rows.sort_by(|left, right| {
            compare_string(&left.preview_type, &right.preview_type, descending)
                .then_with(|| left.id.cmp(&right.id))
        }),
        "sourceAmount" => rows.sort_by(|left, right| {
            compare_f64(left.preview_amount, right.preview_amount, descending)
                .then_with(|| left.id.cmp(&right.id))
        }),
        "counterparty" => rows.sort_by(|left, right| {
            compare_string(&left.preview_counterparty, &right.preview_counterparty, descending)
                .then_with(|| left.id.cmp(&right.id))
        }),
        "paymentMethod" => rows.sort_by(|left, right| {
            compare_string(
                &left.preview_payment_method,
                &right.preview_payment_method,
                descending,
            )
            .then_with(|| left.id.cmp(&right.id))
        }),
        "comment" => rows.sort_by(|left, right| {
            compare_string(
                &left.preview_description,
                &right.preview_description,
                descending,
            )
            .then_with(|| left.id.cmp(&right.id))
        }),
        _ => {}
    }
}

fn compare_string(left: &str, right: &str, descending: bool) -> std::cmp::Ordering {
    let ordering = normalized_text(left).cmp(&normalized_text(right));
    if descending {
        ordering.reverse()
    } else {
        ordering
    }
}

fn compare_f64(left: f64, right: f64, descending: bool) -> std::cmp::Ordering {
    let ordering = left
        .partial_cmp(&right)
        .unwrap_or(std::cmp::Ordering::Equal);
    if descending {
        ordering.reverse()
    } else {
        ordering
    }
}

fn build_preview_metadata(rows: &[ImportPreviewRow]) -> ImportPreviewMetadata {
    let mut categories = std::collections::BTreeMap::<String, (Option<String>, usize)>::new();
    let mut accounts = std::collections::BTreeMap::<String, (Option<String>, usize)>::new();
    let mut tags = std::collections::BTreeMap::<String, (Option<String>, usize)>::new();
    let mut annotations = std::collections::BTreeMap::<String, usize>::new();
    let mut signals = std::collections::BTreeMap::<String, usize>::new();
    let mut selected = 0;
    let mut selected_invalid = 0;

    for row in rows {
        let has_annotation_issues =
            preview_row_has_annotation_issues(row) || preview_row_has_feedback_annotation_issue(row);
        if row.preview_selected {
            selected += 1;
            if has_annotation_issues {
                selected_invalid += 1;
            }
        }
        let category = first_non_empty_text([
            row.preview_sub_category.as_str(),
            row.preview_main_category.as_str(),
        ]);
        if let Some(category) = category {
            increment_facet(&mut categories, category, Some(category));
        }
        if let Some(account_id) = row.preview_source_account_id {
            let value = account_id.to_string();
            increment_facet(&mut accounts, value.as_str(), Some(value.as_str()));
        }
        if let Some(account_id) = row.preview_destination_account_id {
            let value = account_id.to_string();
            increment_facet(&mut accounts, value.as_str(), Some(value.as_str()));
        }
        for tag in &row.preview_parser_tags {
            if !tag.trim().is_empty() {
                increment_facet(&mut tags, tag, Some(tag));
            }
        }
        let annotation_key = if has_annotation_issues {
            "needs-review"
        } else {
            "no-issues"
        };
        *annotations.entry(annotation_key.to_string()).or_insert(0) += 1;
        for signal in signal_count_keys(row) {
            *signals.entry(signal).or_insert(0) += 1;
        }
    }

    ImportPreviewMetadata {
        facets: ImportPreviewFacets {
            categories: facet_entries(categories),
            accounts: facet_entries(accounts),
            tags: facet_entries(tags),
        },
        counts: ImportPreviewCounts {
            annotations,
            signals,
            selected,
            selected_invalid,
            total: rows.len(),
        },
    }
}

fn build_preview_metadata_by_query(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    filters: &ImportPreviewQueryFilters,
    total: usize,
) -> DbResult<ImportPreviewMetadata> {
    let mut metadata = ImportPreviewMetadata {
        facets: ImportPreviewFacets {
            categories: query_category_facets(connection, session_id, user_id, filters)?,
            accounts: query_account_facets(connection, session_id, user_id, filters)?,
            tags: query_tag_facets(connection, session_id, user_id, filters)?,
        },
        counts: ImportPreviewCounts {
            annotations: query_annotation_counts(connection, session_id, user_id, filters)?,
            signals: query_signal_counts(connection, session_id, user_id, filters)?,
            selected: 0,
            selected_invalid: 0,
            total,
        },
    };
    apply_session_selected_counts(connection, session_id, user_id, &mut metadata)?;
    Ok(metadata)
}

fn query_category_facets(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    filters: &ImportPreviewQueryFilters,
) -> DbResult<Vec<ImportPreviewFacetEntry>> {
    let mut query = build_preview_sql_query(
        "COALESCE(NULLIF(TRIM(preview_sub_category), ''), NULLIF(TRIM(preview_main_category), '')) AS facet_value, COUNT(*) AS facet_count",
        session_id,
        user_id,
        filters,
        None,
        None,
        None,
    )?;
    query
        .sql
        .push_str(" GROUP BY facet_value ORDER BY LOWER(facet_value) ASC");
    query_facet_entries(connection, &query)
}

fn query_account_facets(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    filters: &ImportPreviewQueryFilters,
) -> DbResult<Vec<ImportPreviewFacetEntry>> {
    let mut accounts = std::collections::BTreeMap::<String, (Option<String>, usize)>::new();
    for column in ["preview_source_account_id", "preview_destination_account_id"] {
        let mut query = build_preview_sql_query(
            &format!("{column} AS facet_value, COUNT(*) AS facet_count"),
            session_id,
            user_id,
            filters,
            None,
            None,
            None,
        )?;
        query
            .sql
            .push_str(&format!(" AND {column} IS NOT NULL GROUP BY {column}"));
        for entry in query_facet_entries(connection, &query)? {
            let next_count = accounts
                .entry(entry.value.clone())
                .or_insert_with(|| (entry.label.clone(), 0));
            next_count.1 += entry.count;
        }
    }
    Ok(facet_entries(accounts))
}

fn query_tag_facets(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    filters: &ImportPreviewQueryFilters,
) -> DbResult<Vec<ImportPreviewFacetEntry>> {
    let mut query = build_preview_sql_query_from(PreviewSqlBuildOptions {
        select: "TRIM(CAST(json_each.value AS TEXT)) AS facet_value, COUNT(*) AS facet_count",
        from: "bills_preview, json_each(CASE WHEN json_valid(COALESCE(preview_parser_tags_json, '')) THEN preview_parser_tags_json ELSE '[]' END)",
        session_id,
        user_id,
        filters,
        sort: None,
        limit: None,
        offset: None,
    })?;
    query.sql.push_str(
        " AND TRIM(CAST(json_each.value AS TEXT)) <> '' \
         GROUP BY facet_value ORDER BY LOWER(facet_value) ASC",
    );
    query_facet_entries(connection, &query)
}

fn query_annotation_counts(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    filters: &ImportPreviewQueryFilters,
) -> DbResult<std::collections::BTreeMap<String, usize>> {
    let issue_clause = preview_annotation_issue_sql_clause();
    let query = build_preview_sql_query(
        &format!("SUM(CASE WHEN {issue_clause} THEN 1 ELSE 0 END), COUNT(*)"),
        session_id,
        user_id,
        filters,
        None,
        None,
        None,
    )?;
    let (needs_review, total): (i64, i64) = connection.query_row(
        query.sql.as_str(),
        params_from_iter(query.params.iter()),
        |row| Ok((row.get::<_, Option<i64>>(0)?.unwrap_or(0), row.get(1)?)),
    )?;
    let needs_review = usize::try_from(needs_review.max(0)).unwrap_or(usize::MAX);
    let total = usize::try_from(total.max(0)).unwrap_or(usize::MAX);
    let mut counts = std::collections::BTreeMap::new();
    counts.insert("needs-review".to_string(), needs_review);
    counts.insert("no-issues".to_string(), total.saturating_sub(needs_review));
    Ok(counts)
}

fn query_signal_counts(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    filters: &ImportPreviewQueryFilters,
) -> DbResult<std::collections::BTreeMap<String, usize>> {
    let mut signals = std::collections::BTreeMap::<String, usize>::new();

    let mut dedup_query = build_preview_sql_query(
        "TRIM(dedup_type) AS signal_key, COUNT(*) AS signal_count",
        session_id,
        user_id,
        filters,
        None,
        None,
        None,
    )?;
    dedup_query.sql.push_str(
        " AND TRIM(COALESCE(dedup_type, '')) <> '' \
         GROUP BY signal_key",
    );
    merge_signal_counts(connection, &dedup_query, &mut signals)?;

    let mut feedback_query = build_preview_sql_query_from(PreviewSqlBuildOptions {
        select: "json_each.key AS signal_key, COUNT(*) AS signal_count",
        from: "bills_preview, json_each(CASE WHEN json_valid(COALESCE(preview_matching_feedback_json, '')) THEN preview_matching_feedback_json ELSE '{}' END)",
        session_id,
        user_id,
        filters,
        sort: None,
        limit: None,
        offset: None,
    })?;
    feedback_query.sql.push_str(
        " AND TRIM(CAST(json_each.key AS TEXT)) <> '' \
         AND json_each.value IS NOT NULL \
         GROUP BY signal_key",
    );
    merge_signal_counts(connection, &feedback_query, &mut signals)?;

    Ok(signals)
}

fn query_facet_entries(
    connection: &Connection,
    query: &PreviewSqlQuery,
) -> DbResult<Vec<ImportPreviewFacetEntry>> {
    let mut statement = connection.prepare(query.sql.as_str())?;
    let rows = statement.query_map(params_from_iter(query.params.iter()), |row| {
        let value = sql_value_to_text(row.get::<_, SqlValue>(0)?);
        let count = row.get::<_, i64>(1)?;
        Ok((value, count))
    })?;
    let mut entries = Vec::new();
    for row in rows {
        let (value, count) = row?;
        let value = value.trim().to_string();
        if value.is_empty() || count <= 0 {
            continue;
        }
        entries.push(ImportPreviewFacetEntry {
            value: value.clone(),
            label: Some(value),
            count: usize::try_from(count).unwrap_or(usize::MAX),
        });
    }
    Ok(entries)
}

fn merge_signal_counts(
    connection: &Connection,
    query: &PreviewSqlQuery,
    signals: &mut std::collections::BTreeMap<String, usize>,
) -> DbResult<()> {
    let mut statement = connection.prepare(query.sql.as_str())?;
    let rows = statement.query_map(params_from_iter(query.params.iter()), |row| {
        let value = sql_value_to_text(row.get::<_, SqlValue>(0)?);
        let count = row.get::<_, i64>(1)?;
        Ok((value, count))
    })?;
    for row in rows {
        let (key, count) = row?;
        let key = key.trim().to_string();
        if key.is_empty() || count <= 0 {
            continue;
        }
        *signals.entry(key).or_insert(0) += usize::try_from(count).unwrap_or(usize::MAX);
    }
    Ok(())
}

fn sql_value_to_text(value: SqlValue) -> String {
    match value {
        SqlValue::Null => String::new(),
        SqlValue::Integer(value) => value.to_string(),
        SqlValue::Real(value) => value.to_string(),
        SqlValue::Text(value) => value,
        SqlValue::Blob(_) => String::new(),
    }
}

fn apply_session_selected_counts(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    metadata: &mut ImportPreviewMetadata,
) -> DbResult<()> {
    let selected_counts = count_session_selected_state(connection, session_id, user_id)?;
    metadata.counts.selected = selected_counts.0;
    metadata.counts.selected_invalid = selected_counts.1;
    Ok(())
}

fn count_session_selected_state(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<(usize, usize)> {
    let filters = ImportPreviewQueryFilters {
        selected_only: true,
        ..Default::default()
    };
    let issue_clause = preview_annotation_issue_sql_clause();
    let query = build_preview_sql_query(
        &format!("COUNT(*), SUM(CASE WHEN {issue_clause} THEN 1 ELSE 0 END)"),
        session_id,
        user_id,
        &filters,
        None,
        None,
        None,
    )?;
    let (selected, selected_invalid): (i64, i64) = connection.query_row(
        query.sql.as_str(),
        params_from_iter(query.params.iter()),
        |row| Ok((row.get(0)?, row.get::<_, Option<i64>>(1)?.unwrap_or(0))),
    )?;
    Ok((
        usize::try_from(selected.max(0)).unwrap_or(usize::MAX),
        usize::try_from(selected_invalid.max(0)).unwrap_or(usize::MAX),
    ))
}

fn first_non_empty_text<'a>(values: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    values
        .into_iter()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

fn increment_facet(
    facets: &mut std::collections::BTreeMap<String, (Option<String>, usize)>,
    value: &str,
    label: Option<&str>,
) {
    let entry = facets
        .entry(value.to_string())
        .or_insert_with(|| (label.map(ToOwned::to_owned), 0));
    entry.1 += 1;
}

fn facet_entries(
    facets: std::collections::BTreeMap<String, (Option<String>, usize)>,
) -> Vec<ImportPreviewFacetEntry> {
    facets
        .into_iter()
        .map(|(value, (label, count))| ImportPreviewFacetEntry {
            value,
            label,
            count,
        })
        .collect()
}

fn annotation_count_key(feedback: &Value) -> Option<String> {
    let value = feedback.get("annotation")?;
    if let Some(text) = value.as_str().map(str::trim).filter(|text| !text.is_empty()) {
        return Some(text.to_string());
    }
    value.as_object().and_then(|object| {
        ["status", "review_status", "level", "type"]
            .iter()
            .find_map(|key| object.get(*key)?.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn annotation_type_key(feedback: &Value) -> Option<String> {
    let value = feedback.get("annotation")?;
    if let Some(text) = value.as_str().map(str::trim).filter(|text| !text.is_empty()) {
        return Some(text.to_string());
    }
    value
        .as_object()
        .and_then(|object| object.get("type")?.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn annotation_type_in(annotation_type: &str, candidates: &[&str]) -> bool {
    let normalized = normalized_text(annotation_type);
    candidates.iter().any(|candidate| normalized == *candidate)
}

fn preview_row_has_feedback_annotation_issue(row: &ImportPreviewRow) -> bool {
    if annotation_count_key(&row.preview_matching_feedback).is_none() {
        return false;
    }

    let Some(annotation_type) = annotation_type_key(&row.preview_matching_feedback) else {
        return true;
    };

    if annotation_type_in(
        &annotation_type,
        &[
            "category",
            "category_missing",
            "missing_category",
            "missing-category",
            "missing_classification",
        ],
    ) {
        return preview_row_has_category_issues(row);
    }

    if annotation_type_in(
        &annotation_type,
        &[
            "account",
            "missing_account",
            "source_account",
            "source_account_missing",
            "missing_source_account",
            "missing-source-account",
        ],
    ) {
        return preview_row_has_source_account_issues(row);
    }

    if annotation_type_in(
        &annotation_type,
        &[
            "destination_account",
            "destination_account_missing",
            "missing_destination_account",
            "missing-destination-account",
        ],
    ) {
        return preview_row_has_destination_account_issues(row);
    }

    if annotation_type_in(
        &annotation_type,
        &[
            "transfer_account_direction",
            "transfer_accounts",
            "review_transfer_accounts",
            "same_transfer_accounts",
        ],
    ) {
        return preview_row_has_account_issues(row);
    }

    true
}

fn signal_count_keys(row: &ImportPreviewRow) -> Vec<String> {
    let mut keys = Vec::new();
    if !row.dedup_type.trim().is_empty() {
        keys.push(row.dedup_type.clone());
    }
    if let Some(object) = row.preview_matching_feedback.as_object() {
        keys.extend(
            object
                .iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(key, _)| key.clone()),
        );
    }
    keys.sort();
    keys.dedup();
    keys
}

pub fn get_preview_filter_index_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewFilterIndexRow>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            preview_date,
            preview_type,
            preview_amount,
            preview_main_category,
            preview_sub_category,
            preview_source_account_id,
            preview_destination_account_id,
            preview_counterparty,
            preview_payment_method,
            preview_description,
            preview_parser_id,
            preview_parser_tags_json,
            preview_recurring_id,
            preview_recurring_candidate_count,
            preview_recurring_match_reasons,
            preview_recurring_matched_date,
            preview_selected,
            dedup_type,
            dedup_source_ids,
            preview_matching_feedback_json
        FROM bills_preview
        WHERE session_id = ?1 AND user_id = ?2
        ORDER BY preview_date ASC, id ASC
        ",
    )?;
    let rows = statement.query_map(
        params![session_id, user_id_i64(user_id)?],
        preview_filter_index_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn get_preview_bill_by_id(
    connection: &Connection,
    preview_id: i64,
    user_id: UserId,
) -> DbResult<Option<ImportPreviewRow>> {
    connection
        .query_row(
            "SELECT * FROM bills_preview WHERE id = ?1 AND user_id = ?2",
            params![preview_id, user_id_i64(user_id)?],
            preview_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_preview_by_ids(
    connection: &Connection,
    session_id: &str,
    preview_ids: &[i64],
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewRow>> {
    let normalized_ids: Vec<i64> = preview_ids
        .iter()
        .copied()
        .filter(|preview_id| *preview_id > 0)
        .collect();
    if normalized_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = std::iter::repeat_n("?", normalized_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut statement = connection.prepare(&format!(
        "SELECT * FROM bills_preview WHERE session_id = ? AND user_id = ? AND id IN ({placeholders})"
    ))?;
    let user_id = user_id_i64(user_id)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(normalized_ids.len() + 2);
    params.push(&session_id);
    params.push(&user_id);
    for preview_id in &normalized_ids {
        params.push(preview_id);
    }
    let rows = statement.query_map(params.as_slice(), preview_from_row)?;
    let lookup = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|preview| (preview.id, preview))
        .collect::<std::collections::HashMap<_, _>>();
    Ok(normalized_ids
        .iter()
        .filter_map(|preview_id| lookup.get(preview_id).cloned())
        .collect())
}
