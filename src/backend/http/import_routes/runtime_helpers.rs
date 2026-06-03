// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

struct ImportRuntime {
    inner: PostgresRepositoryRuntime,
}

impl ImportRuntime {
    fn pool(&self) -> &PostgresPool {
        self.inner.pool()
    }

    fn connection(&self) -> &PostgresPool {
        self.pool()
    }

    fn connection_mut(&mut self) -> &PostgresPool {
        self.pool()
    }
}

fn open_runtime(state: &HttpAppState) -> Result<ImportRuntime, ImportV2RouteResponse> {
    open_postgres_runtime(state).map(|inner| ImportRuntime { inner })
}

fn open_postgres_runtime(
    state: &HttpAppState,
) -> Result<PostgresRepositoryRuntime, ImportV2RouteResponse> {
    state
        .open_postgres_repository_runtime("import")
        .map_err(|error| {
            import_v2_error_response(
                error.http_status_code(),
                &error.public_message(),
            )
        })
}

fn init_import_runtime_schema(runtime: &ImportRuntime) -> Result<(), ImportV2RouteResponse> {
    init_import_staging_schema(runtime.connection()).map_err(db_error_response)
}

fn user_id_from_headers(
    headers: &HeaderMap,
    config: &HttpShellConfig,
) -> Result<UserId, ImportV2RouteResponse> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER)
        .map_err(|error| import_v2_error_response(error.status, &error.message))
}

fn db_error_response(_error: impl std::fmt::Display) -> ImportV2RouteResponse {
    import_v2_error_response(500, "Rust import route runtime DB error")
}

fn non_negative_usize(value: i64) -> usize {
    usize::try_from(value.max(0)).unwrap_or(usize::MAX)
}

#[derive(Debug, Default, Deserialize)]
pub struct ImportLearningRulesQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    #[serde(rename = "pageSize")]
    page_size_camel: Option<usize>,
    limit: Option<usize>,
    enabled_only: Option<bool>,
    #[serde(rename = "enabledOnly")]
    enabled_only_camel: Option<bool>,
}

impl ImportLearningRulesQuery {
    fn page_size(&self) -> usize {
        self.page_size
            .or(self.page_size_camel)
            .or(self.limit)
            .unwrap_or(100)
    }

    fn enabled_only(&self) -> Option<bool> {
        self.enabled_only.or(self.enabled_only_camel)
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct LearningCenterListQuery {
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    enabled_only: Option<bool>,
    #[serde(rename = "enabledOnly")]
    enabled_only_camel: Option<bool>,
}

impl LearningCenterListQuery {
    fn enabled_only(&self) -> Option<bool> {
        self.enabled_only.or(self.enabled_only_camel)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PreviewPageQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    #[serde(rename = "pageSize")]
    page_size_camel: Option<usize>,
    sort_by: Option<String>,
    #[serde(rename = "sortBy")]
    sort_by_camel: Option<String>,
    sort_direction: Option<String>,
    #[serde(rename = "sortDirection")]
    sort_direction_camel: Option<String>,
    preview_ids: Option<String>,
    #[serde(rename = "previewIds")]
    preview_ids_camel: Option<String>,
    selected_only: Option<bool>,
    #[serde(rename = "selectedOnly")]
    selected_only_camel: Option<bool>,
    min_datetime: Option<String>,
    #[serde(rename = "minDatetime")]
    min_datetime_camel: Option<String>,
    max_datetime: Option<String>,
    #[serde(rename = "maxDatetime")]
    max_datetime_camel: Option<String>,
    transaction_type: Option<String>,
    #[serde(rename = "transactionType")]
    transaction_type_camel: Option<String>,
    category: Option<String>,
    account: Option<String>,
    tag: Option<String>,
    signal: Option<String>,
    annotation: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LlmMemoryQuery {
    session_id: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LlmCandidatesQuery {
    status: Option<String>,
    r#type: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}
