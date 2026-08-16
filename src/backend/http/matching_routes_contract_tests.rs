use super::*;
use axum::{
    body::to_bytes,
    extract::{Query, State},
    http::HeaderValue,
    Json,
};
use bill_analyser_db::ImportPreviewRow;

async fn response_value(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body bytes");
    (
        status,
        serde_json::from_slice(&body).expect("JSON response body"),
    )
}

fn matching_test_state() -> HttpAppState {
    HttpAppState::new(HttpShellConfig::default().with_trusted_user_header_secret("matching-secret"))
        .expect("state builds")
}

fn trusted_headers() -> HeaderMap {
    trusted_headers_for_user(42)
}

fn trusted_headers_for_user(user_id: i64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        TRUSTED_USER_SECRET_HEADER,
        HeaderValue::from_static("matching-secret"),
    );
    headers.insert(
        "x-user-id",
        HeaderValue::from_str(&user_id.to_string()).expect("valid user id header"),
    );
    headers
}

#[tokio::test]
async fn matching_response_helpers_pin_data_error_and_message_envelopes() {
    let (status, data) = response_value(success_data(StatusCode::OK, json!({"pairs": []}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data, json!({"success": true, "data": {"pairs": []}}));

    let (status, error) = response_value(error_response(StatusCode::BAD_REQUEST, "bad")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error, json!({"success": false, "error": "bad"}));

    let (status, message) = response_value(message_response(StatusCode::GONE, "gone")).await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(message, json!({"success": false, "message": "gone"}));
}

#[tokio::test]
async fn learning_row_version_conflict_returns_the_authoritative_preview_snapshot() {
    let latest = ImportPreviewRow {
        id: 44,
        version: 8,
        session_id: "session-http".to_string(),
        user_id: 42,
        preview_date: "2026-08-16T00:00:00Z".to_string(),
        preview_type: "支出".to_string(),
        preview_amount_cents: 1234,
        preview_destination_amount_cents: 0,
        category_id: None,
        preview_main_category: String::new(),
        preview_sub_category: String::new(),
        preview_source_account_id: None,
        preview_destination_account_id: None,
        preview_counterparty: "测试商户".to_string(),
        preview_payment_method: String::new(),
        preview_description: String::new(),
        preview_parser_id: "fixture".to_string(),
        preview_parser_tags: vec!["parser:fixture".to_string()],
        preview_recurring_id: None,
        preview_recurring_name: String::new(),
        preview_recurring_candidate_count: 0,
        preview_recurring_match_score: 0.0,
        preview_recurring_match_reasons: String::new(),
        preview_recurring_matched_date: String::new(),
        preview_selected: true,
        dedup_type: String::new(),
        dedup_source_ids: Vec::new(),
        preview_matching_feedback: json!({
            "learning": {"review_status": "pending"}
        }),
        created_at: "2026-08-16T00:00:00Z".to_string(),
    };

    let (status, body) =
        response_value(preview_row_version_conflict_response(7, latest)).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
    assert_eq!(body["data"]["expected_row_version"], 7);
    assert_eq!(body["data"]["actual_row_version"], 8);
    assert_eq!(body["data"]["previewItem"]["id"], 44);
    assert_eq!(body["data"]["previewItem"]["row_version"], 8);
    assert_eq!(
        body["data"]["previewItem"]["preview_state"]["decisions"]["learning"]["status"],
        "pending"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn learning_action_handler_rebases_stale_row_version_from_postgres(
) -> Result<(), Box<dyn std::error::Error>> {
    let postgres_url = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for the matching row CAS contract",
        )
    })?;
    let config = HttpShellConfig::default()
        .with_postgres_url(postgres_url)?
        .with_trusted_user_header_secret("matching-secret");
    let state = HttpAppState::new(config)?;
    let runtime = state.open_postgres_repository_runtime("matching-row-cas-test")?;
    bill_analyser_db::run_postgres_migrations(runtime.pool()).await?;

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let username = format!("matching-row-cas-{unique}");
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id",
    )
    .bind(&username)
    .bind(format!("{username}@example.test"))
    .fetch_one(runtime.pool())
    .await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let session_id = format!("matching-row-cas-session-{unique}");
    bill_analyser_db::create_import_session(
        runtime.pool(),
        &bill_analyser_db::ImportSessionDraft {
            session_id: session_id.clone(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    let preview_id = bill_analyser_db::insert_preview_bill(
        runtime.pool(),
        &session_id,
        scoped_user_id,
        &bill_analyser_db::ImportPreviewDraft {
            preview_date: "2026-08-16 12:00:00".to_string(),
            preview_type: "支出".to_string(),
            preview_amount_cents: -1234,
            preview_counterparty: "CAS 测试商户".to_string(),
            preview_parser_id: "matching-row-cas-test".to_string(),
            preview_matching_feedback: json!({
                "learning": {
                    "review_status": "pending",
                    "recommendation_key": "matching-row-cas"
                }
            }),
            preview_selected: true,
            ..bill_analyser_db::ImportPreviewDraft::default()
        },
    )?;

    let response = matching_candidate_action_response(
        &state,
        &trusted_headers_for_user(user_id),
        format!("preview:{preview_id}:learning"),
        "accept",
        Some(Json(json!({
            "expectedState": {
                "sessionId": session_id,
                "rowVersion": 2
            },
            "responseMode": "preview-item"
        }))),
    );
    let (status, body) = response_value(response).await;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(runtime.pool())
        .await?;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
    assert_eq!(body["data"]["expected_row_version"], 2);
    assert_eq!(body["data"]["actual_row_version"], 1);
    assert_eq!(body["data"]["previewItem"]["id"], preview_id);
    assert_eq!(body["data"]["previewItem"]["row_version"], 1);
    assert_eq!(
        body["data"]["previewItem"]["preview_state"]["decisions"]["learning"]["status"],
        "pending"
    );
    Ok(())
}

#[tokio::test]
async fn matching_candidates_query_validation_rejects_missing_both_both_and_bad_bill() {
    let state = matching_test_state();
    for (query, expected_error) in [
        (
            MatchingCandidatesQuery::default(),
            "Exactly one of sessionId or billId is required",
        ),
        (
            MatchingCandidatesQuery {
                session_id: Some("session-1".to_string()),
                bill_id: Some("101".to_string()),
            },
            "Exactly one of sessionId or billId is required",
        ),
        (
            MatchingCandidatesQuery {
                session_id: None,
                bill_id: Some("not-a-bill".to_string()),
            },
            "Invalid billId",
        ),
    ] {
        let (status, body) = response_value(
            matching_candidates_handler(State(state.clone()), HeaderMap::new(), Query(query)).await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, json!({"success": false, "error": expected_error}));
    }
}

#[tokio::test]
async fn non_preview_matching_candidate_actions_keep_explicit_conflict_envelope() {
    let state = matching_test_state();
    let headers = trusted_headers();
    let response = matching_candidate_action_response(
        &state,
        &headers,
        "bill:42:transfer:43".to_string(),
        "accept",
        Some(Json(
            json!({"expectedState": {"sessionId": "session-http"}}),
        )),
    );
    let (status, body) = response_value(response).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body,
        json!({
            "success": false,
            "error": "Matching candidate action is not available for this candidate"
        })
    );
}

#[test]
fn stale_preview_actions_keep_conflict_contract_without_resurrecting_removed_signals() {
    let matching_source = include_str!("matching_routes/payloads.rs").replace("\r\n", "\n");
    let direct_llm_source =
        include_str!("import_routes/llm/review_memory_config.rs").replace("\r\n", "\n");
    let learning_db_source =
        include_str!("../db/import_staging/preview_learning_lifecycle/decisions.rs")
            .replace("\r\n", "\n");
    let llm_db_source = include_str!("../db/import_staging/preview_llm/review.rs")
        .replace("\r\n", "\n");

    let matching_conflict_guard = matching_source
        .find("Ok(result) if result.state_conflict")
        .expect("matching action state-conflict guard");
    let matching_conflict_response = matching_source
        .find("error_response(StatusCode::CONFLICT, \"Preview state is stale\")")
        .expect("matching action stale-state 409 response");
    let matching_feedback_projection = matching_source
        .find("let review_status = preview_action_review_status(")
        .expect("matching action feedback projection");
    assert!(
        matching_conflict_guard < matching_conflict_response
            && matching_conflict_response < matching_feedback_projection,
        "state conflicts must return 409 before projecting a replacement signal"
    );

    let direct_llm_conflict_guard = direct_llm_source
        .find("Ok(result) if result.state_conflict")
        .expect("direct LLM state-conflict guard");
    let direct_llm_conflict_response = direct_llm_source
        .find("route_response(import_v2_error_response(409, \"Preview state is stale\"))")
        .expect("direct LLM stale-state 409 response");
    let direct_llm_success_response = direct_llm_source
        .find("Ok(result) => route_response(llm_decision_result_response(")
        .expect("direct LLM success response");
    assert!(
        direct_llm_conflict_guard < direct_llm_conflict_response
            && direct_llm_conflict_response < direct_llm_success_response,
        "state conflicts must return 409 before the direct LLM success response"
    );
    let expected_state_start = direct_llm_source
        .find("let expected_state = if first_value(object, &[\"expectedState\", \"expected_state\"])")
        .expect("direct LLM optional expected-state guard");
    let expected_state_end = direct_llm_source[expected_state_start..]
        .find("let user_correction =")
        .map(|offset| expected_state_start + offset)
        .expect("direct LLM expected-state block end");
    let compact_expected_state = direct_llm_source[expected_state_start..expected_state_end]
        .split_whitespace()
        .collect::<String>();
    assert!(compact_expected_state.contains("Some(expected_state)"));
    assert!(compact_expected_state.contains("}else{None};"));

    let learning_guard = learning_db_source
        .find("match preview_terminal_action(")
        .expect("learning terminal guard");
    let learning_feedback = learning_db_source
        .find("let lifecycle_input = preview_learning_lifecycle_input")
        .expect("learning feedback path");
    let llm_guard = llm_db_source
        .find("match preview_terminal_action(")
        .expect("LLM terminal guard");
    let llm_feedback = llm_db_source
        .find("preview.preview_matching_feedback = set_feedback_review_status(")
        .expect("LLM feedback path");
    let llm_event = llm_db_source
        .find("insert_llm_review_event_in_transaction(")
        .expect("LLM event path");

    assert!(learning_guard < learning_feedback);
    assert!(llm_guard < llm_feedback && llm_feedback < llm_event);
}
