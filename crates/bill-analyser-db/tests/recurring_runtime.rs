use std::error::Error;

use bill_analyser_core::{matching::RecurringPattern, UserId};
use bill_analyser_db::{
    accept_recurring_suggestion, count_recurring_suggestions,
    detect_and_save_recurring_suggestions, get_bills_linked_to_recurring,
    init_recurring_runtime_schema, list_recent_bills_for_recurring_detection,
    list_recurring_suggestions, reject_recurring_suggestion, SqliteConnectionConfig, SqliteDbPath,
    SqliteRuntime,
};
use rusqlite::params;

#[test]
fn recurring_suggestion_repository_persists_updates_and_state_transitions(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = SqliteRuntime::open(SqliteConnectionConfig {
        path: SqliteDbPath::temporary_file(temp_dir.path().join("recurring.db"))?,
        create_if_missing: true,
        busy_timeout: std::time::Duration::from_secs(1),
    })?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE users(id INTEGER PRIMARY KEY, username TEXT NOT NULL);
        CREATE TABLE bills(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT NOT NULL,
            description TEXT NOT NULL,
            main_category TEXT,
            sub_category TEXT,
            source_account_id INTEGER
        );
        INSERT INTO users(id, username) VALUES (42, 'owner');
        ",
    )?;
    init_recurring_runtime_schema(runtime.connection())?;
    let user_id = UserId::new(42)?;

    let created = detect_and_save_recurring_suggestions(
        runtime.connection(),
        user_id,
        &[pattern("stable-rent", "Rent", 0.8)],
    )?;
    assert_eq!(created.created, 1);
    assert_eq!(
        count_recurring_suggestions(runtime.connection(), user_id, Some("pending"))?,
        1
    );

    let updated = detect_and_save_recurring_suggestions(
        runtime.connection(),
        user_id,
        &[pattern("stable-rent", "Rent Updated", 0.9)],
    )?;
    assert_eq!(updated.updated, 1);
    let listed = list_recurring_suggestions(runtime.connection(), user_id, None, 10, 0)?;
    assert_eq!(listed[0]["name"], "Rent Updated");
    assert_eq!(listed[0]["sample_bill_ids"][1], 2);

    let accepted = accept_recurring_suggestion(runtime.connection_mut(), user_id, 1)?
        .expect("accepted suggestion");
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["suggestion_id"], 1);
    let rule_count = runtime.connection().query_row(
        "SELECT COUNT(*) FROM recurring_bills WHERE user_id = ? AND name = ?",
        params![42, "Rent Updated"],
        |row| row.get::<_, i64>(0),
    )?;
    assert_eq!(rule_count, 1);

    let skipped = detect_and_save_recurring_suggestions(
        runtime.connection(),
        user_id,
        &[pattern("stable-rent", "Rent Skipped", 1.0)],
    )?;
    assert_eq!(skipped.skipped, 1);
    assert!(accept_recurring_suggestion(runtime.connection_mut(), user_id, 1)?.is_none());
    assert!(!reject_recurring_suggestion(
        runtime.connection_mut(),
        user_id,
        1
    )?);

    let rejected_created = detect_and_save_recurring_suggestions(
        runtime.connection(),
        user_id,
        &[pattern("stable-phone", "Phone", 0.7)],
    )?;
    assert_eq!(rejected_created.created, 1);
    assert!(reject_recurring_suggestion(
        runtime.connection_mut(),
        user_id,
        2
    )?);
    assert_eq!(
        count_recurring_suggestions(runtime.connection(), user_id, Some("rejected"))?,
        1
    );

    for date in ["2026-01-01", "2026-02-01", "2026-03-01"] {
        runtime.connection().execute(
            "INSERT INTO bills(user_id, date, type, amount, counterparty, description, main_category, sub_category, source_account_id)
             VALUES (42, ?1, 'expense', -12.5, 'Gym', 'Membership', 'Health', 'Fitness', 10)",
            params![date],
        )?;
    }
    assert!(get_bills_linked_to_recurring(runtime.connection(), user_id)?.is_empty());
    let bills = list_recent_bills_for_recurring_detection(runtime.connection(), user_id, 5)?;
    assert_eq!(bills.len(), 3);
    assert_eq!(bills[0]["counterparty"], "Gym");

    Ok(())
}

fn pattern(hash: &str, name: &str, confidence: f64) -> RecurringPattern {
    RecurringPattern {
        pattern_hash: hash.to_string(),
        name: name.to_string(),
        description: "monthly recurring".to_string(),
        transaction_type: "expense".to_string(),
        amount: 12.5,
        source_account_id: Some(10),
        destination_account_id: String::new(),
        counterparty: "Gym".to_string(),
        frequency: "monthly".to_string(),
        detected_interval_days: 30.0,
        confidence_score: confidence,
        sample_count: 3,
        sample_bill_ids: vec![1, 2, 3],
        first_occurrence: "2026-01-01".to_string(),
        last_occurrence: "2026-03-01".to_string(),
        suggested_next_date: "2026-04-01".to_string(),
    }
}
