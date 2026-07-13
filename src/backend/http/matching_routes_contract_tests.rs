use super::*;
use axum::{
    body::to_bytes,
    extract::{Query, State},
    http::HeaderValue,
    Json,
};

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
    let mut headers = HeaderMap::new();
    headers.insert(
        TRUSTED_USER_SECRET_HEADER,
        HeaderValue::from_static("matching-secret"),
    );
    headers.insert("x-user-id", HeaderValue::from_static("42"));
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
    let matching_source = include_str!("matching_routes/payloads.rs");
    let direct_llm_source = include_str!("import_routes/llm/review_memory_config.rs");
    let learning_db_source =
        include_str!("../db/import_staging/preview_learning_lifecycle.rs");
    let llm_db_source = include_str!("../db/import_staging/preview_llm.rs");

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
