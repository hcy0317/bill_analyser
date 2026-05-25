use std::error::Error;

use bill_analyser_core::UserId;
use bill_analyser_db::{
    apply_matching_candidate_action, apply_preview_llm_recommendation, create_import_session,
    create_manual_matching_pair, delete_manual_matching_pair, get_preview_bill_by_id,
    init_import_staging_schema, init_matching_runtime_schema, insert_preview_bills_batch,
    list_reconciliation_candidates_payload, query_matching_bill_candidates_payload,
    query_matching_bill_feedback_payload, query_matching_pairs_payload,
    query_matching_session_candidates_payload, ImportPreviewDraft, ImportPreviewExpectedState,
    ImportPreviewLearningApply, ImportPreviewLlmApplyRequest, ImportPreviewLlmSuggestion,
    ImportPreviewRecurringCandidate, ImportSessionDraft, PreviewMatchingActionRequest,
    ReconciliationCandidateFilters,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

const OWNER_ID: u64 = 42;

#[test]
fn matching_runtime_repository_covers_bill_preview_and_reconciliation_flows(
) -> Result<(), Box<dyn Error>> {
    let mut connection = Connection::open_in_memory()?;
    seed_matching_fixture(&mut connection)?;
    let user_id = user_id();

    let session_payload =
        query_matching_session_candidates_payload(&connection, user_id, "session-matching")?
            .expect("session candidates");
    assert_eq!(session_payload["session_id"], "session-matching");
    assert!(session_payload["candidates"]
        .as_array()
        .expect("session candidate rows")
        .iter()
        .any(|candidate| candidate["candidate_id"] == "preview:1:transfer"));
    assert!(query_matching_session_candidates_payload(&connection, user_id, "missing")?.is_none());

    let transfer_payload = query_matching_bill_candidates_payload(&connection, user_id, 101)?
        .expect("transfer candidates");
    assert!(candidate_ids(&transfer_payload).contains(&"bill:101:transfer:102".to_string()));
    let strict_transfer_payload =
        query_matching_bill_candidates_payload(&connection, user_id, 601)?
            .expect("strict transfer candidates");
    let strict_transfer_ids = candidate_ids(&strict_transfer_payload);
    assert!(!strict_transfer_ids.contains(&"bill:601:transfer:602".to_string()));
    assert!(!strict_transfer_ids.contains(&"bill:601:transfer:603".to_string()));
    assert!(!strict_transfer_ids.contains(&"bill:601:transfer:604".to_string()));
    let duplicate_payload = query_matching_bill_candidates_payload(&connection, user_id, 701)?
        .expect("duplicate candidates");
    assert!(candidate_ids(&duplicate_payload).contains(&"bill:701:duplicate:702".to_string()));
    assert!(!candidate_ids(&duplicate_payload).contains(&"bill:701:duplicate:703".to_string()));
    assert!(duplicate_payload["candidates"]
        .as_array()
        .expect("candidate rows")
        .iter()
        .any(|candidate| {
            candidate["candidateId"] == "bill:701:duplicate:702"
                && candidate["kind"] == "duplicate"
                && candidate["bill"]["description"] == "Identical coffee"
        }));
    let rejected_duplicate = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:701:duplicate:702",
        "reject",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(rejected_duplicate["action"], "reject");
    let suppressed_duplicate = query_matching_bill_candidates_payload(&connection, user_id, 701)?
        .expect("suppressed duplicate");
    assert!(!candidate_ids(&suppressed_duplicate).contains(&"bill:701:duplicate:702".to_string()));
    let accepted_duplicate = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:704:duplicate:705",
        "accept",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(accepted_duplicate["pair"]["pairType"], "duplicate");
    assert_eq!(accepted_duplicate["mergedBillId"], 705);
    assert_eq!(bill_count(&connection, 704)?, 1);
    assert_eq!(bill_count(&connection, 705)?, 0);
    let duplicate_after_accept = query_matching_bill_candidates_payload(&connection, user_id, 704)?
        .expect("duplicate merge payload");
    assert!(duplicate_after_accept["linkedPair"].is_null());
    assert!(!candidate_ids(&duplicate_after_accept).contains(&"bill:704:duplicate:705".to_string()));
    let investment_payload = query_matching_bill_candidates_payload(&connection, user_id, 201)?
        .expect("investment candidates");
    assert!(candidate_ids(&investment_payload).contains(&"bill:201:investment:202".to_string()));
    let learning_payload =
        query_matching_bill_candidates_payload(&connection, user_id, 301)?.expect("learning");
    let learning_candidate_id = candidate_ids(&learning_payload)
        .into_iter()
        .find(|candidate_id| candidate_id.starts_with("bill:301:learning:8:"))
        .expect("learning candidate id");
    assert!(query_matching_bill_candidates_payload(&connection, user_id, 999)?.is_none());

    let manual_pair = create_manual_matching_pair(
        &mut connection,
        user_id,
        101,
        102,
        "transfer",
        Some("bill:101:transfer:102"),
    )?;
    let manual_pair_id = manual_pair["pair"]["id"].as_i64().expect("manual pair id");
    let pairs = query_matching_pairs_payload(&connection, user_id)?;
    assert_eq!(pairs["pairs"][0]["leftBillId"], 101);
    let feedback =
        query_matching_bill_feedback_payload(&connection, user_id, 101)?.expect("feedback payload");
    assert_eq!(feedback["events"][0]["action"], "accept");
    let deleted_pair = delete_manual_matching_pair(&mut connection, user_id, manual_pair_id)?;
    assert_eq!(deleted_pair["pair"]["id"], manual_pair_id);

    let accepted_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:101:transfer:102",
        "accept",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(accepted_transfer["pair"]["pairType"], "transfer");
    assert_eq!(accepted_transfer["mergedBillId"], 102);
    assert_eq!(accepted_transfer["bill"]["type"], "转账");
    assert_eq!(accepted_transfer["bill"]["amount"], -50.0);
    assert_eq!(accepted_transfer["bill"]["source_account_id"], 10);
    assert_eq!(accepted_transfer["bill"]["destination_account_id"], 11);
    assert_eq!(accepted_transfer["bill"]["destination_amount"], 50.0);
    assert_eq!(bill_count(&connection, 101)?, 1);
    assert_eq!(bill_count(&connection, 102)?, 0);
    let transfer_after_accept = query_matching_bill_candidates_payload(&connection, user_id, 101)?
        .expect("merged transfer payload");
    assert!(transfer_after_accept["linkedPair"].is_null());
    assert!(!candidate_ids(&transfer_after_accept).contains(&"bill:101:transfer:102".to_string()));

    let accepted_incoming_anchor_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:707:transfer:706",
        "accept",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(
        accepted_incoming_anchor_transfer["pair"]["pairType"],
        "transfer"
    );
    assert_eq!(accepted_incoming_anchor_transfer["bill"]["id"], 707);
    assert_eq!(accepted_incoming_anchor_transfer["bill"]["type"], "转账");
    assert_eq!(accepted_incoming_anchor_transfer["bill"]["amount"], -88.0);
    assert_eq!(
        accepted_incoming_anchor_transfer["bill"]["source_account_id"],
        10
    );
    assert_eq!(
        accepted_incoming_anchor_transfer["bill"]["destination_account_id"],
        11
    );
    assert_eq!(
        accepted_incoming_anchor_transfer["bill"]["destination_amount"],
        88.0
    );
    assert_eq!(accepted_incoming_anchor_transfer["mergedBillId"], 706);
    assert_eq!(bill_count(&connection, 706)?, 0);
    assert_eq!(bill_count(&connection, 707)?, 1);

    let rejected_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:103:transfer:104",
        "reject",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(rejected_transfer["action"], "reject");
    let suppressed_transfer =
        query_matching_bill_candidates_payload(&connection, user_id, 103)?.expect("suppressed");
    assert!(!candidate_ids(&suppressed_transfer).contains(&"bill:103:transfer:104".to_string()));

    let accepted_investment = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:201:investment:202",
        "accept",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(accepted_investment["pair"]["pairType"], "investment");
    let rejected_investment = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:203:investment:204",
        "reject",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(rejected_investment["action"], "reject");

    let accepted_learning = apply_matching_candidate_action(
        &mut connection,
        user_id,
        &learning_candidate_id,
        "accept",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(accepted_learning["bill"]["main_category"], "Food");
    let accepted_learning_rule_count: i64 = connection.query_row(
        "SELECT applied_count FROM import_learning_rules WHERE id = 8",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(accepted_learning_rule_count, 1);
    let post_learning_payload =
        query_matching_bill_candidates_payload(&connection, user_id, 301)?.expect("post learning");
    assert!(!candidate_ids(&post_learning_payload).contains(&learning_candidate_id));

    let learning_reject_payload =
        query_matching_bill_candidates_payload(&connection, user_id, 302)?.expect("learning 302");
    let learning_reject_id = candidate_ids(&learning_reject_payload)
        .into_iter()
        .find(|candidate_id| candidate_id.starts_with("bill:302:learning:8:"))
        .expect("learning reject candidate id");
    let rejected_learning = apply_matching_candidate_action(
        &mut connection,
        user_id,
        &learning_reject_id,
        "reject",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(rejected_learning["action"], "reject");

    let accepted_preview_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "accept",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            reviewed_type: Some("transfer".to_string()),
            ..Default::default()
        },
    )?;
    assert_eq!(
        accepted_preview_transfer["preview_item"]["matching"]["transfer"]["review_status"],
        "accepted"
    );
    let cleared_preview_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "clear",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            ..Default::default()
        },
    )?;
    assert!(cleared_preview_transfer["preview_item"]["matching"]
        .get("transfer")
        .is_none());

    let accepted_preview_learning = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:2:learning",
        "accept",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            learning_apply: Some(ImportPreviewLearningApply {
                preview_type: Some("expense".to_string()),
                preview_main_category: Some("Food".to_string()),
                preview_sub_category: Some("Coffee".to_string()),
                preview_source_account_id: Some(Some(10)),
                preview_destination_account_id: Some(None),
                rule_id: Some(8),
            }),
            ..Default::default()
        },
    )?;
    assert_eq!(
        accepted_preview_learning["preview_item"]["matching"]["learning"]["review_status"],
        "accepted"
    );
    let rejected_preview_learning = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:2:learning",
        "reject",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            ..Default::default()
        },
    )?;
    assert_eq!(
        rejected_preview_learning["preview_item"]["matching"]["learning"]["review_status"],
        "rejected"
    );

    let recurring_candidate = ImportPreviewRecurringCandidate {
        id: 900,
        name: "Monthly Rent".to_string(),
        match_score: 0.91,
        match_reasons: vec!["same_amount".to_string(), "monthly".to_string()],
        matched_occurrence_date: "2026-04-01".to_string(),
    };
    let accepted_preview_recurring = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:3:recurring",
        "accept",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            recurring_id: Some(900),
            recurring_candidate_count: 1,
            recurring_candidate: Some(recurring_candidate.clone()),
            ..Default::default()
        },
    )?;
    assert_eq!(accepted_preview_recurring["recurring_id"], 900);
    let rejected_preview_recurring = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:3:recurring",
        "reject",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            ..Default::default()
        },
    )?;
    assert!(rejected_preview_recurring["preview_item"]["matching"]["recurring"]["id"].is_null());

    let conflict = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "accept",
        &PreviewMatchingActionRequest {
            expected_state: Some(ImportPreviewExpectedState {
                session_id: Some("wrong-session".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .expect_err("stale preview expected state");
    assert_eq!(conflict.status_code(), 409);
    assert_eq!(conflict.message(), "Preview row changed, please refresh");

    let missing_recurring = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:3:recurring",
        "accept",
        &PreviewMatchingActionRequest::default(),
    )
    .expect_err("missing recurring id");
    assert_eq!(missing_recurring.status_code(), 400);

    let reconciliation_list = list_reconciliation_candidates_payload(
        &connection,
        user_id,
        &ReconciliationCandidateFilters {
            session_id: Some("session-matching".to_string()),
            preview_id: Some(3),
            existing_bill_id: Some(401),
            candidate_type: Some("duplicate".to_string()),
            status: Some("pending".to_string()),
            limit: 25,
        },
    )?;
    assert_eq!(
        reconciliation_list["candidates"][0]["candidateId"],
        reconciliation_id()
    );
    let reconciliation_bill =
        query_matching_bill_candidates_payload(&connection, user_id, 401)?.expect("reconcile bill");
    assert!(candidate_ids(&reconciliation_bill)
        .into_iter()
        .any(|candidate_id| candidate_id == reconciliation_id()));
    let accepted_reconciliation = apply_matching_candidate_action(
        &mut connection,
        user_id,
        &reconciliation_id(),
        "accept",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(accepted_reconciliation["projection"]["bill_id"], 401);
    assert_eq!(
        accepted_reconciliation["projection"]["description"],
        "Existing subscription|Imported subscription note"
    );
    assert_eq!(
        accepted_reconciliation["projection"]["tag_ids"],
        json!([71, 72])
    );
    assert_eq!(
        bill_description(&connection, 401)?,
        "Existing subscription|Imported subscription note"
    );
    assert_eq!(bill_tag_ids(&connection, 401)?, vec![71, 72]);
    assert!(!preview_selected(&connection, 3)?);
    let projection = query_matching_bill_candidates_payload(&connection, user_id, 401)?
        .expect("projection after merge");
    assert_eq!(projection["reconciliation"]["status"], "merged");
    let cleared_reconciliation = apply_matching_candidate_action(
        &mut connection,
        user_id,
        &reconciliation_id(),
        "clear",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(
        cleared_reconciliation["projection"]["description"],
        "Existing subscription"
    );
    assert_eq!(cleared_reconciliation["projection"]["tag_ids"], json!([71]));
    assert_eq!(
        cleared_reconciliation["projection"]["candidate_ids"],
        json!([])
    );
    assert_eq!(bill_description(&connection, 401)?, "Existing subscription");
    assert_eq!(bill_tag_ids(&connection, 401)?, vec![71]);
    assert!(preview_selected(&connection, 3)?);
    let projection_after_clear = query_matching_bill_candidates_payload(&connection, user_id, 401)?
        .expect("projection after clear");
    assert!(projection_after_clear["reconciliation"].is_null());

    apply_matching_candidate_action(
        &mut connection,
        user_id,
        &reconciliation_id(),
        "accept",
        &PreviewMatchingActionRequest::default(),
    )?;
    let rejected_reconciliation = apply_matching_candidate_action(
        &mut connection,
        user_id,
        &reconciliation_id(),
        "reject",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(rejected_reconciliation["action"], "reject");
    assert_eq!(bill_description(&connection, 401)?, "Existing subscription");
    assert_eq!(bill_tag_ids(&connection, 401)?, vec![71]);
    assert!(preview_selected(&connection, 3)?);

    apply_matching_candidate_action(
        &mut connection,
        user_id,
        &reconciliation_id(),
        "accept",
        &PreviewMatchingActionRequest::default(),
    )?;
    connection.execute(
        "UPDATE bills SET description = 'Manual override after merge' WHERE id = 401",
        [],
    )?;
    let stale_clear = apply_matching_candidate_action(
        &mut connection,
        user_id,
        &reconciliation_id(),
        "clear",
        &PreviewMatchingActionRequest::default(),
    )
    .expect_err("stale projection conflict");
    assert_eq!(stale_clear.status_code(), 409);
    assert_eq!(
        stale_clear.message(),
        "Bill changed since reconciliation projection, please refresh"
    );
    assert_eq!(
        bill_description(&connection, 401)?,
        "Manual override after merge"
    );

    let accepted_transfer_reconciliation = apply_matching_candidate_action(
        &mut connection,
        user_id,
        &transfer_reconciliation_id(),
        "accept",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(
        accepted_transfer_reconciliation["projection"]["signal_label"],
        "\u{5339}\u{914d}\u{ff1a}\u{4eba}\u{5de5}|\u{652f}\u{4ed8}\u{5b9d}"
    );
    assert_eq!(
        accepted_transfer_reconciliation["projection"]["description"],
        "Existing transfer|Imported transfer note"
    );
    assert!(!preview_selected(&connection, 1)?);
    let cleared_transfer_reconciliation = apply_matching_candidate_action(
        &mut connection,
        user_id,
        &transfer_reconciliation_id(),
        "clear",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert_eq!(
        cleared_transfer_reconciliation["projection"]["description"],
        "Existing transfer"
    );
    assert!(preview_selected(&connection, 1)?);

    let bad_candidate = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "not-a-candidate",
        "accept",
        &PreviewMatchingActionRequest::default(),
    )
    .expect_err("invalid candidate");
    assert_eq!(bad_candidate.status_code(), 400);
    assert_eq!(bad_candidate.message(), "Invalid candidateId");

    Ok(())
}

#[test]
fn preview_actionable_decisions_restore_rule_account_baseline_without_overwriting_manual_drift(
) -> Result<(), Box<dyn Error>> {
    let mut connection = Connection::open_in_memory()?;
    seed_matching_fixture(&mut connection)?;
    let user_id = user_id();

    connection.execute("UPDATE categories SET type = 3 WHERE id = 6", [])?;
    connection.execute(
        "
        UPDATE bills_preview
        SET preview_type = '转账',
            preview_main_category = '内部转账',
            preview_sub_category = '账户互转',
            preview_source_account_id = 10,
            preview_destination_account_id = 11,
            preview_matching_feedback_json = ?1
        WHERE id = 1
        ",
        params![json!({
            "category_rule": {
                "category_id": 6,
                "review_status": "auto_applied"
            },
            "account": {
                "source_account_id": 10,
                "review_status": "auto_applied"
            },
            "stage2_baseline": {
                "preview_type": "支出",
                "preview_main_category": "Food",
                "preview_sub_category": "Coffee",
                "preview_source_account_id": 10,
                "preview_destination_account_id": null
            },
            "transfer": {
                "candidate_type": "transfer",
                "score": 0.96,
                "level": "high",
                "reason": "opposite_amount",
                "review_status": "pending",
                "applied_preview": {
                    "preview_type": "转账",
                    "preview_main_category": "内部转账",
                    "preview_sub_category": "账户互转",
                    "preview_source_account_id": 10,
                    "preview_destination_account_id": 11,
                    "preview_recurring_id": null,
                    "preview_recurring_name": "",
                    "preview_recurring_candidate_count": 0,
                    "preview_recurring_match_score": 0.0,
                    "preview_recurring_match_reasons": "",
                    "preview_recurring_matched_date": ""
                }
            }
        })
        .to_string()],
    )?;
    connection.execute(
        "UPDATE categories SET main_category = 'Renamed', sub_category = 'Changed', type = 2 WHERE id = 6",
        [],
    )?;

    let rejected_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "reject",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            ..Default::default()
        },
    )?;
    let rejected_item = &rejected_transfer["preview_item"];
    assert_eq!(rejected_item["preview_type"], "支出");
    assert_eq!(rejected_item["preview_main_category"], "Food");
    assert_eq!(rejected_item["preview_sub_category"], "Coffee");
    assert_eq!(rejected_item["preview_source_account_id"], 10);
    assert!(rejected_item["preview_destination_account_id"].is_null());
    assert_eq!(
        rejected_item["matching"]["transfer"]["review_status"],
        "rejected"
    );

    connection.execute(
        "
        UPDATE bills_preview
        SET preview_type = '支出',
            preview_main_category = 'Food',
            preview_sub_category = 'Coffee',
            preview_source_account_id = 10,
            preview_destination_account_id = NULL,
            preview_matching_feedback_json = ?1
        WHERE id = 1
        ",
        params![json!({
            "transfer": {
                "candidate_type": "transfer",
                "score": 0.96,
                "level": "high",
                "reason": "opposite_amount",
                "review_status": "pending",
                "applied_preview": {
                    "preview_type": "转账",
                    "preview_main_category": "内部转账",
                    "preview_sub_category": "账户互转",
                    "preview_source_account_id": 10,
                    "preview_destination_account_id": 11,
                    "preview_recurring_id": null,
                    "preview_recurring_name": "",
                    "preview_recurring_candidate_count": 0,
                    "preview_recurring_match_score": 0.0,
                    "preview_recurring_match_reasons": "",
                    "preview_recurring_matched_date": ""
                }
            }
        })
        .to_string()],
    )?;
    let _accepted_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "accept",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            reviewed_type: Some("转账".to_string()),
            ..Default::default()
        },
    )?;
    connection.execute(
        "
        UPDATE bills_preview
        SET preview_type = '支出',
            preview_main_category = 'Manual',
            preview_sub_category = 'Edited',
            preview_source_account_id = 11,
            preview_destination_account_id = NULL
        WHERE id = 1
        ",
        [],
    )?;
    let cleared_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "clear",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            ..Default::default()
        },
    )?;
    let cleared_item = &cleared_transfer["preview_item"];
    assert_eq!(cleared_item["preview_main_category"], "Manual");
    assert_eq!(cleared_item["preview_sub_category"], "Edited");
    assert_eq!(cleared_item["preview_source_account_id"], 11);
    assert!(cleared_item["matching"].get("transfer").is_none());

    Ok(())
}

#[test]
fn preview_actionable_decisions_fall_back_to_legacy_rule_account_baseline(
) -> Result<(), Box<dyn Error>> {
    let mut connection = Connection::open_in_memory()?;
    seed_matching_fixture(&mut connection)?;
    let user_id = user_id();

    connection.execute("UPDATE categories SET type = 3 WHERE id = 6", [])?;
    connection.execute(
        "
        UPDATE bills_preview
        SET preview_type = '转账',
            preview_main_category = '内部转账',
            preview_sub_category = '账户互转',
            preview_source_account_id = 10,
            preview_destination_account_id = 11,
            preview_matching_feedback_json = ?1
        WHERE id = 1
        ",
        params![json!({
            "category_rule": {
                "category_id": "6",
                "review_status": "auto_applied"
            },
            "account": {
                "source_account_id": "10",
                "destination_account_id": "11",
                "review_status": "auto_applied"
            },
            "stage2_baseline": {
                "legacy": true
            },
            "transfer": {
                "candidate_type": "transfer",
                "score": 0.96,
                "level": "high",
                "reason": "opposite_amount",
                "review_status": "pending",
                "applied_preview": {
                    "preview_type": "转账",
                    "preview_main_category": "内部转账",
                    "preview_sub_category": "账户互转",
                    "preview_source_account_id": 10,
                    "preview_destination_account_id": 11,
                    "preview_recurring_id": null,
                    "preview_recurring_name": "",
                    "preview_recurring_candidate_count": 0,
                    "preview_recurring_match_score": 0.0,
                    "preview_recurring_match_reasons": "",
                    "preview_recurring_matched_date": ""
                }
            }
        })
        .to_string()],
    )?;

    let rejected_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "reject",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            ..Default::default()
        },
    )?;
    let rejected_item = &rejected_transfer["preview_item"];
    assert_eq!(rejected_item["preview_type"], "支出");
    assert_eq!(rejected_item["preview_main_category"], "Food");
    assert_eq!(rejected_item["preview_sub_category"], "Coffee");
    assert_eq!(rejected_item["preview_source_account_id"], 10);
    assert_eq!(rejected_item["preview_destination_account_id"], 11);
    assert_eq!(
        rejected_item["matching"]["transfer"]["review_status"],
        "rejected"
    );

    Ok(())
}

#[test]
fn preview_pending_rejects_do_not_overwrite_manual_drift() -> Result<(), Box<dyn Error>> {
    let mut connection = Connection::open_in_memory()?;
    seed_matching_fixture(&mut connection)?;
    let user_id = user_id();

    connection.execute(
        "
        UPDATE bills_preview
        SET preview_type = '转账',
            preview_main_category = '内部转账',
            preview_sub_category = '账户互转',
            preview_source_account_id = 10,
            preview_destination_account_id = 11,
            preview_matching_feedback_json = ?1
        WHERE id = 1
        ",
        params![json!({
            "stage2_baseline": {
                "preview_type": "支出",
                "preview_main_category": "Food",
                "preview_sub_category": "Coffee",
                "preview_source_account_id": 10,
                "preview_destination_account_id": null
            },
            "transfer": {
                "candidate_type": "transfer",
                "review_status": "pending",
                "applied_preview": {
                    "preview_type": "转账",
                    "preview_main_category": "内部转账",
                    "preview_sub_category": "账户互转",
                    "preview_source_account_id": 10,
                    "preview_destination_account_id": 11,
                    "preview_recurring_id": null,
                    "preview_recurring_name": "",
                    "preview_recurring_candidate_count": 0,
                    "preview_recurring_match_score": 0.0,
                    "preview_recurring_match_reasons": "",
                    "preview_recurring_matched_date": ""
                }
            }
        })
        .to_string()],
    )?;
    connection.execute(
        "
        UPDATE bills_preview
        SET preview_type = '支出',
            preview_main_category = 'Manual',
            preview_sub_category = 'Edited',
            preview_source_account_id = 11,
            preview_destination_account_id = NULL
        WHERE id = 1
        ",
        [],
    )?;

    let rejected_transfer = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "reject",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            ..Default::default()
        },
    )?;
    let rejected_transfer_item = &rejected_transfer["preview_item"];
    assert_eq!(rejected_transfer_item["preview_main_category"], "Manual");
    assert_eq!(rejected_transfer_item["preview_sub_category"], "Edited");
    assert_eq!(rejected_transfer_item["preview_source_account_id"], 11);
    assert_eq!(
        rejected_transfer_item["matching"]["transfer"]["review_status"],
        "rejected"
    );

    connection.execute(
        "
        UPDATE bills_preview
        SET preview_type = '支出',
            preview_main_category = 'Travel',
            preview_sub_category = 'Metro',
            preview_source_account_id = 11,
            preview_destination_account_id = NULL,
            preview_matching_feedback_json = ?1
        WHERE id = 2
        ",
        params![json!({
            "stage2_baseline": {
                "preview_type": "支出",
                "preview_main_category": "Food",
                "preview_sub_category": "Coffee",
                "preview_source_account_id": 10,
                "preview_destination_account_id": null
            },
            "learning": {
                "rule_id": 8,
                "review_status": "auto_applied",
                "applied_preview": {
                    "preview_type": "支出",
                    "preview_main_category": "Travel",
                    "preview_sub_category": "Metro",
                    "preview_source_account_id": 11,
                    "preview_destination_account_id": null
                }
            },
            "llm": {
                "suggested_main_category": "Travel",
                "suggested_sub_category": "Metro",
                "suggested_source_account": "Card",
                "suggested_destination_account": "",
                "confidence": 0.86,
                "reason": "merchant pattern",
                "review_status": "pending",
                "previous_preview": {
                    "preview_main_category": "Food",
                    "preview_sub_category": "Coffee",
                    "preview_source_account_id": 10,
                    "preview_destination_account_id": null
                },
                "applied_preview": {
                    "preview_main_category": "Travel",
                    "preview_sub_category": "Metro",
                    "preview_source_account_id": 11,
                    "preview_destination_account_id": null
                }
            }
        })
        .to_string()],
    )?;
    connection.execute(
        "
        UPDATE bills_preview
        SET preview_main_category = 'Manual',
            preview_sub_category = 'Edited',
            preview_source_account_id = 10
        WHERE id = 2
        ",
        [],
    )?;

    let rejected_learning = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:2:learning",
        "reject",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            ..Default::default()
        },
    )?;
    let rejected_learning_item = &rejected_learning["preview_item"];
    assert_eq!(rejected_learning_item["preview_main_category"], "Manual");
    assert_eq!(rejected_learning_item["preview_sub_category"], "Edited");
    assert_eq!(rejected_learning_item["preview_source_account_id"], 10);
    assert_eq!(
        rejected_learning_item["matching"]["learning"]["review_status"],
        "rejected"
    );

    let suggestion = ImportPreviewLlmSuggestion {
        suggested_main_category: "Travel".to_string(),
        suggested_sub_category: "Metro".to_string(),
        suggested_source_account: "Card".to_string(),
        suggested_destination_account: String::new(),
        resolved_source_account_id: Some(11),
        resolved_destination_account_id: None,
        confidence: 0.86,
        reason: "merchant pattern".to_string(),
    };
    let llm_rejected = bill_analyser_db::review_preview_llm_recommendation(
        &mut connection,
        bill_analyser_db::ImportPreviewLlmReviewRequest {
            session_id: "session-matching",
            preview_id: 2,
            user_id,
            decision: bill_analyser_db::ImportPreviewDecision::Reject,
            suggestion: Some(&suggestion),
            user_correction_category: None,
            user_correction_account: None,
        },
    )?;
    assert!(!llm_rejected.restored);
    let llm_rejected_item = llm_rejected.preview.expect("llm reject preview");
    assert_eq!(llm_rejected_item.preview_main_category, "Manual");
    assert_eq!(llm_rejected_item.preview_sub_category, "Edited");
    assert_eq!(llm_rejected_item.preview_source_account_id, Some(10));
    assert_eq!(
        llm_rejected_item.preview_matching_feedback["llm"]["review_status"],
        "rejected"
    );

    Ok(())
}

#[test]
fn preview_llm_recommendation_keeps_transfer_and_rejects_to_rule_account_baseline(
) -> Result<(), Box<dyn Error>> {
    let mut connection = Connection::open_in_memory()?;
    seed_matching_fixture(&mut connection)?;
    let user_id = user_id();

    connection.execute("UPDATE categories SET type = 3 WHERE id = 6", [])?;
    connection.execute(
        "
        UPDATE bills_preview
        SET preview_main_category = '',
            preview_sub_category = '',
            preview_source_account_id = NULL,
            preview_destination_account_id = NULL,
            preview_matching_feedback_json = ?1
        WHERE id = 2
        ",
        params![json!({
            "category_rule": {
                "category_id": 6,
                "review_status": "auto_applied"
            },
            "account": {
                "source_account_id": 10,
                "review_status": "auto_applied"
            },
            "stage2_baseline": {
                "preview_type": "支出",
                "preview_main_category": "Food",
                "preview_sub_category": "Coffee",
                "preview_source_account_id": 10,
                "preview_destination_account_id": null
            },
            "transfer": {
                "candidate_type": "transfer",
                "review_status": "pending"
            }
        })
        .to_string()],
    )?;
    connection.execute(
        "UPDATE categories SET main_category = 'Renamed', sub_category = 'Changed', type = 2 WHERE id = 6",
        [],
    )?;

    let suggestion = ImportPreviewLlmSuggestion {
        suggested_main_category: "Travel".to_string(),
        suggested_sub_category: "Metro".to_string(),
        suggested_source_account: "Card".to_string(),
        suggested_destination_account: String::new(),
        resolved_source_account_id: Some(11),
        resolved_destination_account_id: None,
        confidence: 0.86,
        reason: "merchant pattern".to_string(),
    };
    let applied = apply_preview_llm_recommendation(
        &mut connection,
        ImportPreviewLlmApplyRequest {
            session_id: "session-matching",
            preview_id: 2,
            user_id,
            suggestion: &suggestion,
            prompt_text: Some("fixture prompt"),
            llm_provider: Some("fixture"),
            llm_model: Some("fixture-model"),
        },
    )?;
    assert_eq!(applied.applied_fields.len(), 3);
    let after_apply = get_preview_bill_by_id(&connection, 2, user_id)?.expect("preview row");
    assert_eq!(
        after_apply.preview_matching_feedback["transfer"]["review_status"],
        "pending"
    );

    let rejected = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:2:learning",
        "reject",
        &PreviewMatchingActionRequest {
            response_mode_preview_item: true,
            ..Default::default()
        },
    )?;
    assert_eq!(
        rejected["preview_item"]["matching"]["learning"]["review_status"],
        "rejected"
    );

    let llm_rejected = bill_analyser_db::review_preview_llm_recommendation(
        &mut connection,
        bill_analyser_db::ImportPreviewLlmReviewRequest {
            session_id: "session-matching",
            preview_id: 2,
            user_id,
            decision: bill_analyser_db::ImportPreviewDecision::Reject,
            suggestion: Some(&suggestion),
            user_correction_category: None,
            user_correction_account: None,
        },
    )?;
    let restored = llm_rejected.preview.expect("preview after llm reject");
    assert_eq!(restored.preview_main_category, "Food");
    assert_eq!(restored.preview_sub_category, "Coffee");
    assert_eq!(restored.preview_source_account_id, Some(10));
    assert_eq!(restored.preview_destination_account_id, None);
    assert_eq!(
        restored.preview_matching_feedback["llm"]["review_status"],
        "rejected"
    );

    Ok(())
}

#[test]
fn matching_runtime_candidate_action_error_edges_are_mapped() -> Result<(), Box<dyn Error>> {
    let mut connection = Connection::open_in_memory()?;
    seed_matching_fixture(&mut connection)?;
    let user_id = user_id();

    let missing_pair_bill = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:101:transfer:999",
        "reject",
        &PreviewMatchingActionRequest::default(),
    )
    .expect_err("missing pair bill");
    assert_eq!(missing_pair_bill.status_code(), 404);
    assert_eq!(missing_pair_bill.message(), "Bill not found");

    create_manual_matching_pair(
        &mut connection,
        user_id,
        101,
        102,
        "transfer",
        Some("bill:101:transfer:102"),
    )?;
    let already_paired = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:101:transfer:102",
        "reject",
        &PreviewMatchingActionRequest::default(),
    )
    .expect_err("already paired rejection");
    assert_eq!(already_paired.status_code(), 409);
    assert_eq!(
        already_paired.message(),
        "Bills already belong to an existing transfer pair"
    );

    connection.execute(
        "
        INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description, payment_method,
            main_category, sub_category, source_account_id, destination_account_id, destination_amount,
            created_at, updated_at
        ) VALUES
            (501, 42, '2026-04-09T09:00:00', 'expense', -70.0, 'Same Account', 'Out', 'cash', 'Transfer', '', 10, 0, 0, 'now', 'now'),
            (502, 42, '2026-04-09T09:05:00', 'income', 70.0, 'Same Account', 'In', 'cash', 'Transfer', '', 10, 0, 0, 'now', 'now')
        ",
        [],
    )?;
    let ineligible_pair = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:501:transfer:502",
        "reject",
        &PreviewMatchingActionRequest::default(),
    )
    .expect_err("ineligible pair");
    assert_eq!(ineligible_pair.status_code(), 409);
    assert_eq!(
        ineligible_pair.message(),
        "Bills are not eligible for transfer pairing"
    );

    let stale_learning = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "bill:302:learning:8:stale",
        "reject",
        &PreviewMatchingActionRequest::default(),
    )
    .expect_err("stale learning candidate");
    assert_eq!(stale_learning.status_code(), 400);
    assert_eq!(stale_learning.message(), "Learning candidate not available");

    let invalid_preview_action = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "archive",
        &PreviewMatchingActionRequest::default(),
    )
    .expect_err("invalid preview action");
    assert_eq!(invalid_preview_action.status_code(), 400);
    assert_eq!(
        invalid_preview_action.message(),
        "Candidate family not supported"
    );

    let invalid_reconciliation_action = apply_matching_candidate_action(
        &mut connection,
        user_id,
        &reconciliation_id(),
        "archive",
        &PreviewMatchingActionRequest::default(),
    )
    .expect_err("invalid reconciliation action");
    assert_eq!(invalid_reconciliation_action.status_code(), 400);
    assert_eq!(
        invalid_reconciliation_action.message(),
        "Candidate family not supported"
    );

    let preview_array_payload = apply_matching_candidate_action(
        &mut connection,
        user_id,
        "preview:1:transfer",
        "reject",
        &PreviewMatchingActionRequest::default(),
    )?;
    assert!(preview_array_payload["preview"].is_array());
    assert!(preview_array_payload.get("preview_item").is_none());

    let invalid_pair_type =
        create_manual_matching_pair(&mut connection, user_id, 103, 104, "dup", None)
            .expect_err("invalid pair type");
    assert_eq!(invalid_pair_type.status_code(), 400);
    assert_eq!(invalid_pair_type.message(), "Invalid pairType");

    Ok(())
}

#[test]
fn matching_bill_candidates_tolerate_legacy_learning_rule_shape() -> Result<(), Box<dyn Error>> {
    let mut connection = Connection::open_in_memory()?;
    seed_matching_fixture(&mut connection)?;
    let user_id = user_id();

    connection.execute_batch(
        "
        DROP TABLE import_learning_rules;
        CREATE TABLE import_learning_rules(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            match_type TEXT
        );
        INSERT INTO import_learning_rules(id, user_id, match_type)
        VALUES (88, 42, 'legacy');
        ",
    )?;

    let payload = query_matching_bill_candidates_payload(&connection, user_id, 101)?
        .expect("valid bill candidates");
    let ids = candidate_ids(&payload);

    assert!(ids.contains(&"bill:101:transfer:102".to_string()));
    assert!(ids.iter().all(|id| !id.starts_with("bill:101:learning:")));
    Ok(())
}

#[test]
fn matching_bill_candidates_support_current_learning_rule_shape_without_confidence(
) -> Result<(), Box<dyn Error>> {
    let mut connection = Connection::open_in_memory()?;
    seed_matching_fixture(&mut connection)?;
    let user_id = user_id();

    connection.execute_batch(
        "
        ALTER TABLE import_learning_rules RENAME TO import_learning_rules_with_confidence;
        CREATE TABLE import_learning_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            match_type TEXT NOT NULL,
            match_value TEXT NOT NULL,
            normalized_match_value TEXT NOT NULL,
            learned_type TEXT,
            learned_category_id INTEGER,
            learned_source_account_id INTEGER,
            learned_destination_account_id INTEGER,
            enabled INTEGER NOT NULL DEFAULT 1,
            source_session_id TEXT,
            source_preview_id INTEGER,
            parser_id TEXT,
            composite_match_hash TEXT,
            match_features_json TEXT,
            applied_count INTEGER NOT NULL DEFAULT 0,
            last_applied_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, match_type, normalized_match_value)
        );
        INSERT INTO import_learning_rules(
            id, user_id, match_type, match_value, normalized_match_value,
            learned_type, learned_category_id, learned_source_account_id,
            learned_destination_account_id, enabled, parser_id, composite_match_hash,
            match_features_json, applied_count, last_applied_at, created_at, updated_at
        )
        SELECT
            id, user_id, match_type, match_value, normalized_match_value,
            learned_type, learned_category_id, learned_source_account_id,
            learned_destination_account_id, enabled, parser_id, composite_match_hash,
            match_features_json, applied_count, last_applied_at, created_at, updated_at
        FROM import_learning_rules_with_confidence;
        DROP TABLE import_learning_rules_with_confidence;
        ",
    )?;

    let payload = query_matching_bill_candidates_payload(&connection, user_id, 301)?
        .expect("learning candidates");
    let ids = candidate_ids(&payload);

    assert!(ids.iter().any(|id| id.starts_with("bill:301:learning:8:")));
    Ok(())
}

fn seed_matching_fixture(connection: &mut Connection) -> Result<(), Box<dyn Error>> {
    create_business_schema(connection)?;
    init_import_staging_schema(connection)?;
    init_matching_runtime_schema(connection)?;
    insert_core_rows(connection)?;
    seed_import_preview_rows(connection)?;
    seed_reconciliation_rows(connection)?;
    Ok(())
}

fn create_business_schema(connection: &Connection) -> Result<(), Box<dyn Error>> {
    connection.execute_batch(
        "
        CREATE TABLE users(
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            import_learning_enabled INTEGER DEFAULT 1,
            investment_platform_keywords TEXT,
            investment_product_keywords TEXT,
            investment_exclude_keywords TEXT
        );
        CREATE TABLE accounts(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            type TEXT,
            balance REAL,
            initial_balance REAL,
            currency TEXT,
            icon TEXT,
            hidden INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT
        );
        CREATE TABLE categories(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            main_category TEXT,
            sub_category TEXT,
            name TEXT,
            type INTEGER
        );
        CREATE TABLE tags(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            color TEXT,
            icon TEXT,
            display_order INTEGER DEFAULT 0,
            hidden INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT
        );
        CREATE TABLE bills(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT NOT NULL,
            description TEXT NOT NULL,
            payment_method TEXT DEFAULT '',
            main_category TEXT,
            sub_category TEXT,
            batch_id TEXT,
            hash TEXT,
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0,
            created_from_template INTEGER,
            created_from_recurring INTEGER,
            import_history_id INTEGER,
            created_at TEXT,
            updated_at TEXT
        );
        CREATE TABLE bill_tags(
            bill_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (bill_id, tag_id)
        );
        CREATE TABLE import_learning_rules(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            match_type TEXT,
            match_value TEXT,
            normalized_match_value TEXT,
            learned_type TEXT,
            learned_category_id INTEGER,
            learned_source_account_id INTEGER,
            learned_destination_account_id INTEGER,
            enabled INTEGER DEFAULT 1,
            parser_id TEXT,
            composite_match_hash TEXT,
            match_features_json TEXT,
            applied_count INTEGER DEFAULT 0,
            confidence REAL DEFAULT 1,
            support_count INTEGER DEFAULT 1,
            last_applied_at TEXT,
            created_at TEXT,
            updated_at TEXT
        );
        CREATE TABLE import_learning_rule_logs(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            rule_id INTEGER NOT NULL,
            action TEXT NOT NULL,
            payload_json TEXT,
            created_at TEXT
        );
        ",
    )?;
    Ok(())
}

fn insert_core_rows(connection: &Connection) -> Result<(), Box<dyn Error>> {
    connection.execute(
        "
        INSERT INTO users(
            id, username, import_learning_enabled,
            investment_platform_keywords, investment_product_keywords, investment_exclude_keywords
        ) VALUES (42, 'owner', 1, ?1, ?2, '[]')
        ",
        params![
            json!(["Acme Invest"]).to_string(),
            json!(["Index Fund"]).to_string()
        ],
    )?;
    connection.execute(
        "INSERT INTO users(id, username, import_learning_enabled) VALUES (77, 'other', 1)",
        [],
    )?;
    connection.execute(
        "
        INSERT INTO accounts(id, user_id, name, type, balance, initial_balance, currency, icon, hidden, created_at, updated_at)
        VALUES (10, 42, 'Cash', 'cash', 1000.0, 0.0, 'CNY', 'wallet', 0, 'now', 'now'),
               (11, 42, 'Card', 'cash', 500.0, 0.0, 'CNY', 'card', 0, 'now', 'now'),
               (12, 42, 'Brokerage', 'investment', 0.0, 0.0, 'CNY', 'chart', 0, 'now', 'now'),
               (99, 77, 'Other', 'cash', 1.0, 0.0, 'CNY', 'wallet', 0, 'now', 'now')
        ",
        [],
    )?;
    connection.execute(
        "INSERT INTO categories(id, user_id, main_category, sub_category, name, type)
         VALUES (6, 42, 'Food', 'Coffee', 'Coffee', 0)",
        [],
    )?;
    connection.execute(
        "
        INSERT INTO tags(id, user_id, name, created_at, updated_at)
        VALUES (71, 42, 'Manual', 'now', 'now'),
               (72, 42, 'Import', 'now', 'now'),
               (73, 77, 'Other user tag', 'now', 'now')
        ",
        [],
    )?;
    connection.execute(
        "
        INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description, payment_method,
            main_category, sub_category, source_account_id, destination_account_id, destination_amount,
            created_at, updated_at
        ) VALUES
            (101, 42, '2026-04-01T09:00:00', 'expense', -50.0, 'Cash', 'Transfer out', 'cash', 'Transfer', '', 10, 0, 0, 'now', 'now'),
            (102, 42, '2026-04-01T09:04:00', 'income', 50.0, 'Card', 'Transfer in', 'card', 'Transfer', '', 11, 0, 0, 'now', 'now'),
            (103, 42, '2026-04-02T09:00:00', 'expense', -60.0, 'Cash', 'Reject transfer out', 'cash', 'Transfer', '', 10, 0, 0, 'now', 'now'),
            (104, 42, '2026-04-02T09:04:00', 'income', 60.0, 'Card', 'Reject transfer in', 'card', 'Transfer', '', 11, 0, 0, 'now', 'now'),
            (201, 42, '2026-04-03T10:00:00', 'expense', -100.0, 'Acme Invest', 'Buy Index Fund', 'cash', 'Investment', 'Fund', 10, 0, 0, 'now', 'now'),
            (202, 42, '2026-04-03T10:30:00', 'income', 100.0, 'Acme Invest', 'Sell Index Fund', 'brokerage', 'Investment', 'Fund', 12, 0, 0, 'now', 'now'),
            (203, 42, '2026-04-04T10:00:00', 'expense', -120.0, 'Acme Invest', 'Buy Index Fund', 'cash', 'Investment', 'Fund', 10, 0, 0, 'now', 'now'),
            (204, 42, '2026-04-04T10:30:00', 'income', 120.0, 'Acme Invest', 'Sell Index Fund', 'brokerage', 'Investment', 'Fund', 12, 0, 0, 'now', 'now'),
            (301, 42, '2026-04-05T08:00:00', 'expense', -4.5, 'Coffee Shop', 'Latte', 'card', '', '', 11, 0, 0, 'now', 'now'),
            (302, 42, '2026-04-06T08:00:00', 'expense', -5.5, 'Coffee Shop', 'Latte', 'card', '', '', 11, 0, 0, 'now', 'now'),
            (401, 42, '2026-04-07T08:00:00', 'expense', -20.0, 'Subscription', 'Existing subscription', 'card', 'Life', 'Service', 11, 0, 0, 'now', 'now'),
            (402, 42, '2026-04-08T09:00:00', 'expense', -30.0, 'Transfer peer', 'Existing transfer', 'card', 'Transfer', '', 11, 0, 0, 'now', 'now'),
            (601, 42, '2026-04-09T09:00:00', 'expense', -70.0, 'Strict transfer anchor', 'Out', 'cash', 'Transfer', '', 10, 0, 0, 'now', 'now'),
            (602, 42, '2026-04-10T09:00:00', 'income', 70.0, 'Different day', 'In', 'card', 'Transfer', '', 11, 0, 0, 'now', 'now'),
            (603, 42, '2026-04-09T09:04:00', 'income', 71.0, 'Different amount', 'In', 'card', 'Transfer', '', 11, 0, 0, 'now', 'now'),
            (604, 42, '2026-04-09T09:06:00', 'income', 70.0, 'Outside import window', 'In', 'card', 'Transfer', '', 11, 0, 0, 'now', 'now'),
            (701, 42, '2026-04-11T10:00:00', 'expense', -18.8, 'Coffee Shop', 'Identical coffee', 'wechat', 'Food', 'Coffee', 10, 0, 0, 'now', 'now'),
            (702, 42, '2026-04-11T10:00:00', 'expense', -18.8, 'Coffee Shop', 'Identical coffee', 'wechat', 'Food', 'Coffee', 10, 0, 0, 'now', 'now'),
            (703, 42, '2026-04-11T10:00:00', 'expense', -18.8, 'Coffee Shop', 'Different coffee', 'wechat', 'Food', 'Coffee', 10, 0, 0, 'now', 'now'),
            (704, 42, '2026-04-12T10:00:00', 'expense', -28.8, 'Tea Shop', 'Identical tea', 'wechat', 'Food', 'Tea', 10, 0, 0, 'now', 'now'),
            (705, 42, '2026-04-12T10:00:00', 'expense', -28.8, 'Tea Shop', 'Identical tea', 'wechat', 'Food', 'Tea', 10, 0, 0, 'now', 'now'),
            (706, 42, '2026-04-13T10:00:00', 'expense', -88.0, 'Wallet', 'Reverse anchor transfer out', 'cash', 'Transfer', '', 10, 0, 0, 'now', 'now'),
            (707, 42, '2026-04-13T10:03:00', 'income', 88.0, 'Card', 'Reverse anchor transfer in', 'card', 'Transfer', '', 11, 0, 0, 'now', 'now'),
            (901, 77, '2026-04-01', 'expense', -50.0, 'Other', 'Other user', 'cash', 'Other', '', 99, 0, 0, 'now', 'now')
        ",
        [],
    )?;
    connection.execute(
        "INSERT INTO bill_tags(bill_id, tag_id, created_at) VALUES (401, 71, 'now'), (402, 71, 'now')",
        [],
    )?;
    connection.execute(
        "
        INSERT INTO import_learning_rules(
            id, user_id, match_type, match_value, normalized_match_value,
            learned_type, learned_category_id, learned_source_account_id,
            learned_destination_account_id, enabled, parser_id, composite_match_hash,
            match_features_json, applied_count, confidence, support_count, created_at, updated_at
        ) VALUES (8, 42, 'composite', '', '', 'expense', 6, 10, NULL, 1, '',
                  'coffee-shop-latte-card', ?1, 0, 0.98, 4, 'now', 'now')
        ",
        params![json!({
            "counterparty": "Coffee Shop",
            "description": "Latte",
            "payment_method": "card"
        })
        .to_string()],
    )?;
    Ok(())
}

fn seed_import_preview_rows(connection: &mut Connection) -> Result<(), Box<dyn Error>> {
    create_import_session(
        connection,
        &ImportSessionDraft {
            session_id: "session-matching".to_string(),
            user_id: user_id(),
            file_count: 1,
        },
    )?;
    insert_preview_bills_batch(
        connection,
        "session-matching",
        user_id(),
        &[
            preview_draft("2026-04-08", 50.0, "Preview transfer"),
            preview_draft("2026-04-09", 4.5, "Preview learning"),
            recurring_preview_draft(),
        ],
    )?;
    connection.execute(
        "UPDATE bills_preview SET preview_matching_feedback_json = ?1 WHERE id = 1",
        params![json!({
            "transfer": {
                "candidate_type": "transfer",
                "score": 0.96,
                "level": "high",
                "reason": "opposite_amount",
                "review_status": "pending"
            }
        })
        .to_string()],
    )?;
    connection.execute(
        "UPDATE bills_preview SET preview_matching_feedback_json = ?1 WHERE id = 2",
        params![json!({
            "learning": {
                "rule_id": 8,
                "score": 0.98,
                "level": "high",
                "reason": "counterparty:exact",
                "recommended_type": "expense",
                "summary": "Food / Coffee",
                "review_status": "pending"
            }
        })
        .to_string()],
    )?;
    Ok(())
}

fn seed_reconciliation_rows(connection: &Connection) -> Result<(), Box<dyn Error>> {
    connection.execute(
        "
        INSERT INTO bill_merge_groups(user_id, family, group_key, group_type, status, metadata_json, created_at, updated_at)
        VALUES (42, 'import_reconciliation', 'group-dup-401', 'duplicate', 'pending', ?1, 'now', 'now')
        ",
        params![json!({
            "signal_label": "duplicate import",
            "source_chain": ["import", "dedup"]
        })
        .to_string()],
    )?;
    let group_id = connection.last_insert_rowid();
    let import_snapshot = json!({
        "id": 0,
        "date": "2026-04-07",
        "type": "expense",
        "amount": -20.0,
        "counterparty": "Subscription",
        "description": "Imported subscription note",
        "payment_method": "card",
        "main_category": "Life",
        "sub_category": "Service",
        "tag_ids": [72, 73],
        "source_account_id": 11,
        "destination_account_id": 0
    });
    let existing_snapshot = json!({
        "id": 401,
        "date": "2026-04-07",
        "type": "expense",
        "amount": -20.0,
        "counterparty": "Subscription",
        "description": "Existing subscription",
        "payment_method": "card",
        "main_category": "Life",
        "sub_category": "Service",
        "tag_ids": [71],
        "source_account_id": 11,
        "destination_account_id": 0
    });
    connection.execute(
        "
        INSERT INTO bill_reconciliation_candidates(
            user_id, family, candidate_id, candidate_type, status, session_id, preview_id,
            import_bill_key, existing_bill_id, group_key, amount_abs, time_diff_seconds,
            score, level, reason, import_bill_snapshot_json, existing_bill_snapshot_json,
            source_payload_json, seen_count, first_seen_at, last_seen_at, created_at, updated_at
        ) VALUES (
            42, 'import_reconciliation', ?1, 'duplicate', 'pending', 'session-matching', 3,
            'import-preview-3', 401, 'group-dup-401', 20.0, 0, 0.98, 'high',
            'same_date_amount_counterparty', ?2, ?3, ?4, 1, 'now', 'now', 'now', 'now'
        )
        ",
        params![
            reconciliation_id(),
            import_snapshot.to_string(),
            existing_snapshot.to_string(),
            json!({
                "signal_label": "duplicate import",
                "source_chain": ["import", "dedup"],
            })
            .to_string()
        ],
    )?;
    connection.execute(
        "
        INSERT INTO bill_merge_members(
            group_id, user_id, member_key, member_type, bill_id, import_bill_key,
            candidate_id, role, snapshot_json, created_at, updated_at
        ) VALUES (?1, 42, 'bill-401', 'existing_bill', 401, NULL, ?2, 'canonical', ?3, 'now', 'now'),
                 (?1, 42, 'import-preview-3', 'import_bill', NULL, 'import-preview-3', ?2, 'candidate', ?4, 'now', 'now')
        ",
        params![
            group_id,
            reconciliation_id(),
            existing_snapshot.to_string(),
            import_snapshot.to_string()
        ],
    )?;
    connection.execute(
        "
        INSERT INTO bill_merge_groups(user_id, family, group_key, group_type, status, metadata_json, created_at, updated_at)
        VALUES (42, 'import_reconciliation', 'group-transfer-402', 'transfer', 'pending', ?1, 'now', 'now')
        ",
        params![json!({
            "signal_label": "transfer import",
            "source_chain": ["import", "transfer"]
        })
        .to_string()],
    )?;
    let transfer_group_id = connection.last_insert_rowid();
    let transfer_import_snapshot = json!({
        "id": 0,
        "date": "2026-04-08",
        "type": "income",
        "amount": 30.0,
        "counterparty": "Transfer peer",
        "description": "Imported transfer note",
        "payment_method": "alipay",
        "parser_id": "alipay",
        "main_category": "Transfer",
        "tag_ids": [72],
        "source_account_id": 10,
        "destination_account_id": 0
    });
    let transfer_existing_snapshot = json!({
        "id": 402,
        "date": "2026-04-08",
        "type": "expense",
        "amount": -30.0,
        "counterparty": "Transfer peer",
        "description": "Existing transfer",
        "payment_method": "card",
        "main_category": "Transfer",
        "tag_ids": [71],
        "source_account_id": 11,
        "destination_account_id": 0
    });
    connection.execute(
        "
        INSERT INTO bill_reconciliation_candidates(
            user_id, family, candidate_id, candidate_type, status, session_id, preview_id,
            import_bill_key, existing_bill_id, group_key, amount_abs, time_diff_seconds,
            score, level, reason, import_bill_snapshot_json, existing_bill_snapshot_json,
            source_payload_json, seen_count, first_seen_at, last_seen_at, created_at, updated_at
        ) VALUES (
            42, 'import_reconciliation', ?1, 'transfer', 'pending', 'session-matching', NULL,
            'session:session-matching:template:2', 402, 'group-transfer-402', 30.0, 0, 0.97, 'high',
            'opposite_amount', ?2, ?3, ?4, 1, 'now', 'now', 'now', 'now'
        )
        ",
        params![
            transfer_reconciliation_id(),
            transfer_import_snapshot.to_string(),
            transfer_existing_snapshot.to_string(),
            json!({
                "signal_label": "transfer import",
                "source_chain": ["import", "transfer"],
            })
            .to_string()
        ],
    )?;
    connection.execute(
        "
        INSERT INTO bill_merge_members(
            group_id, user_id, member_key, member_type, bill_id, import_bill_key,
            candidate_id, role, snapshot_json, created_at, updated_at
        ) VALUES (?1, 42, 'bill-402', 'existing_bill', 402, NULL, ?2, 'canonical', ?3, 'now', 'now'),
                 (?1, 42, 'session:session-matching:template:2', 'import_bill', NULL, 'session:session-matching:template:2', ?2, 'candidate', ?4, 'now', 'now')
        ",
        params![
            transfer_group_id,
            transfer_reconciliation_id(),
            transfer_existing_snapshot.to_string(),
            transfer_import_snapshot.to_string()
        ],
    )?;
    Ok(())
}

fn preview_draft(date: &str, amount: f64, description: &str) -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: date.to_string(),
        preview_type: "expense".to_string(),
        preview_amount: amount,
        preview_destination_amount: 0.0,
        preview_main_category: "Food".to_string(),
        preview_sub_category: "Coffee".to_string(),
        preview_source_account_id: Some(10),
        preview_destination_account_id: None,
        preview_counterparty: "Coffee Shop".to_string(),
        preview_payment_method: "card".to_string(),
        preview_description: description.to_string(),
        preview_parser_id: "fixture".to_string(),
        preview_parser_tags: Some(json!(["fixture"])),
        dedup_type: Some("remaining".to_string()),
        dedup_source_ids: vec![1, 2],
        ..Default::default()
    }
}

fn recurring_preview_draft() -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: "2026-04-10".to_string(),
        preview_type: "expense".to_string(),
        preview_amount: 100.0,
        preview_destination_amount: 0.0,
        preview_main_category: "Housing".to_string(),
        preview_sub_category: "Rent".to_string(),
        preview_source_account_id: Some(10),
        preview_counterparty: "Landlord".to_string(),
        preview_payment_method: "cash".to_string(),
        preview_description: "Monthly rent".to_string(),
        preview_parser_id: "fixture".to_string(),
        preview_recurring_candidate_count: 1,
        preview_recurring_match_score: 0.88,
        preview_recurring_match_reasons: "same_amount|monthly".to_string(),
        preview_recurring_matched_date: "2026-03-10".to_string(),
        dedup_type: Some("remaining".to_string()),
        ..Default::default()
    }
}

fn candidate_ids(payload: &Value) -> Vec<String> {
    payload["candidates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|candidate| {
            candidate["candidateId"]
                .as_str()
                .or_else(|| candidate["candidate_id"].as_str())
                .map(str::to_string)
        })
        .collect()
}

fn bill_count(connection: &Connection, bill_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM bills WHERE id = ?1",
        [bill_id],
        |row| row.get(0),
    )?)
}

fn reconciliation_id() -> String {
    "reconcile:import:duplicate:bill:401:preview-3".to_string()
}

fn transfer_reconciliation_id() -> String {
    "reconcile:import:transfer:bill:402:template-2".to_string()
}

fn bill_description(connection: &Connection, bill_id: i64) -> Result<String, Box<dyn Error>> {
    Ok(connection.query_row(
        "SELECT description FROM bills WHERE id = ?",
        params![bill_id],
        |row| row.get(0),
    )?)
}

fn bill_tag_ids(connection: &Connection, bill_id: i64) -> Result<Vec<i64>, Box<dyn Error>> {
    let mut statement =
        connection.prepare("SELECT tag_id FROM bill_tags WHERE bill_id = ? ORDER BY tag_id")?;
    let rows = statement.query_map(params![bill_id], |row| row.get::<_, i64>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn preview_selected(connection: &Connection, preview_id: i64) -> Result<bool, Box<dyn Error>> {
    Ok(connection.query_row(
        "SELECT preview_selected FROM bills_preview WHERE id = ?",
        params![preview_id],
        |row| row.get::<_, i64>(0),
    )? != 0)
}

fn user_id() -> UserId {
    UserId::new(OWNER_ID).expect("valid test user id")
}
