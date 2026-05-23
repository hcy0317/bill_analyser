// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn record_learning_feedback_event(
    connection: &Connection,
    user_id: i64,
    event_type: &str,
    rule_id: Option<i64>,
    suggestion_id: Option<i64>,
    payload: Option<Value>,
) -> Result<(), ImportV2RouteResponse> {
    let payload_json = payload.map(|value| value.to_string());
    connection
        .execute(
            "
            INSERT INTO import_learning_feedback_events (
                user_id, event_type, rule_id, suggestion_id, payload_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                user_id,
                event_type,
                rule_id,
                suggestion_id,
                payload_json,
                now_text(),
            ],
        )
        .map(|_| ())
        .map_err(db_error_response)
}

