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

