#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ImportV2RouteResponse {
    status_code: u16,
    body: Value,
}

/// 中文说明：生成导入 v2 标准错误响应，统一 success=false 与 error 字段形态。
#[tracing::instrument(level = "debug", skip_all)]
fn import_v2_error_response(status_code: u16, error: &str) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({"success": false, "error": error}),
    }
}

fn import_version_required_response(token_kind: &str) -> ImportV2RouteResponse {
    tracing::warn!(
        domain = "import_contract",
        token_kind,
        contract = "version_required_rejected",
        required_tokens = 1,
        present_tokens = 0,
        missing_tokens = 1,
        "import mutation rejected a missing concurrency token"
    );
    let (code, error, required_token) = match token_kind {
        "session_version" => (
            "IMPORT_SESSION_VERSION_REQUIRED",
            "Import session version is required",
            "expected_session_version",
        ),
        _ => (
            "PREVIEW_ROW_VERSION_REQUIRED",
            "Preview row version is required",
            "expected_row_version",
        ),
    };
    ImportV2RouteResponse {
        status_code: 428,
        body: json!({
            "success": false,
            "error": error,
            "code": code,
            "data": {"required_token": required_token},
        }),
    }
}

/// 中文说明：生成导入 v2 消息响应，供取消会话等无需 data payload 的路由复用。
#[tracing::instrument(level = "debug", skip_all)]
fn import_v2_message_response(
    status_code: u16,
    success: bool,
    message: &str,
) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({"success": success, "message": message}),
    }
}

/// 中文说明：生成导入 v2 成功 data 响应，统一后端导入链路的 REST envelope。
#[tracing::instrument(level = "debug", skip_all)]
fn import_v2_data_response<T>(data: T) -> ImportV2RouteResponse
where
    T: serde::Serialize,
{
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": data}),
    }
}

/// 中文说明：包装 parse stage 成功响应，保持导入阶段响应结构与前端服务适配器一致。
#[tracing::instrument(level = "debug", skip_all)]
fn import_stage_parse_success(data: ImportStageParseData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_stage_parse_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

/// 中文说明：包装 dedup stage 成功响应，保留去重候选、转账和重复组数据的标准 envelope。
#[tracing::instrument(level = "debug", skip_all)]
fn import_stage_dedup_success(data: ImportStageDedupData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_stage_dedup_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

/// 中文说明：包装 confirm stage 成功响应，统一确认导入后的数量和结果 payload。
#[tracing::instrument(level = "debug", skip_all)]
fn import_stage_confirm_success(data: ImportStageConfirmData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_stage_confirm_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

/// 中文说明：包装分页预览响应，供前端预览表格读取 page/item/filter metadata。
#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_page_success(data: ImportPreviewPageData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_preview_page_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

/// 中文说明：包装预览索引响应，供前端跨页筛选和选择目标使用。
#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_index_success(data: ImportPreviewIndexData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_preview_index_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

/// 中文说明：包装导入会话摘要响应，保留 session 状态、阶段和用户归属边界。
#[tracing::instrument(level = "debug", skip_all)]
fn import_session_success(session: ImportSessionSummary) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_session_success",
        "business operation entered"
    );
    import_v2_data_response(session)
}

/// 中文说明：生成导入会话不存在响应，统一过期或无权访问 session 的客户端错误。
#[tracing::instrument(level = "debug", skip_all)]
fn import_session_not_found_response() -> ImportV2RouteResponse {
    import_v2_error_response(404, "Session not found or expired")
}

/// 中文说明：生成取消导入时 session 缺失响应，保持旧客户端可接受的 success=false 消息语义。
#[tracing::instrument(level = "debug", skip_all)]
fn import_session_cancel_missing_response() -> ImportV2RouteResponse {
    import_v2_message_response(200, false, "Session not found")
}

/// 中文说明：生成取消导入成功响应，统一会话清理完成的消息 envelope。
#[tracing::instrument(level = "debug", skip_all)]
fn import_session_cancel_success_response() -> ImportV2RouteResponse {
    import_v2_message_response(200, true, "Session cleared")
}

/// 中文说明：生成预览状态冲突响应，提示前端刷新后再确认导入。
#[tracing::instrument(level = "debug", skip_all)]
fn preview_state_conflict_response() -> ImportV2RouteResponse {
    import_v2_error_response(409, "Preview state changed, please refresh")
}
