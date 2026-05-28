use std::{env, error::Error, net::SocketAddr, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::{connect_info::ConnectInfo, Request},
    http::{header, HeaderValue, Method, StatusCode},
    Router,
};
use base64::{engine::general_purpose, Engine as _};
use bcrypt::{hash, verify};
use bill_analyser_db::{hash_two_factor_recovery_code, run_postgres_migrations};
use bill_analyser_http::{
    build_router, config::DatabaseBackend, HttpAppState, HttpShellConfig, ImportRouteMode,
    AUTH_TOKEN_ROUTE_PATTERNS,
};
use chrono::{Duration as ChronoDuration, Local, Utc};
use ring::hmac;
use rusqlite::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, types::Json};
use tempfile::TempDir;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "auth-token-route-secret";
const TEST_PASSWORD: &str = "correct-password";

#[tokio::test]
async fn auth_token_runtime_lists_and_revokes_user_scoped_sessions() -> Result<(), Box<dyn Error>> {
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/tokens")));
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("DELETE", "/api/tokens/{token_id}")));
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tokens/refresh")));
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/auth/logout")));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let list_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    let tokens = list_body["result"].as_array().expect("token list");
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0]["tokenId"], "3");
    assert_eq!(tokens[0]["tokenType"], 5);
    assert_eq!(tokens[1]["tokenId"], "2");
    assert_eq!(tokens[1]["tokenType"], 8);
    assert_eq!(tokens[2]["tokenId"], "1");
    assert_eq!(tokens[2]["tokenType"], 0);
    assert_eq!(tokens[2]["deviceName"], "Windows 10 (Chrome)");
    assert_eq!(tokens[2]["isCurrent"], true);
    assert_eq!(tokens[2]["isCurrentToken"], true);
    let empty_timestamp_token = token_by_id(tokens, "6");
    assert_eq!(empty_timestamp_token["lastActivityAt"], "");
    assert_eq!(empty_timestamp_token["lastSeen"], 0);
    assert!(!session_is_active(fixture.db_path(), 5)?);

    let revoke_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens/2",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(revoke_response.status(), StatusCode::OK);
    assert_eq!(read_json(revoke_response).await["result"], true);
    assert!(!session_is_active(fixture.db_path(), 2)?);

    let not_found_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens/999",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(not_found_response.status(), StatusCode::NOT_FOUND);
    let not_found_body = read_json(not_found_response).await;
    assert_eq!(not_found_body["error"], "Not Found");
    assert_eq!(not_found_body["message"], "Token not found");

    let revoke_others_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(revoke_others_response.status(), StatusCode::OK);
    let revoke_others_body = read_json(revoke_others_response).await;
    assert_eq!(revoke_others_body["result"], true);
    assert_eq!(revoke_others_body["revokedCount"], 2);
    assert!(session_is_active(fixture.db_path(), 1)?);
    assert!(!session_is_active(fixture.db_path(), 3)?);
    assert!(session_is_active(fixture.db_path(), 4)?);
    assert!(!session_is_active(fixture.db_path(), 6)?);

    Ok(())
}

#[tokio::test]
async fn auth_logout_runtime_invalidates_session_and_is_idempotent() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let logout_response = app.clone().oneshot(logout_request(&token)).await?;
    assert_eq!(logout_response.status(), StatusCode::OK);
    let logout_body = read_json(logout_response).await;
    assert_eq!(logout_body["success"], true);
    assert_eq!(logout_body["result"], true);
    assert_eq!(logout_body["message"], "Logged out successfully");
    assert!(!session_is_active(fixture.db_path(), 1)?);
    assert_auth_log(
        fixture.db_path(),
        "logout",
        true,
        "Mozilla/5.0 (Logout contract)",
    )?;

    let repeated_logout_response = app.clone().oneshot(logout_request(&token)).await?;
    assert_eq!(repeated_logout_response.status(), StatusCode::OK);
    assert_eq!(read_json(repeated_logout_response).await["result"], true);

    let unknown_token_response = app
        .clone()
        .oneshot(logout_request("not-a-known-token"))
        .await?;
    assert_eq!(unknown_token_response.status(), StatusCode::OK);
    assert_eq!(read_json(unknown_token_response).await["result"], true);

    let missing_header_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/auth/logout")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(missing_header_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(missing_header_response).await["message"],
        "Missing authorization header"
    );

    let malformed_header_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/auth/logout")
                .header("authorization", "Basic abc")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(malformed_header_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(malformed_header_response).await["message"],
        "Invalid authorization header"
    );

    Ok(())
}

#[tokio::test]
async fn auth_logout_postgres_cutover_does_not_open_sqlite_fallback() -> Result<(), Box<dyn Error>>
{
    let token = test_access_token(42, TEST_AUTH_SECRET);
    let app = postgres_cutover_runtime_router();

    let response = app.oneshot(logout_request(&token)).await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["result"], true);
    Ok(())
}

#[tokio::test]
async fn auth_postgres_runtime_serves_login_refresh_register_profile_without_sqlite_fallback(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        eprintln!("skipping auth postgres contract without BILL_ANALYSER_TEST_POSTGRES_URL");
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;
    sqlx::query("TRUNCATE business_audit_events, users RESTART IDENTITY CASCADE")
        .execute(&pool)
        .await?;
    let password_hash = hash(TEST_PASSWORD, bcrypt::DEFAULT_COST)?;
    sqlx::query(
        r#"
        INSERT INTO users (
            id, username, email, display_name, password_hash, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(42_i64)
    .bind("pgalice")
    .bind("pgalice@example.test")
    .bind("PG Alice")
    .bind(password_hash)
    .bind(Json(json!({
        "is_active": true,
        "language": "zh_Hans",
        "default_currency": "CNY",
        "first_day_of_week": 1,
        "email_verified": true,
        "import_learning_enabled": true
    })))
    .execute(&pool)
    .await?;
    let edge_password_hash = hash(TEST_PASSWORD, bcrypt::DEFAULT_COST)?;
    let future_lock = (Utc::now().naive_utc() + ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    let expired_lock = (Utc::now().naive_utc() - ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    for (id, username, email, metadata) in [
        (
            78_i64,
            "pgtwofa",
            "pgtwofa@example.test",
            json!({
                "is_active": "true",
                "two_factor_enabled": "yes",
                "two_factor_secret": "JBSWY3DPEHPK3PXP",
                "email_verified": true
            }),
        ),
        (
            79_i64,
            "pglocked",
            "pglocked@example.test",
            json!({
                "is_active": true,
                "locked_until": future_lock,
                "failed_login_attempts": 5,
                "email_verified": true
            }),
        ),
        (
            80_i64,
            "pginactive",
            "pginactive@example.test",
            json!({
                "is_active": false,
                "email_verified": true
            }),
        ),
        (
            81_i64,
            "pgnearlylocked",
            "pgnearlylocked@example.test",
            json!({
                "is_active": true,
                "failed_login_attempts": 4,
                "email_verified": true
            }),
        ),
        (
            82_i64,
            "pgexpiredwrong",
            "pgexpiredwrong@example.test",
            json!({
                "is_active": true,
                "locked_until": expired_lock,
                "failed_login_attempts": 5,
                "email_verified": true
            }),
        ),
        (
            83_i64,
            "pgreset",
            "pgreset@example.test",
            json!({
                "is_active": true,
                "email_verified": true
            }),
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO users (
                id, username, email, display_name, password_hash, metadata
            ) VALUES ($1, $2, $3, '', $4, $5)
            "#,
        )
        .bind(id)
        .bind(username)
        .bind(email)
        .bind(&edge_password_hash)
        .bind(Json(metadata))
        .execute(&pool)
        .await?;
    }
    sqlx::query(
        r#"
        INSERT INTO settings (user_id, key, value)
        VALUES
            ($1, 'application_cloud_settings.showAmountInHomePage', $2),
            ($1, 'application_cloud_settings.dashboardVersion', $3)
        "#,
    )
    .bind(42_i64)
    .bind(Json(Value::String("true".to_string())))
    .bind(Json(json!(2)))
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO accounts (id, user_id, name, account_type, currency)
        VALUES (420, 42, 'PG Cash', '1', 'CNY')
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO categories (id, user_id, name, category_type, path)
        VALUES (421, 42, 'Cash Transfer', '3', 'Cash Transfer')
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO user_external_auths(
            user_id, external_auth_category, external_auth_type,
            external_username, created_at
        ) VALUES
            (42, 'oauth2', 'github', 'pgalice-gh', '2026-01-01T00:00:00Z'),
            (78, 'oauth2', 'github', 'pgtwofa-gh', '2026-01-01T00:00:00Z')
        "#,
    )
    .execute(&pool)
    .await?;

    let app = postgres_auth_runtime_router(&postgres_url)?;
    let login_response = app
        .clone()
        .oneshot(login_request("pgalice", TEST_PASSWORD))
        .await?;
    let login_status = login_response.status();
    let login_body = read_json(login_response).await;
    assert_eq!(login_status, StatusCode::OK, "login body: {login_body}");
    assert_eq!(login_body["success"], true);
    assert_eq!(login_body["result"]["need2FA"], false);
    assert_eq!(login_body["result"]["user"]["username"], "pgalice");
    let login_cloud_settings = login_body["result"]["applicationCloudSettings"]
        .as_array()
        .expect("cloud settings array");
    assert!(login_cloud_settings
        .iter()
        .any(|item| { item["settingKey"] == "dashboardVersion" && item["settingValue"] == "2" }));
    assert!(login_cloud_settings.iter().any(|item| {
        item["settingKey"] == "showAmountInHomePage" && item["settingValue"] == "true"
    }));
    let access_token = login_body["result"]["token"]
        .as_str()
        .expect("access token")
        .to_string();
    let refresh_token = login_body["result"]["refreshToken"]
        .as_str()
        .expect("refresh token")
        .to_string();

    let profile_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/profile")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(profile_response.status(), StatusCode::OK);
    let profile_body = read_json(profile_response).await;
    assert_eq!(profile_body["result"]["username"], "pgalice");
    assert_eq!(profile_body["result"]["nickname"], "PG Alice");

    let profile_update_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({
                "nickname": "PG Alice Updated",
                "email": "pgalice-updated@example.test",
                "language": "en",
                "defaultCurrency": "USD",
                "firstDayOfWeek": 0,
                "defaultAccountId": 420,
                "transactionEditScope": 2,
                "fiscalYearStart": 4,
                "calendarDisplayType": 1,
                "dateDisplayType": 2,
                "longDateFormat": 3,
                "shortDateFormat": 4,
                "longTimeFormat": 5,
                "shortTimeFormat": 6,
                "fiscalYearFormat": 7,
                "currencyDisplayType": 8,
                "numeralSystem": 9,
                "decimalSeparator": 10,
                "digitGroupingSymbol": 11,
                "digitGrouping": 12,
                "coordinateDisplayType": 13,
                "expenseAmountColor": 14,
                "incomeAmountColor": 15,
                "cashAccountId": 420,
                "cashTransferCategoryId": 421,
                "importLearningEnabled": false,
                "investmentPlatformKeywords": ["ETF", "Fund"],
                "investmentProductKeywords": ["Bond"],
                "investmentExcludeKeywords": ["Ignore"]
            }),
        ))
        .await?;
    assert_eq!(profile_update_response.status(), StatusCode::OK);
    let profile_update_body = read_json(profile_update_response).await;
    assert_eq!(
        profile_update_body["result"]["user"]["nickname"],
        "PG Alice Updated"
    );
    assert_eq!(
        profile_update_body["result"]["user"]["email"],
        "pgalice-updated@example.test"
    );
    assert_eq!(
        profile_update_body["result"]["user"]["emailVerified"],
        false
    );
    assert_eq!(
        profile_update_body["result"]["user"]["defaultAccountId"],
        "420"
    );
    assert_eq!(
        profile_update_body["result"]["user"]["transactionEditScope"],
        2
    );
    assert_eq!(
        profile_update_body["result"]["user"]["cashAccountId"],
        "420"
    );
    assert_eq!(
        profile_update_body["result"]["user"]["cashTransferCategoryId"],
        "421"
    );

    let missing_postgres_config_response = postgres_cutover_runtime_router()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({"nickname": "Missing PG"}),
        ))
        .await?;
    assert_eq!(
        missing_postgres_config_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let missing_profile_user_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &test_access_token(777, TEST_AUTH_SECRET),
            json!({"nickname": "Ghost"}),
        ))
        .await?;
    assert_eq!(
        missing_profile_user_response.status(),
        StatusCode::NOT_FOUND
    );

    let empty_profile_update_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({}),
        ))
        .await?;
    assert_eq!(
        empty_profile_update_response.status(),
        StatusCode::BAD_REQUEST
    );

    let same_email_profile_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({"email": "pgalice-updated@example.test"}),
        ))
        .await?;
    assert_eq!(same_email_profile_response.status(), StatusCode::OK);

    let avatar_in_profile_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({"avatar": "inline"}),
        ))
        .await?;
    assert_eq!(avatar_in_profile_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(avatar_in_profile_response).await["message"],
        "Avatar must be updated via /api/profile/avatar"
    );

    let invalid_profile_email_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({"email": "not-an-email"}),
        ))
        .await?;
    assert_eq!(
        invalid_profile_email_response.status(),
        StatusCode::BAD_REQUEST
    );

    let duplicate_profile_email_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({"email": "pginactive@example.test"}),
        ))
        .await?;
    assert_eq!(
        duplicate_profile_email_response.status(),
        StatusCode::CONFLICT
    );

    let invalid_profile_account_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({"defaultAccountId": 9999}),
        ))
        .await?;
    assert_eq!(
        invalid_profile_account_response.status(),
        StatusCode::BAD_REQUEST
    );

    let invalid_profile_cash_account_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({"cashAccountId": 9999}),
        ))
        .await?;
    assert_eq!(
        invalid_profile_cash_account_response.status(),
        StatusCode::BAD_REQUEST
    );

    let invalid_profile_transfer_category_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &access_token,
            json!({"cashTransferCategoryId": 9999}),
        ))
        .await?;
    assert_eq!(
        invalid_profile_transfer_category_response.status(),
        StatusCode::BAD_REQUEST
    );

    let avatar_response = app
        .clone()
        .oneshot(multipart_avatar_request(
            &access_token,
            b"\x89PNG\r\n\x1A\npostgres-avatar",
            "image/png",
        ))
        .await?;
    assert_eq!(avatar_response.status(), StatusCode::OK);
    assert!(read_json(avatar_response).await["result"]["avatar"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));

    let remove_avatar_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/profile/avatar",
            &access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(remove_avatar_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(remove_avatar_response).await["result"]["avatar"],
        ""
    );

    let cloud_settings_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/profile/cloud-settings",
            &access_token,
            Body::empty(),
        ))
        .await?;
    let cloud_settings_status = cloud_settings_response.status();
    let cloud_settings_body = read_json(cloud_settings_response).await;
    assert_eq!(
        cloud_settings_status,
        StatusCode::OK,
        "cloud settings body: {cloud_settings_body}"
    );
    assert!(cloud_settings_body["result"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["settingKey"] == "showAmountInHomePage"));

    let update_cloud_settings_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile/cloud-settings",
            &access_token,
            json!({
                "fullUpdate": true,
                "settings": [
                    {"settingKey": "autoSaveTransactionDraft", "settingValue": "yes"}
                ]
            }),
        ))
        .await?;
    assert_eq!(update_cloud_settings_response.status(), StatusCode::OK);
    let cloud_settings_after_update = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/profile/cloud-settings",
            &access_token,
            Body::empty(),
        ))
        .await?;
    let cloud_settings_after_update_body = read_json(cloud_settings_after_update).await;
    let updated_settings = cloud_settings_after_update_body["result"]
        .as_array()
        .unwrap();
    assert_eq!(updated_settings.len(), 1);
    assert_eq!(
        updated_settings[0]["settingKey"],
        "autoSaveTransactionDraft"
    );
    assert_eq!(updated_settings[0]["settingValue"], "yes");

    let delete_cloud_settings_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/profile/cloud-settings",
            &access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_cloud_settings_response.status(), StatusCode::OK);
    let cloud_settings_after_delete = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/profile/cloud-settings",
            &access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        read_json(cloud_settings_after_delete).await["result"],
        Value::Bool(false)
    );

    let profile_resend_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/profile/email/resend-verification",
            &access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(profile_resend_response.status(), StatusCode::OK);

    let bad_public_resend_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/resend-verification",
            json!({"email": "pgalice-updated@example.test", "password": "wrong-password"}),
        ))
        .await?;
    assert_eq!(
        bad_public_resend_response.status(),
        StatusCode::UNAUTHORIZED
    );

    let public_resend_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/resend-verification",
            json!({"email": "pgalice-updated@example.test", "password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(public_resend_response.status(), StatusCode::OK);
    let Json(verification_metadata): Json<Value> = sqlx::query_scalar(
        r#"
        SELECT metadata
        FROM business_audit_events
        WHERE action = 'verification_email_resend_requested'
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .fetch_one(&pool)
    .await?;
    let verification_token = verification_metadata["metadata"]["verification_token"]
        .as_str()
        .expect("postgres verification token");
    sqlx::query(
        "UPDATE users SET metadata = jsonb_set(metadata, '{email_verified}', 'false'::jsonb, true) WHERE id = 42",
    )
    .execute(&pool)
    .await?;
    let verify_email_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/verify",
            json!({"token": verification_token, "requestNewToken": true}),
        ))
        .await?;
    assert_eq!(verify_email_response.status(), StatusCode::OK);
    let verify_email_body = read_json(verify_email_response).await;
    let verify_email_new_token = verify_email_body["result"]["newToken"]
        .as_str()
        .expect("postgres verify email replacement token")
        .to_string();
    assert_eq!(verify_email_body["result"]["user"]["emailVerified"], true);
    let Json(verified_metadata): Json<Value> =
        sqlx::query_scalar("SELECT metadata FROM users WHERE id = 42")
            .fetch_one(&pool)
            .await?;
    assert_eq!(verified_metadata["email_verified"], true);
    let verify_email_session_logout_response = app
        .clone()
        .oneshot(logout_request(&verify_email_new_token))
        .await?;
    assert_eq!(
        verify_email_session_logout_response.status(),
        StatusCode::OK
    );

    let forgot_unknown_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/forgot",
            json!({"email": "nobody@example.test"}),
        ))
        .await?;
    assert_eq!(forgot_unknown_response.status(), StatusCode::OK);
    let forgot_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/forgot",
            json!({"email": "pgreset@example.test"}),
        ))
        .await?;
    assert_eq!(forgot_response.status(), StatusCode::OK);
    let Json(reset_metadata): Json<Value> = sqlx::query_scalar(
        r#"
        SELECT metadata
        FROM business_audit_events
        WHERE action = 'password_reset_requested'
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .fetch_one(&pool)
    .await?;
    let reset_token = reset_metadata["metadata"]["reset_token"]
        .as_str()
        .expect("postgres reset token");
    let reset_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/reset",
            json!({"email": "pgreset@example.test", "password": "pg-new-password", "token": reset_token}),
        ))
        .await?;
    assert_eq!(reset_response.status(), StatusCode::OK);
    let reset_hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = 83")
        .fetch_one(&pool)
        .await?;
    assert!(bcrypt::verify("pg-new-password", &reset_hash)?);

    let external_auths_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/profile/external-auths",
            &access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(external_auths_response.status(), StatusCode::OK);
    let external_auths_body = read_json(external_auths_response).await;
    let external_auths = external_auths_body["result"]
        .as_array()
        .expect("postgres external auth list");
    assert!(external_auths.iter().any(|auth| {
        auth["externalAuthType"] == "github"
            && auth["externalUsername"] == "pgalice-gh"
            && auth["linked"] == true
    }));
    assert!(external_auths
        .iter()
        .any(|auth| auth["externalAuthType"] == "google" && auth["linked"] == false));
    let unlink_external_auth_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/profile/external-auths/unlink",
            &access_token,
            json!({"externalAuthType": "github", "password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(unlink_external_auth_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(unlink_external_auth_response).await["result"],
        true
    );
    let linked_github_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM user_external_auths WHERE user_id = 42 AND external_auth_type = 'github'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(linked_github_count, 0);
    let other_user_github_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM user_external_auths WHERE user_id = 78 AND external_auth_type = 'github'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(other_user_github_count, 1);

    let refresh_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/tokens/refresh")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "refreshToken": refresh_token }).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_body = read_json(refresh_response).await;
    assert_eq!(refresh_body["success"], true);
    let refreshed_access_token = refresh_body["result"]["token"]
        .as_str()
        .expect("refreshed token")
        .to_string();
    assert!(refreshed_access_token.len() > 16);
    let active_pg_session_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM token_sessions WHERE user_id = 42 AND is_active = TRUE",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(active_pg_session_count, 1);

    let token_list_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            &refreshed_access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(token_list_response.status(), StatusCode::OK);
    let token_list_body = read_json(token_list_response).await;
    let token_list = token_list_body["result"]
        .as_array()
        .expect("postgres token list");
    assert_eq!(token_list.len(), 1);
    assert_eq!(token_list[0]["isCurrent"], true);

    let api_token_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/tokens/api",
            &refreshed_access_token,
            json!({"password": TEST_PASSWORD, "expiresInSeconds": 3600}),
        ))
        .await?;
    assert_eq!(api_token_response.status(), StatusCode::OK);
    let api_token_body = read_json(api_token_response).await;
    assert!(api_token_body["result"]["token"].as_str().is_some());
    assert_eq!(
        api_token_body["result"]["apiBaseUrl"],
        "https://api.example.test/api"
    );
    let token_list_after_api = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            &refreshed_access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(token_list_after_api.status(), StatusCode::OK);
    let token_list_after_api_body = read_json(token_list_after_api).await;
    let token_list_after_api_items = token_list_after_api_body["result"]
        .as_array()
        .expect("postgres token list after api token");
    let api_token_id = token_list_after_api_items
        .iter()
        .find(|token| token["tokenType"] == 8)
        .and_then(|token| token["tokenId"].as_str())
        .expect("api token id")
        .to_string();
    let revoke_pg_token_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            &format!("/api/tokens/{api_token_id}"),
            &refreshed_access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(revoke_pg_token_response.status(), StatusCode::OK);
    assert_eq!(read_json(revoke_pg_token_response).await["result"], true);

    let mcp_token_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/tokens/mcp",
            &refreshed_access_token,
            json!({"password": TEST_PASSWORD, "expiresInSeconds": 3600}),
        ))
        .await?;
    assert_eq!(mcp_token_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(mcp_token_response).await["result"]["mcpUrl"],
        "https://api.example.test/mcp"
    );
    let revoke_other_pg_tokens_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens",
            &refreshed_access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(revoke_other_pg_tokens_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(revoke_other_pg_tokens_response).await["revokedCount"],
        1
    );

    let two_factor_status_disabled_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/2fa/status",
            &refreshed_access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(two_factor_status_disabled_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(two_factor_status_disabled_response).await["result"]["isEnabled"],
        false
    );
    let two_factor_request_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/2fa/enable/request",
            &refreshed_access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(two_factor_request_response.status(), StatusCode::OK);
    let two_factor_request_body = read_json(two_factor_request_response).await;
    let generated_secret = two_factor_request_body["result"]["secret"]
        .as_str()
        .expect("generated secret")
        .to_string();
    assert!(two_factor_request_body["result"]["qrcode"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    let enable_passcode = totp_passcode(&generated_secret, Local::now().timestamp());
    let two_factor_confirm_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/enable/confirm",
            &refreshed_access_token,
            json!({"secret": generated_secret.clone(), "passcode": enable_passcode}),
        ))
        .await?;
    assert_eq!(two_factor_confirm_response.status(), StatusCode::OK);
    let two_factor_confirm_body = read_json(two_factor_confirm_response).await;
    let two_factor_access_token = two_factor_confirm_body["result"]["token"]
        .as_str()
        .expect("2fa session token")
        .to_string();
    let recovery_codes = two_factor_confirm_body["result"]["recoveryCodes"]
        .as_array()
        .expect("recovery codes");
    assert_eq!(recovery_codes.len(), 8);

    let step_up_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &two_factor_access_token,
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(step_up_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(step_up_response).await["result"]["verifiedVia"],
        "password"
    );

    let invalid_step_up_password_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &two_factor_access_token,
            json!({"password": "wrong-password"}),
        ))
        .await?;
    assert_eq!(
        invalid_step_up_password_response.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        read_json(invalid_step_up_password_response).await["message"],
        "Current password is incorrect"
    );

    let invalid_step_up_passcode_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &two_factor_access_token,
            json!({"passcode": "000000"}),
        ))
        .await?;
    assert_eq!(
        invalid_step_up_passcode_response.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        read_json(invalid_step_up_passcode_response).await["message"],
        "The current passcode is incorrect"
    );

    let step_up_passcode_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &two_factor_access_token,
            json!({"passcode": totp_passcode(&generated_secret, Local::now().timestamp())}),
        ))
        .await?;
    assert_eq!(step_up_passcode_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(step_up_passcode_response).await["result"]["verifiedVia"],
        "passcode"
    );

    let regenerate_recovery_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/regenerate",
            &two_factor_access_token,
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(regenerate_recovery_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(regenerate_recovery_response).await["result"]["recoveryCodes"]
            .as_array()
            .unwrap()
            .len(),
        8
    );

    let disable_two_factor_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/disable",
            &two_factor_access_token,
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(disable_two_factor_response.status(), StatusCode::OK);
    assert_eq!(read_json(disable_two_factor_response).await["result"], true);

    let pg_recovery_code = "PG-RECOVERY-0001";
    let pg_recovery_hash =
        hash_two_factor_recovery_code(pg_recovery_code).expect("valid recovery code hash");
    sqlx::query(
        r#"
        INSERT INTO user_two_factor_recovery_codes (user_id, code_hash)
        VALUES (78, $1)
        "#,
    )
    .bind(pg_recovery_hash)
    .execute(&pool)
    .await?;
    let pending_recovery_token = test_action_token(
        78,
        "pgtwofa",
        "pgtwofa@example.test",
        "pending_2fa",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let recovery_login_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &pending_recovery_token,
            json!({"recoveryCode": pg_recovery_code}),
        ))
        .await?;
    assert_eq!(recovery_login_response.status(), StatusCode::OK);
    let recovery_login_body = read_json(recovery_login_response).await;
    assert_eq!(recovery_login_body["result"]["need2FA"], false);
    let used_recovery_codes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM user_two_factor_recovery_codes WHERE user_id = 78 AND used_at IS NOT NULL",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(used_recovery_codes, 1);

    let register_response = app
        .clone()
        .oneshot(register_request(json!({
            "username": "pgnew",
            "email": "pgnew@example.test",
            "password": TEST_PASSWORD,
            "nickname": "PG New",
            "language": "en",
            "defaultCurrency": "USD",
            "categories": [
                {
                    "name": "Food",
                    "type": 3,
                    "icon": "mdi-food",
                    "color": "#ff8800",
                    "subCategories": [
                        {"name": "Lunch"},
                        {"name": "Coffee", "icon": "mdi-coffee", "color": "#663300"},
                        {"name": ""}
                    ]
                },
                {"name": " "}
            ]
        })))
        .await?;
    let register_status = register_response.status();
    let register_body = read_json(register_response).await;
    assert_eq!(
        register_status,
        StatusCode::OK,
        "register body: {register_body}"
    );
    assert_eq!(register_body["success"], true);
    assert_eq!(register_body["result"]["username"], "pgnew");
    assert_eq!(register_body["result"]["presetAccountsSaved"], true);
    assert_eq!(register_body["result"]["presetCategoriesSaved"], true);
    let account_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(register_body["result"]["user_id"].as_i64().unwrap())
        .fetch_one(&pool)
        .await?;
    assert!(account_count > 0);
    let category_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM categories WHERE user_id = $1")
            .bind(register_body["result"]["user_id"].as_i64().unwrap())
            .fetch_one(&pool)
            .await?;
    assert!(category_count > 0);

    let duplicate_username_response = app
        .clone()
        .oneshot(register_request(json!({
            "username": "pgnew",
            "email": "pgnew-other@example.test",
            "password": TEST_PASSWORD
        })))
        .await?;
    assert_eq!(duplicate_username_response.status(), StatusCode::CONFLICT);
    assert_eq!(
        read_json(duplicate_username_response).await["message"],
        "Username already exists"
    );

    let duplicate_email_response = app
        .clone()
        .oneshot(register_request(json!({
            "username": "pgnew2",
            "email": "pgnew@example.test",
            "password": TEST_PASSWORD
        })))
        .await?;
    assert_eq!(duplicate_email_response.status(), StatusCode::CONFLICT);
    assert_eq!(
        read_json(duplicate_email_response).await["message"],
        "Email already exists"
    );

    let unknown_response = app
        .clone()
        .oneshot(login_request("pgmissing", TEST_PASSWORD))
        .await?;
    assert_eq!(unknown_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(unknown_response).await["message"],
        "Invalid username or password"
    );

    let wrong_password_response = app
        .clone()
        .oneshot(login_request("pgnearlylocked", "wrong-password"))
        .await?;
    assert_eq!(wrong_password_response.status(), StatusCode::UNAUTHORIZED);
    let attempts: i64 = sqlx::query_scalar(
        "SELECT (metadata->>'failed_login_attempts')::bigint FROM users WHERE id = 81",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(attempts, 5);
    let locked_until: String = sqlx::query_scalar(
        "SELECT COALESCE(metadata->>'locked_until', '') FROM users WHERE id = 81",
    )
    .fetch_one(&pool)
    .await?;
    assert!(!locked_until.trim().is_empty());

    let expired_wrong_response = app
        .clone()
        .oneshot(login_request("pgexpiredwrong", "wrong-password"))
        .await?;
    assert_eq!(expired_wrong_response.status(), StatusCode::UNAUTHORIZED);
    let expired_attempts: i64 = sqlx::query_scalar(
        "SELECT (metadata->>'failed_login_attempts')::bigint FROM users WHERE id = 82",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(expired_attempts, 1);

    let locked_response = app
        .clone()
        .oneshot(login_request("pglocked", TEST_PASSWORD))
        .await?;
    assert_eq!(locked_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        read_json(locked_response).await["message"],
        "Account is temporarily locked due to multiple failed login attempts"
    );

    let inactive_response = app
        .clone()
        .oneshot(login_request("pginactive", TEST_PASSWORD))
        .await?;
    assert_eq!(inactive_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        read_json(inactive_response).await["message"],
        "Your account has been deactivated"
    );

    let two_factor_response = app
        .clone()
        .oneshot(login_request("pgtwofa", TEST_PASSWORD))
        .await?;
    assert_eq!(two_factor_response.status(), StatusCode::OK);
    let two_factor_body = read_json(two_factor_response).await;
    assert_eq!(two_factor_body["result"]["need2FA"], true);
    assert_eq!(
        jwt_payload(two_factor_body["result"]["token"].as_str().unwrap())["type"],
        "pending_2fa"
    );

    let missing_refresh_user_response = app
        .clone()
        .oneshot(refresh_token_request(&test_refresh_token(
            999,
            "pgmissing",
            TEST_AUTH_SECRET,
            ChronoDuration::hours(1),
        )))
        .await?;
    assert_eq!(
        missing_refresh_user_response.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        read_json(missing_refresh_user_response).await["message"],
        "Invalid refresh token"
    );

    sqlx::query(
        r#"
        INSERT INTO tags (id, user_id, name, color)
        VALUES (430, 42, 'PG Tag', '#3399ff')
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO bills (
            id, user_id, occurred_at, amount_cents, direction, transaction_type,
            account_id, category_id, merchant, payment_method, description, standard_payload
        ) VALUES (
            440, 42, '2026-05-28T08:00:00Z'::timestamptz, -1234, 'expense', 'expense',
            420, 421, 'PG Merchant', 'PG Cash', 'Postgres export bill', $1
        )
        "#,
    )
    .bind(Json(json!({
        "main_category": "Cash Transfer",
        "sub_category": "",
        "batch_id": "pg-user-data"
    })))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO bill_tags (bill_id, tag_id, user_id) VALUES (440, 430, 42)")
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO transaction_templates (id, user_id, template_type, name)
        VALUES (450, 42, 1, 'PG Template')
        "#,
    )
    .execute(&pool)
    .await?;

    let data_statistics_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/data/statistics",
            &access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(data_statistics_response.status(), StatusCode::OK);
    let data_statistics_body = read_json(data_statistics_response).await;
    assert_eq!(data_statistics_body["result"]["billCount"], 1);
    assert_eq!(data_statistics_body["result"]["accountCount"], 1);
    assert_eq!(data_statistics_body["result"]["categoryCount"], 1);
    assert_eq!(data_statistics_body["result"]["tagCount"], 1);
    assert_eq!(data_statistics_body["result"]["templateCount"], 1);

    let data_export_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/data/export.csv?account_ids=420&tag_ids=430&category_ids=421",
            &access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(data_export_response.status(), StatusCode::OK);
    let data_export_text = read_text(data_export_response).await;
    assert!(data_export_text.contains("PG Merchant"));
    assert!(data_export_text.contains("PG Tag"));
    assert!(data_export_text.contains("PG Cash"));

    let clear_step_up_token = test_action_token(
        42,
        "pgalice",
        "pgalice-updated@example.test",
        "step_up",
        TEST_AUTH_SECRET,
        ChronoDuration::minutes(5),
    );
    let clear_transactions_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/data/clear/transactions",
            &access_token,
            json!({"stepUpToken": clear_step_up_token}),
        ))
        .await?;
    assert_eq!(clear_transactions_response.status(), StatusCode::OK);
    let clear_transactions_body = read_json(clear_transactions_response).await;
    assert_eq!(clear_transactions_body["result"], true);
    assert_eq!(clear_transactions_body["deletedCount"], 1);
    let pg_bill_count_after_clear: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bills WHERE user_id = 42")
            .fetch_one(&pool)
            .await?;
    assert_eq!(pg_bill_count_after_clear, 0);

    sqlx::query(
        r#"
        INSERT INTO account_rules (id, user_id, account_id, name)
        VALUES (460, 42, 420, 'PG account rule')
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO category_rules (id, user_id, category_id, name)
        VALUES (461, 42, 421, 'PG category rule')
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO budgets (id, user_id, name, period_type, start_date)
        VALUES (462, 42, 'PG Budget', 'monthly', '2026-05-01')
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO settings (user_id, key, value)
        VALUES (42, 'operation_password', $1)
        ON CONFLICT (user_id, key) DO UPDATE SET value = EXCLUDED.value
        "#,
    )
    .bind(Json(Value::String("pg-operation-secret".to_string())))
    .execute(&pool)
    .await?;

    let clear_all_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/data/clear/all",
            &access_token,
            json!({"password": "pg-operation-secret"}),
        ))
        .await?;
    assert_eq!(clear_all_response.status(), StatusCode::OK);
    let clear_all_body = read_json(clear_all_response).await;
    assert_eq!(clear_all_body["result"], true);
    assert_eq!(clear_all_body["counts"]["accounts"], 1);
    assert_eq!(clear_all_body["counts"]["categories"], 1);
    assert_eq!(clear_all_body["counts"]["tags"], 1);
    assert_eq!(clear_all_body["counts"]["templates"], 1);
    assert_eq!(clear_all_body["counts"]["account_rules"], 1);
    assert_eq!(clear_all_body["counts"]["category_rules"], 1);
    assert_eq!(clear_all_body["counts"]["budgets"], 1);
    let pg_account_count_after_clear_all: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM accounts WHERE user_id = 42")
            .fetch_one(&pool)
            .await?;
    assert_eq!(pg_account_count_after_clear_all, 0);
    let pg_user_data_audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM business_audit_events WHERE user_id = 42 AND entity_type = 'user_data'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(pg_user_data_audit_count, 2);

    Ok(())
}

#[tokio::test]
async fn auth_two_factor_status_runtime_reads_user_flag() -> Result<(), Box<dyn Error>> {
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/2fa/status")));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let disabled_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/2fa/status",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(disabled_response.status(), StatusCode::OK);
    let disabled_body = read_json(disabled_response).await;
    assert_eq!(disabled_body["success"], true);
    assert_eq!(disabled_body["result"]["enable"], false);
    assert_eq!(disabled_body["result"]["isEnabled"], false);

    set_two_factor_enabled(fixture.db_path(), 42, true)?;
    let enabled_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/2fa/status",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(enabled_response.status(), StatusCode::OK);
    let enabled_body = read_json(enabled_response).await;
    assert_eq!(enabled_body["success"], true);
    assert_eq!(enabled_body["result"]["enable"], true);
    assert_eq!(enabled_body["result"]["isEnabled"], true);

    let missing_auth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/2fa/status")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(missing_auth_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(missing_auth_response).await["message"],
        "Missing authorization header"
    );

    let missing_db_path_response = runtime_router_without_sqlite_path()
        .oneshot(trusted_request(
            Method::GET,
            "/api/2fa/status",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        missing_db_path_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        read_json(missing_db_path_response).await["message"],
        "Rust auth token runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    delete_user(fixture.db_path(), 42)?;
    let missing_user_response = app
        .clone()
        .oneshot(trusted_request(
            Method::GET,
            "/api/2fa/status",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_user_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_user_response).await["message"],
        "User not found"
    );

    seed_minimal_user_with_malformed_two_factor_status(fixture.db_path(), 42)?;
    let db_error_response = app
        .clone()
        .oneshot(trusted_request(
            Method::GET,
            "/api/2fa/status",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        db_error_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        read_json(db_error_response).await["message"],
        "Rust auth token runtime DB error"
    );

    Ok(())
}

#[tokio::test]
async fn auth_two_factor_verify_runtime_exchanges_pending_token_for_session(
) -> Result<(), Box<dyn Error>> {
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/2fa/verify")));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    set_two_factor_state(fixture.db_path(), 42, true, Some("JBSWY3DPEHPK3PXP"))?;
    let app = runtime_router(&fixture);
    let pending_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "pending_2fa",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );

    let missing_header_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/2fa/verify")
                .header("content-type", "application/json")
                .body(Body::from(json!({"passcode": "123456"}).to_string()))?,
        )
        .await?;
    assert_eq!(missing_header_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(missing_header_response).await["message"],
        "Missing authorization header"
    );

    let missing_passcode_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/verify",
            &pending_token,
            json!({}),
        ))
        .await?;
    assert_eq!(missing_passcode_response.status(), StatusCode::BAD_REQUEST);
    let missing_passcode_body = read_json(missing_passcode_response).await;
    assert_eq!(missing_passcode_body["message"], "Passcode is required");
    assert_eq!(missing_passcode_body["errorCode"], 203005);

    let invalid_token_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/verify",
            &token,
            json!({"passcode": "123456"}),
        ))
        .await?;
    assert_eq!(invalid_token_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(invalid_token_response).await["message"],
        "Invalid or expired 2FA token"
    );

    let missing_user_pending_token = test_action_token(
        999,
        "missing",
        "missing@example.test",
        "pending_2fa",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let missing_user_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/verify",
            &missing_user_pending_token,
            json!({"passcode": "123456"}),
        ))
        .await?;
    assert_eq!(missing_user_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_user_response).await["error"],
        "User not found"
    );

    set_user_active(fixture.db_path(), 42, false)?;
    let inactive_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/verify",
            &pending_token,
            json!({"passcode": "123456"}),
        ))
        .await?;
    assert_eq!(inactive_response.status(), StatusCode::FORBIDDEN);
    let inactive_body = read_json(inactive_response).await;
    assert_eq!(inactive_body["error"], "Account not active");
    assert_eq!(
        inactive_body["message"],
        "Your account has been deactivated"
    );
    set_user_active(fixture.db_path(), 42, true)?;

    let not_enabled_token = test_action_token(
        77,
        "bob",
        "bob@example.test",
        "pending_2fa",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let not_enabled_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/verify",
            &not_enabled_token,
            json!({"passcode": "123456"}),
        ))
        .await?;
    assert_eq!(not_enabled_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(not_enabled_response).await["message"],
        "Two-factor authentication is not enabled"
    );

    let invalid_passcode_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/verify",
            &pending_token,
            json!({"passcode": "000000"}),
        ))
        .await?;
    assert_eq!(invalid_passcode_response.status(), StatusCode::UNAUTHORIZED);
    let invalid_passcode_body = read_json(invalid_passcode_response).await;
    assert_eq!(invalid_passcode_body["error"], "Invalid passcode");
    assert_eq!(
        invalid_passcode_body["message"],
        "The current passcode is incorrect"
    );

    let passcode = totp_passcode("JBSWY3DPEHPK3PXP", Local::now().timestamp());
    let mut success_request = bearer_json_request(
        Method::POST,
        "/api/2fa/verify",
        &pending_token,
        json!({"passcode": passcode}),
    );
    success_request.headers_mut().insert(
        "x-forwarded-for",
        HeaderValue::from_static("203.0.113.250, 198.51.100.13"),
    );
    let success_response = app.clone().oneshot(success_request).await?;
    assert_eq!(success_response.status(), StatusCode::OK);
    let success_body = read_json(success_response).await;
    assert_eq!(success_body["success"], true);
    let result = success_body["result"].as_object().expect("2fa result");
    let access_token = result["token"].as_str().expect("2fa access token");
    let refresh_token = result["refreshToken"].as_str().expect("2fa refresh token");
    assert_eq!(result["need2FA"], false);
    assert_eq!(result["user"]["id"], 42);
    assert_eq!(result["user"]["username"], "alice");
    assert_session_token_pair(
        fixture.db_path(),
        access_token,
        refresh_token,
        "",
        "203.0.113.250",
    )?;
    assert_auth_log(fixture.db_path(), "login_2fa_success", true, "")?;
    assert_eq!(
        latest_auth_log_ip(fixture.db_path(), "login_2fa_success")?,
        "203.0.113.250"
    );

    Ok(())
}

#[tokio::test]
async fn auth_two_factor_recovery_verify_runtime_consumes_code_and_issues_session(
) -> Result<(), Box<dyn Error>> {
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/2fa/recovery/verify")));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    set_two_factor_state(fixture.db_path(), 42, true, Some("JBSWY3DPEHPK3PXP"))?;
    seed_two_factor_recovery_code(fixture.db_path(), 42, "ABCD-1234")?;
    let app = runtime_router(&fixture);
    let pending_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "pending_2fa",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );

    let missing_header_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/2fa/recovery/verify")
                .header("content-type", "application/json")
                .body(Body::from(json!({"recoveryCode": "ABCD-1234"}).to_string()))?,
        )
        .await?;
    assert_eq!(missing_header_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(missing_header_response).await["message"],
        "Missing authorization header"
    );

    let missing_code_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &pending_token,
            json!({}),
        ))
        .await?;
    assert_eq!(missing_code_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_code_response).await["message"],
        "Recovery code is required"
    );

    let invalid_token_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &token,
            json!({"recoveryCode": "ABCD-1234"}),
        ))
        .await?;
    assert_eq!(invalid_token_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(invalid_token_response).await["message"],
        "Invalid or expired 2FA token"
    );

    let invalid_user_id_token = test_action_token(
        0,
        "invalid",
        "invalid@example.test",
        "pending_2fa",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let invalid_user_id_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &invalid_user_id_token,
            json!({"recoveryCode": "ABCD-1234"}),
        ))
        .await?;
    assert_eq!(invalid_user_id_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(invalid_user_id_response).await["message"],
        "Invalid or expired 2FA token"
    );

    let missing_user_pending_token = test_action_token(
        999,
        "missing",
        "missing@example.test",
        "pending_2fa",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let missing_user_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &missing_user_pending_token,
            json!({"recoveryCode": "ABCD-1234"}),
        ))
        .await?;
    assert_eq!(missing_user_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_user_response).await["error"],
        "User not found"
    );

    set_user_active(fixture.db_path(), 42, false)?;
    let inactive_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &pending_token,
            json!({"recoveryCode": "ABCD-1234"}),
        ))
        .await?;
    assert_eq!(inactive_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        read_json(inactive_response).await["message"],
        "Your account has been deactivated"
    );
    assert!(!recovery_code_is_used(fixture.db_path(), 42, "ABCD-1234")?);
    set_user_active(fixture.db_path(), 42, true)?;

    let not_enabled_token = test_action_token(
        77,
        "bob",
        "bob@example.test",
        "pending_2fa",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let not_enabled_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &not_enabled_token,
            json!({"recoveryCode": "ABCD-1234"}),
        ))
        .await?;
    assert_eq!(not_enabled_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(not_enabled_response).await["message"],
        "Two-factor authentication is not enabled"
    );

    let invalid_code_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &pending_token,
            json!({"recoveryCode": "wrong-code"}),
        ))
        .await?;
    assert_eq!(invalid_code_response.status(), StatusCode::UNAUTHORIZED);
    let invalid_code_body = read_json(invalid_code_response).await;
    assert_eq!(invalid_code_body["error"], "Invalid recovery code");
    assert_eq!(
        invalid_code_body["message"],
        "Recovery code is invalid or already used"
    );
    assert!(!recovery_code_is_used(fixture.db_path(), 42, "ABCD-1234")?);

    let mut success_request = bearer_json_request(
        Method::POST,
        "/api/2fa/recovery/verify",
        &pending_token,
        json!({"recoveryCode": "abcd-1234"}),
    );
    success_request.headers_mut().insert(
        "x-forwarded-for",
        HeaderValue::from_static("203.0.113.251, 198.51.100.13"),
    );
    let success_response = app.clone().oneshot(success_request).await?;
    assert_eq!(success_response.status(), StatusCode::OK);
    let success_body = read_json(success_response).await;
    assert_eq!(success_body["success"], true);
    let result = success_body["result"]
        .as_object()
        .expect("2fa recovery result");
    let access_token = result["token"].as_str().expect("2fa recovery access token");
    let refresh_token = result["refreshToken"]
        .as_str()
        .expect("2fa recovery refresh token");
    assert_eq!(result["need2FA"], false);
    assert_eq!(result["user"]["id"], 42);
    assert_eq!(result["user"]["username"], "alice");
    assert_session_token_pair(
        fixture.db_path(),
        access_token,
        refresh_token,
        "",
        "203.0.113.251",
    )?;
    assert!(recovery_code_is_used(fixture.db_path(), 42, "ABCD-1234")?);
    assert_auth_log(fixture.db_path(), "login_2fa_recovery_success", true, "")?;
    assert_eq!(
        latest_auth_log_ip(fixture.db_path(), "login_2fa_recovery_success")?,
        "203.0.113.251"
    );
    let audit_details = latest_audit_log_details(fixture.db_path(), "2fa_recovery_code_used", 42)?;
    assert_eq!(audit_details["verification"], "recovery_code");

    let reused_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &pending_token,
            json!({"recoveryCode": "ABCD-1234"}),
        ))
        .await?;
    assert_eq!(reused_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(reused_response).await["message"],
        "Recovery code is invalid or already used"
    );

    Ok(())
}

#[tokio::test]
async fn auth_two_factor_recovery_verify_runtime_rolls_back_on_db_errors(
) -> Result<(), Box<dyn Error>> {
    let token = test_access_token(42, TEST_AUTH_SECRET);
    let pending_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "pending_2fa",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );

    let no_db_response = runtime_router_without_sqlite_path()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &pending_token,
            json!({"recoveryCode": "ABCD-1234"}),
        ))
        .await?;
    assert_eq!(no_db_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(no_db_response).await["message"],
        "Rust auth token runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    let rollback_fixture = RuntimeFixture::new()?;
    seed_auth_db(rollback_fixture.db_path(), &token)?;
    set_two_factor_state(
        rollback_fixture.db_path(),
        42,
        true,
        Some("JBSWY3DPEHPK3PXP"),
    )?;
    seed_two_factor_recovery_code(rollback_fixture.db_path(), 42, "ROLL-BACK")?;
    let before_session_count = session_count(rollback_fixture.db_path())?;
    Connection::open(rollback_fixture.db_path())?.execute_batch(
        "CREATE TRIGGER fail_recovery_auth_log_insert
         BEFORE INSERT ON auth_logs
         WHEN NEW.event_type = 'login_2fa_recovery_success'
         BEGIN
             SELECT RAISE(FAIL, 'forced recovery auth log failure');
         END;",
    )?;
    let rollback_response = runtime_router(&rollback_fixture)
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/verify",
            &pending_token,
            json!({"recoveryCode": "ROLL-BACK"}),
        ))
        .await?;
    assert_eq!(
        rollback_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        session_count(rollback_fixture.db_path())?,
        before_session_count
    );
    assert!(!recovery_code_is_used(
        rollback_fixture.db_path(),
        42,
        "ROLL-BACK"
    )?);

    Ok(())
}

#[tokio::test]
async fn auth_two_factor_write_runtime_manages_setup_recovery_and_disable(
) -> Result<(), Box<dyn Error>> {
    for route in [
        ("POST", "/api/2fa/enable/request"),
        ("POST", "/api/2fa/enable/confirm"),
        ("POST", "/api/2fa/disable"),
        ("POST", "/api/2fa/recovery/regenerate"),
    ] {
        assert!(AUTH_TOKEN_ROUTE_PATTERNS.iter().any(|item| item == &route));
    }

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let request_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/2fa/enable/request",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(request_response.status(), StatusCode::OK);
    let request_body = read_json(request_response).await;
    assert_eq!(request_body["success"], true);
    let secret = request_body["result"]["secret"]
        .as_str()
        .expect("2fa secret");
    assert_eq!(secret.len(), 32);
    assert!(secret
        .bytes()
        .all(|byte| matches!(byte, b'A'..=b'Z' | b'2'..=b'7')));
    let qrcode = request_body["result"]["qrcode"]
        .as_str()
        .expect("2fa qrcode");
    assert!(qrcode.starts_with("data:image/png;base64,"));
    let qrcode_bytes = general_purpose::STANDARD.decode(
        qrcode
            .strip_prefix("data:image/png;base64,")
            .expect("png data url"),
    )?;
    assert!(qrcode_bytes.starts_with(b"\x89PNG\r\n\x1A\n"));

    let missing_confirm_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/enable/confirm",
            &token,
            json!({}),
        ))
        .await?;
    assert_eq!(missing_confirm_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_confirm_response).await["message"],
        "Secret and passcode are required"
    );

    let invalid_confirm_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/enable/confirm",
            &token,
            json!({"secret": secret, "passcode": "000000"}),
        ))
        .await?;
    assert_eq!(invalid_confirm_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_confirm_response).await["error"],
        "Invalid passcode"
    );

    let passcode = totp_passcode(secret, Local::now().timestamp());
    let mut confirm_request = bearer_json_request(
        Method::POST,
        "/api/2fa/enable/confirm",
        &token,
        json!({"secret": secret, "passcode": passcode}),
    );
    confirm_request.headers_mut().insert(
        "user-agent",
        HeaderValue::from_static("Mozilla/5.0 (2FA write contract)"),
    );
    let confirm_response = app.clone().oneshot(confirm_request).await?;
    assert_eq!(confirm_response.status(), StatusCode::OK);
    let confirm_body = read_json(confirm_response).await;
    assert_eq!(confirm_body["success"], true);
    let confirm_result = confirm_body["result"].as_object().expect("2fa result");
    let access_token = confirm_result["token"].as_str().expect("2fa access token");
    let refresh_token = confirm_result["refreshToken"]
        .as_str()
        .expect("2fa refresh token");
    let recovery_codes = confirm_result["recoveryCodes"]
        .as_array()
        .expect("recovery codes");
    assert_eq!(recovery_codes.len(), 8);
    for code in recovery_codes {
        let code = code.as_str().expect("recovery code");
        assert!(code.len() == 9 && code.as_bytes()[4] == b'-');
        assert!(code
            .bytes()
            .all(|byte| matches!(byte, b'A'..=b'F' | b'0'..=b'9' | b'-')));
    }
    assert_session_token_pair(
        fixture.db_path(),
        access_token,
        refresh_token,
        "Mozilla/5.0 (2FA write contract)",
        "198.51.100.13",
    )?;
    assert_eq!(
        two_factor_state(fixture.db_path(), 42)?,
        (true, secret.to_string())
    );
    assert_eq!(active_recovery_code_count(fixture.db_path(), 42)?, 8);
    let enable_audit = latest_audit_log_details(fixture.db_path(), "2fa_enabled", 42)?;
    assert_eq!(enable_audit["recovery_code_count"], 8);

    let second_request_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/2fa/enable/request",
            access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(second_request_response.status(), StatusCode::OK);
    let second_request_body = read_json(second_request_response).await;
    let second_secret = second_request_body["result"]["secret"]
        .as_str()
        .expect("replacement secret");
    let second_passcode = totp_passcode(second_secret, Local::now().timestamp());
    let already_enabled_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/enable/confirm",
            access_token,
            json!({"secret": second_secret, "passcode": second_passcode}),
        ))
        .await?;
    assert_eq!(already_enabled_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(already_enabled_response).await["message"],
        "Two-factor authentication is already enabled"
    );
    assert_eq!(
        two_factor_state(fixture.db_path(), 42)?,
        (true, secret.to_string())
    );
    assert_eq!(active_recovery_code_count(fixture.db_path(), 42)?, 8);

    let missing_regenerate_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/regenerate",
            access_token,
            json!({}),
        ))
        .await?;
    assert_eq!(
        missing_regenerate_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_regenerate_response).await["message"],
        "Current password or stepUpToken is required"
    );

    let wrong_password_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/regenerate",
            access_token,
            json!({"password": "Wrong123!"}),
        ))
        .await?;
    assert_eq!(wrong_password_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(wrong_password_response).await["message"],
        "Current password is incorrect"
    );

    let step_up_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "step_up",
        TEST_AUTH_SECRET,
        ChronoDuration::minutes(5),
    );
    let regenerate_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/regenerate",
            access_token,
            json!({"stepUpToken": step_up_token}),
        ))
        .await?;
    assert_eq!(regenerate_response.status(), StatusCode::OK);
    let regenerate_body = read_json(regenerate_response).await;
    assert_eq!(regenerate_body["success"], true);
    assert_eq!(
        regenerate_body["result"]["recoveryCodes"]
            .as_array()
            .expect("regenerated codes")
            .len(),
        8
    );
    assert_eq!(active_recovery_code_count(fixture.db_path(), 42)?, 8);
    let regenerate_audit =
        latest_audit_log_details(fixture.db_path(), "2fa_recovery_regenerated", 42)?;
    assert_eq!(regenerate_audit["auth_mode"], "step_up");

    let disable_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/disable",
            access_token,
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(disable_response.status(), StatusCode::OK);
    let disable_body = read_json(disable_response).await;
    assert_eq!(disable_body["success"], true);
    assert_eq!(disable_body["result"], true);
    assert_eq!(
        two_factor_state(fixture.db_path(), 42)?,
        (false, String::new())
    );
    assert_eq!(active_recovery_code_count(fixture.db_path(), 42)?, 0);
    let disable_audit = latest_audit_log_details(fixture.db_path(), "2fa_disabled", 42)?;
    assert_eq!(disable_audit["auth_mode"], "password");

    Ok(())
}

#[tokio::test]
async fn auth_two_factor_write_runtime_covers_missing_runtime_and_user_edges(
) -> Result<(), Box<dyn Error>> {
    let secret = "JBSWY3DPEHPK3PXP";
    let passcode = totp_passcode(secret, Local::now().timestamp());

    let auth_app = runtime_router_without_sqlite_path();
    let unauthenticated_enable_request = auth_app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/2fa/enable/request")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        unauthenticated_enable_request.status(),
        StatusCode::UNAUTHORIZED
    );
    let unauthenticated_enable_confirm = auth_app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/2fa/enable/confirm")
                .body(Body::from(
                    json!({"secret": secret, "passcode": passcode.clone()}).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(
        unauthenticated_enable_confirm.status(),
        StatusCode::UNAUTHORIZED
    );

    let no_db_app = runtime_router_without_sqlite_path();
    let no_db_enable_request = no_db_app
        .clone()
        .oneshot(trusted_request(
            Method::POST,
            "/api/2fa/enable/request",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        no_db_enable_request.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let no_db_confirm = no_db_app
        .clone()
        .oneshot(trusted_user_json_request(
            42,
            Method::POST,
            "/api/2fa/enable/confirm",
            json!({"secret": secret, "passcode": passcode}),
        ))
        .await?;
    assert_eq!(no_db_confirm.status(), StatusCode::SERVICE_UNAVAILABLE);

    let no_db_disable = no_db_app
        .clone()
        .oneshot(trusted_user_json_request(
            42,
            Method::POST,
            "/api/2fa/disable",
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(no_db_disable.status(), StatusCode::SERVICE_UNAVAILABLE);

    let no_db_regenerate = no_db_app
        .clone()
        .oneshot(trusted_user_json_request(
            42,
            Method::POST,
            "/api/2fa/recovery/regenerate",
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(no_db_regenerate.status(), StatusCode::SERVICE_UNAVAILABLE);

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let missing_user_enable_request = app
        .clone()
        .oneshot(trusted_user_json_request(
            999,
            Method::POST,
            "/api/2fa/enable/request",
            json!({}),
        ))
        .await?;
    assert_eq!(missing_user_enable_request.status(), StatusCode::NOT_FOUND);

    let missing_user_confirm = app
        .clone()
        .oneshot(trusted_user_json_request(
            999,
            Method::POST,
            "/api/2fa/enable/confirm",
            json!({"secret": secret, "passcode": totp_passcode(secret, Local::now().timestamp())}),
        ))
        .await?;
    assert_eq!(missing_user_confirm.status(), StatusCode::NOT_FOUND);

    let missing_user_disable = app
        .clone()
        .oneshot(trusted_user_json_request(
            999,
            Method::POST,
            "/api/2fa/disable",
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(missing_user_disable.status(), StatusCode::NOT_FOUND);

    let missing_user_regenerate = app
        .clone()
        .oneshot(trusted_user_json_request(
            999,
            Method::POST,
            "/api/2fa/recovery/regenerate",
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(missing_user_regenerate.status(), StatusCode::NOT_FOUND);

    let regenerate_without_2fa = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/2fa/recovery/regenerate",
            &token,
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(regenerate_without_2fa.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(regenerate_without_2fa).await["message"],
        "Two-factor authentication is not enabled"
    );

    Ok(())
}

#[tokio::test]
async fn auth_step_up_runtime_issues_short_lived_tokens_for_password_or_totp(
) -> Result<(), Box<dyn Error>> {
    let route = ("POST", "/api/security/step-up/verify");
    assert!(AUTH_TOKEN_ROUTE_PATTERNS.iter().any(|item| item == &route));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let missing_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &token,
            json!({}),
        ))
        .await?;
    assert_eq!(missing_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_response).await["message"],
        "password or passcode is required"
    );

    let no_auth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/security/step-up/verify")
                .body(Body::from(json!({"password": TEST_PASSWORD}).to_string()))?,
        )
        .await?;
    assert_eq!(no_auth_response.status(), StatusCode::UNAUTHORIZED);

    let missing_user_response = app
        .clone()
        .oneshot(trusted_user_json_request(
            999,
            Method::POST,
            "/api/security/step-up/verify",
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(missing_user_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_user_response).await["error"],
        "User not found"
    );

    let invalid_path = fixture
        .db_path()
        .parent()
        .expect("fixture parent")
        .join("missing-parent")
        .join("auth.db");
    let invalid_path_app = runtime_router_with_db_path(&invalid_path, true);
    let invalid_path_response = invalid_path_app
        .oneshot(trusted_user_json_request(
            42,
            Method::POST,
            "/api/security/step-up/verify",
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(
        invalid_path_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let wrong_password_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &token,
            json!({"password": "Wrong123!"}),
        ))
        .await?;
    assert_eq!(wrong_password_response.status(), StatusCode::UNAUTHORIZED);
    let wrong_password_body = read_json(wrong_password_response).await;
    assert_eq!(wrong_password_body["error"], "Invalid credentials");
    assert_eq!(
        wrong_password_body["message"],
        "Current password is incorrect"
    );

    let no_2fa_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &token,
            json!({"passcode": "123456"}),
        ))
        .await?;
    assert_eq!(no_2fa_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(no_2fa_response).await["message"],
        "Two-factor authentication is not enabled"
    );

    set_two_factor_state(fixture.db_path(), 42, true, Some("JBSWY3DPEHPK3PXP"))?;
    let invalid_passcode_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &token,
            json!({"passcode": "000000"}),
        ))
        .await?;
    assert_eq!(invalid_passcode_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(invalid_passcode_response).await["message"],
        "The current passcode is incorrect"
    );

    let mut password_request = bearer_json_request(
        Method::POST,
        "/api/security/step-up/verify",
        &token,
        json!({"password": TEST_PASSWORD}),
    );
    password_request.headers_mut().insert(
        "user-agent",
        HeaderValue::from_static("Mozilla/5.0 (Step-up contract)"),
    );
    let password_response = app.clone().oneshot(password_request).await?;
    assert_eq!(password_response.status(), StatusCode::OK);
    let password_body = read_json(password_response).await;
    assert_eq!(password_body["success"], true);
    assert_eq!(password_body["result"]["verifiedVia"], "password");
    let password_step_up_token = password_body["result"]["stepUpToken"]
        .as_str()
        .expect("password step-up token");
    let password_payload = jwt_payload(password_step_up_token);
    assert_eq!(password_payload["type"], "step_up");
    assert_eq!(password_payload["user_id"], 42);
    assert_eq!(password_payload["email"], "alice@example.test");
    assert_eq!(jwt_lifetime_seconds(&password_payload), 3600);
    assert_auth_log(
        fixture.db_path(),
        "step_up_verified",
        true,
        "Mozilla/5.0 (Step-up contract)",
    )?;
    assert_eq!(
        latest_auth_log_metadata(fixture.db_path(), "step_up_verified")?["verified_via"],
        "password"
    );

    let operation_password = std::env::var("BILL_ANALYSER_OPERATION_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "operation-secret".to_string());
    let operation_password_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &token,
            json!({"password": operation_password}),
        ))
        .await?;
    assert_eq!(operation_password_response.status(), StatusCode::OK);
    let operation_password_body = read_json(operation_password_response).await;
    assert_eq!(operation_password_body["result"]["verifiedVia"], "password");
    let operation_password_payload = jwt_payload(
        operation_password_body["result"]["stepUpToken"]
            .as_str()
            .expect("operation password step-up token"),
    );
    assert_eq!(operation_password_payload["type"], "step_up");
    assert_eq!(jwt_lifetime_seconds(&operation_password_payload), 3600);

    {
        let connection = Connection::open(fixture.db_path())?;
        connection.execute(
            "DELETE FROM app_settings WHERE key = 'operation_password'",
            [],
        )?;
    }
    let unset_operation_password_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &token,
            json!({"password": "not-the-current-password"}),
        ))
        .await?;
    assert_eq!(
        unset_operation_password_response.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_auth_log(fixture.db_path(), "step_up_auth_failed", false, "")?;

    let no_jwt_secret_app = runtime_router_with_db_path(fixture.db_path(), false);
    let no_jwt_secret_response = no_jwt_secret_app
        .oneshot(trusted_user_json_request(
            42,
            Method::POST,
            "/api/security/step-up/verify",
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(
        no_jwt_secret_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let log_error_fixture = RuntimeFixture::new()?;
    let log_error_token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(log_error_fixture.db_path(), &log_error_token)?;
    Connection::open(log_error_fixture.db_path())?.execute_batch(
        "CREATE TRIGGER fail_step_up_auth_log_insert
         BEFORE INSERT ON auth_logs
         BEGIN
             SELECT RAISE(FAIL, 'forced step-up auth log failure');
         END;",
    )?;
    let log_error_app = runtime_router(&log_error_fixture);
    let log_error_response = log_error_app
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &log_error_token,
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(
        log_error_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let passcode = totp_passcode("JBSWY3DPEHPK3PXP", Local::now().timestamp());
    let passcode_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &token,
            json!({"passcode": passcode}),
        ))
        .await?;
    assert_eq!(passcode_response.status(), StatusCode::OK);
    let passcode_body = read_json(passcode_response).await;
    assert_eq!(passcode_body["result"]["verifiedVia"], "passcode");
    let passcode_step_up_token = passcode_body["result"]["stepUpToken"]
        .as_str()
        .expect("passcode step-up token");
    let passcode_payload = jwt_payload(passcode_step_up_token);
    assert_eq!(passcode_payload["type"], "step_up");
    assert_eq!(jwt_lifetime_seconds(&passcode_payload), 3600);
    assert_eq!(
        latest_auth_log_metadata(fixture.db_path(), "step_up_verified")?["verified_via"],
        "passcode"
    );

    Ok(())
}

#[tokio::test]
async fn auth_sensitive_operation_failures_are_logged_and_rate_limited(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    for _ in 0..5 {
        let response = app
            .clone()
            .oneshot(bearer_json_request(
                Method::POST,
                "/api/security/step-up/verify",
                &token,
                json!({"password": "wrong-password"}),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert_auth_log(fixture.db_path(), "step_up_auth_failed", false, "")?;
    let rate_limited_step_up = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/security/step-up/verify",
            &token,
            json!({"password": "wrong-password"}),
        ))
        .await?;
    assert_eq!(rate_limited_step_up.status(), StatusCode::TOO_MANY_REQUESTS);

    for _ in 0..5 {
        let response = app
            .clone()
            .oneshot(bearer_json_request(
                Method::POST,
                "/api/data/clear/transactions",
                &token,
                json!({"password": "wrong-password"}),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert_auth_log(fixture.db_path(), "user_data_clear_auth_failed", false, "")?;
    let rate_limited_clear = app
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/data/clear/transactions",
            &token,
            json!({"password": "wrong-password"}),
        ))
        .await?;
    assert_eq!(rate_limited_clear.status(), StatusCode::TOO_MANY_REQUESTS);

    Ok(())
}

#[tokio::test]
async fn auth_login_runtime_issues_session_tokens_and_preserves_edges() -> Result<(), Box<dyn Error>>
{
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/auth/login")));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    seed_login_edge_users(fixture.db_path())?;
    let app = runtime_router(&fixture);

    let login_response = app
        .clone()
        .oneshot(login_request_with_origin_and_forwarded_ip(
            "alice",
            TEST_PASSWORD,
            "http://localhost:8081",
            "203.0.113.44, 198.51.100.2",
        ))
        .await?;
    assert_eq!(login_response.status(), StatusCode::OK);
    assert_eq!(
        login_response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:8081")
    );
    assert_eq!(
        login_response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .and_then(|value| value.to_str().ok()),
        Some("true")
    );
    let login_body = read_json(login_response).await;
    assert_eq!(login_body["success"], true);
    let login_result = login_body["result"].as_object().expect("login result");
    let access_token = login_result["token"].as_str().expect("access token");
    let refresh_token = login_result["refreshToken"]
        .as_str()
        .expect("refresh token");
    assert_eq!(login_result["need2FA"], false);
    assert_eq!(login_result["user"]["id"], 42);
    assert_eq!(login_result["user"]["username"], "alice");
    assert_eq!(login_result["user"]["nickname"], "Alice A.");
    assert_eq!(
        login_result["applicationCloudSettings"][0]["settingKey"],
        "showAmountInHomePage"
    );
    assert_eq!(jwt_payload(access_token)["type"], "access");
    assert_eq!(jwt_payload(refresh_token)["type"], "refresh");
    assert_session_token_pair(
        fixture.db_path(),
        access_token,
        refresh_token,
        "Mozilla/5.0 (Login contract)",
        "203.0.113.44",
    )?;
    assert_auth_log(
        fixture.db_path(),
        "login_success",
        true,
        "Mozilla/5.0 (Login contract)",
    )?;
    assert_login_state(fixture.db_path(), 42, 0, "203.0.113.44")?;

    let email_login_response = app
        .clone()
        .oneshot(login_request("alice@example.test", TEST_PASSWORD))
        .await?;
    assert_eq!(email_login_response.status(), StatusCode::OK);
    let email_login_body = read_json(email_login_response).await;
    assert_eq!(email_login_body["result"]["need2FA"], false);
    let email_access_token = email_login_body["result"]["token"]
        .as_str()
        .expect("email access token");
    let email_refresh_token = email_login_body["result"]["refreshToken"]
        .as_str()
        .expect("email refresh token");
    assert_session_token_pair(
        fixture.db_path(),
        email_access_token,
        email_refresh_token,
        "Mozilla/5.0 (Login contract)",
        "198.51.100.30",
    )?;

    let two_factor_response = app
        .clone()
        .oneshot(login_request("twofa", TEST_PASSWORD))
        .await?;
    assert_eq!(two_factor_response.status(), StatusCode::OK);
    let two_factor_body = read_json(two_factor_response).await;
    assert_eq!(two_factor_body["success"], true);
    assert_eq!(two_factor_body["result"]["need2FA"], true);
    let pending_token = two_factor_body["result"]["token"]
        .as_str()
        .expect("pending token");
    assert_eq!(jwt_payload(pending_token)["type"], "pending_2fa");
    assert_auth_log(
        fixture.db_path(),
        "login_2fa_pending",
        true,
        "Mozilla/5.0 (Login contract)",
    )?;

    let missing_credentials_response = app
        .clone()
        .oneshot(login_request("", TEST_PASSWORD))
        .await?;
    assert_eq!(
        missing_credentials_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_credentials_response).await["message"],
        "Username and password are required"
    );

    let unknown_response = app
        .clone()
        .oneshot(login_request("missing-user", TEST_PASSWORD))
        .await?;
    assert_eq!(unknown_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(unknown_response).await["message"],
        "Invalid username or password"
    );
    assert_auth_log(
        fixture.db_path(),
        "login_failed",
        false,
        "Mozilla/5.0 (Login contract)",
    )?;

    let wrong_password_response = app
        .clone()
        .oneshot(login_request("bob", "wrong-password"))
        .await?;
    assert_eq!(wrong_password_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(wrong_password_response).await["message"],
        "Invalid username or password"
    );
    assert_login_failure_count(fixture.db_path(), 77, 1)?;

    let lockout_transition_response = app
        .clone()
        .oneshot(login_request("nearlylocked", "wrong-password"))
        .await?;
    assert_eq!(
        lockout_transition_response.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_login_locked(fixture.db_path(), 81, 5)?;

    let expired_lock_wrong_password_response = app
        .clone()
        .oneshot(login_request("expiredwrong", "wrong-password"))
        .await?;
    assert_eq!(
        expired_lock_wrong_password_response.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_login_failure_count(fixture.db_path(), 83, 1)?;
    assert_login_unlocked(fixture.db_path(), 83)?;

    let locked_response = app
        .clone()
        .oneshot(login_request("locked", TEST_PASSWORD))
        .await?;
    assert_eq!(locked_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        read_json(locked_response).await["message"],
        "Account is temporarily locked due to multiple failed login attempts"
    );

    let expired_lock_response = app
        .clone()
        .oneshot(login_request("expiredlock", TEST_PASSWORD))
        .await?;
    assert_eq!(expired_lock_response.status(), StatusCode::OK);
    assert_login_state(fixture.db_path(), 82, 0, "198.51.100.30")?;
    assert_login_unlocked(fixture.db_path(), 82)?;

    let inactive_response = app
        .clone()
        .oneshot(login_request("inactive", TEST_PASSWORD))
        .await?;
    assert_eq!(inactive_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        read_json(inactive_response).await["message"],
        "Your account has been deactivated"
    );

    Ok(())
}

#[tokio::test]
async fn auth_register_runtime_creates_user_defaults_and_preserves_edges(
) -> Result<(), Box<dyn Error>> {
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/auth/register")));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let missing_response = app
        .clone()
        .oneshot(register_request(json!({ "username": "missing" })))
        .await?;
    assert_eq!(missing_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_response).await["message"],
        "Username, email and password are required"
    );

    let weak_response = app
        .clone()
        .oneshot(register_request(json!({
            "username": "weak",
            "email": "weak@example.test",
            "password": "short"
        })))
        .await?;
    assert_eq!(weak_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(weak_response).await["message"],
        "Password must be at least 8 characters long"
    );

    let duplicate_username_response = app
        .clone()
        .oneshot(register_request(json!({
            "username": "alice",
            "email": "new-alice@example.test",
            "password": TEST_PASSWORD
        })))
        .await?;
    assert_eq!(duplicate_username_response.status(), StatusCode::CONFLICT);
    assert_eq!(
        read_json(duplicate_username_response).await["message"],
        "Username already exists"
    );
    assert_auth_log(
        fixture.db_path(),
        "register_failed",
        false,
        "Mozilla/5.0 (Register contract)",
    )?;

    let duplicate_email_response = app
        .clone()
        .oneshot(register_request(json!({
            "username": "alice2",
            "email": "alice@example.test",
            "password": TEST_PASSWORD
        })))
        .await?;
    assert_eq!(duplicate_email_response.status(), StatusCode::CONFLICT);
    assert_eq!(
        read_json(duplicate_email_response).await["message"],
        "Email already exists"
    );

    let success_response = app
        .clone()
        .oneshot(register_request(json!({
            "username": "newuser",
            "email": "newuser@example.test",
            "password": TEST_PASSWORD,
            "nickname": "",
            "language": "zh_Hans",
            "defaultCurrency": "CNY",
            "firstDayOfWeek": 2,
            "categories": [
                {
                    "name": "自定义",
                    "type": 3,
                    "icon": "custom",
                    "color": "123456",
                    "subCategories": [{"name": "子类"}]
                }
            ]
        })))
        .await?;
    assert_eq!(success_response.status(), StatusCode::OK);
    assert_eq!(
        success_response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:8081")
    );
    let success_body = read_json(success_response).await;
    assert_eq!(success_body["success"], true);
    assert_eq!(success_body["result"]["username"], "newuser");
    assert_eq!(success_body["result"]["email"], "newuser@example.test");
    assert_eq!(success_body["result"]["needVerifyEmail"], false);
    assert_eq!(success_body["result"]["presetCategoriesSaved"], true);
    assert_eq!(success_body["result"]["presetAccountsSaved"], true);

    assert_registered_user_defaults(fixture.db_path(), "newuser", TEST_PASSWORD)?;
    assert_auth_log(
        fixture.db_path(),
        "register_success",
        true,
        "Mozilla/5.0 (Register contract)",
    )?;
    Ok(())
}

#[tokio::test]
async fn auth_profile_cloud_runtime_preserves_flask_contracts() -> Result<(), Box<dyn Error>> {
    for route in [
        ("GET", "/api/profile"),
        ("PUT", "/api/profile"),
        ("POST", "/api/profile/avatar"),
        ("DELETE", "/api/profile/avatar"),
        ("POST", "/api/profile/email/resend-verification"),
        ("GET", "/api/profile/cloud-settings"),
        ("PUT", "/api/profile/cloud-settings"),
        ("DELETE", "/api/profile/cloud-settings"),
        ("GET", "/api/profile/external-auths"),
        ("POST", "/api/profile/external-auths/unlink"),
        ("POST", "/api/auth/oauth2/authorize"),
        ("GET", "/api/system/version"),
    ] {
        assert!(AUTH_TOKEN_ROUTE_PATTERNS.iter().any(|item| item == &route));
    }

    let disabled_oauth_response = runtime_router_without_sqlite_path()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/auth/oauth2/authorize")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(disabled_oauth_response.status(), StatusCode::FORBIDDEN);
    let disabled_oauth_body = read_json(disabled_oauth_response).await;
    assert_eq!(disabled_oauth_body["success"], false);
    assert_eq!(disabled_oauth_body["error"], "OAuth2 disabled");
    assert_eq!(
        disabled_oauth_body["message"],
        "OAuth2 login is currently disabled"
    );

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let unauthenticated_profile_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/profile")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        unauthenticated_profile_response.status(),
        StatusCode::UNAUTHORIZED
    );
    let unauthenticated_profile_update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/profile")
                .body(Body::from(r#"{"nickname":"No Auth"}"#))?,
        )
        .await?;
    assert_eq!(
        unauthenticated_profile_update_response.status(),
        StatusCode::UNAUTHORIZED
    );
    let unauthenticated_avatar_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/profile/avatar")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        unauthenticated_avatar_response.status(),
        StatusCode::UNAUTHORIZED
    );

    let oauth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/auth/oauth2/authorize")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(oauth_response.status(), StatusCode::NOT_IMPLEMENTED);
    let oauth_body = read_json(oauth_response).await;
    assert_eq!(oauth_body["success"], false);
    assert_eq!(oauth_body["error"], "Not Implemented");
    assert_eq!(
        oauth_body["message"],
        "OAuth2 callback authorization is not implemented in this workspace build"
    );

    let version_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/system/version")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(version_response.status(), StatusCode::OK);
    let version_body = read_json(version_response).await;
    assert_eq!(version_body["result"]["version"], "1.0.0");
    assert_eq!(version_body["result"]["commitHash"], "");
    assert_eq!(version_body["result"]["buildTime"], "");

    let preflight_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/profile/cloud-settings")
                .header("origin", "http://localhost:8081")
                .header("access-control-request-method", "PUT")
                .header(
                    "access-control-request-headers",
                    "authorization, content-type",
                )
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(preflight_response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        preflight_response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:8081")
    );
    assert_eq!(
        preflight_response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .and_then(|value| value.to_str().ok()),
        Some("GET, POST, PUT, DELETE, OPTIONS")
    );
    assert_eq!(
        preflight_response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .and_then(|value| value.to_str().ok()),
        Some("authorization, content-type")
    );

    let external_auths_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/profile/external-auths",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(external_auths_response.status(), StatusCode::OK);
    let external_auths_body = read_json(external_auths_response).await;
    assert_eq!(external_auths_body["success"], true);
    let external_auths = external_auths_body["result"]
        .as_array()
        .expect("external auths");
    assert_eq!(external_auths.len(), 2);
    assert_eq!(external_auths[0]["externalAuthType"], "github");
    assert_eq!(external_auths[0]["externalUsername"], "alice-gh");
    assert_eq!(external_auths[0]["linked"], true);
    assert!(external_auths[0]["createdAt"].as_i64().unwrap_or_default() > 0);
    assert_eq!(external_auths[1]["externalAuthType"], "google");
    assert_eq!(external_auths[1]["linked"], false);
    assert_eq!(external_auths[1]["createdAt"], 0);

    let missing_unlink_type_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/profile/external-auths/unlink",
            &token,
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(
        missing_unlink_type_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_unlink_type_response).await["message"],
        "externalAuthType is required"
    );

    let invalid_unlink_password_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/profile/external-auths/unlink",
            &token,
            json!({"externalAuthType": "github", "password": "wrong"}),
        ))
        .await?;
    assert_eq!(
        invalid_unlink_password_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(invalid_unlink_password_response).await["message"],
        "Invalid password"
    );

    let missing_unlink_password_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/profile/external-auths/unlink",
            &token,
            json!({"externalAuthType": "github"}),
        ))
        .await?;
    assert_eq!(
        missing_unlink_password_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_unlink_password_response).await["message"],
        "password is required"
    );

    let unlink_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/profile/external-auths/unlink",
            &token,
            json!({"externalAuthType": "github", "password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(unlink_response.status(), StatusCode::OK);
    assert_eq!(read_json(unlink_response).await["result"], true);
    assert_eq!(external_auth_count(fixture.db_path(), 42, "github")?, 0);
    assert_eq!(external_auth_count(fixture.db_path(), 77, "github")?, 1);
    let unlink_metadata = latest_auth_log_metadata(fixture.db_path(), "external_auth_unlinked")?;
    assert_eq!(unlink_metadata["external_auth_type"], "github");
    assert_eq!(unlink_metadata["external_auth_category"], "oauth2");

    let missing_link_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/profile/external-auths/unlink",
            &token,
            json!({"externalAuthType": "github", "password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(missing_link_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_link_response).await["message"],
        "Third-party login is not linked"
    );

    let missing_user_unlink_response = app
        .clone()
        .oneshot(trusted_user_json_request(
            999,
            Method::POST,
            "/api/profile/external-auths/unlink",
            json!({"externalAuthType": "github", "password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(missing_user_unlink_response.status(), StatusCode::NOT_FOUND);

    let profile_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/profile",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(profile_response.status(), StatusCode::OK);
    let profile_body = read_json(profile_response).await;
    assert_eq!(profile_body["result"]["username"], "alice");
    assert_eq!(profile_body["result"]["nickname"], "Alice A.");
    assert_eq!(profile_body["result"]["defaultCurrency"], "CNY");
    assert_eq!(profile_body["result"]["emailVerified"], true);

    let empty_update_response = app
        .clone()
        .oneshot(bearer_request(
            Method::PUT,
            "/api/profile",
            &token,
            Body::from("{}"),
        ))
        .await?;
    assert_eq!(empty_update_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(empty_update_response).await["message"],
        "Request body is required"
    );

    let update_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({
                "nickname": "Alice Profile",
                "language": "en",
                "defaultCurrency": "USD",
                "firstDayOfWeek": 2,
                "defaultAccountId": "100",
                "transactionEditScope": 3,
                "calendarDisplayType": 1,
                "cashAccountId": "",
                "importLearningEnabled": false,
                "investmentPlatformKeywords": ["蚂蚁财富", "雪球", "雪球"],
                "investmentProductKeywords": "基金,ETF",
                "investmentExcludeKeywords": ["还款"]
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    let updated_user = &update_body["result"]["user"];
    assert_eq!(updated_user["nickname"], "Alice Profile");
    assert_eq!(updated_user["language"], "en");
    assert_eq!(updated_user["defaultCurrency"], "USD");
    assert_eq!(updated_user["firstDayOfWeek"], 2);
    assert_eq!(updated_user["defaultAccountId"], "100");
    assert_eq!(updated_user["cashAccountId"], "");
    assert_eq!(updated_user["importLearningEnabled"], false);
    assert_eq!(
        updated_user["investmentPlatformKeywords"],
        json!(["蚂蚁财富", "雪球"])
    );
    assert_profile_db_values(fixture.db_path())?;

    let comprehensive_update_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({
                "email": "alice@example.test",
                "fiscalYearStart": "4",
                "dateDisplayType": 2,
                "longDateFormat": 3,
                "shortDateFormat": 4,
                "longTimeFormat": 5,
                "shortTimeFormat": 6,
                "fiscalYearFormat": 7,
                "currencyDisplayType": 8,
                "numeralSystem": 9,
                "decimalSeparator": 10,
                "digitGroupingSymbol": 11,
                "digitGrouping": 12,
                "coordinateDisplayType": 13,
                "expenseAmountColor": 14,
                "incomeAmountColor": 15,
                "cashAccountId": "100",
                "cashTransferCategoryId": "200",
                "importLearningEnabled": 1
            }),
        ))
        .await?;
    assert_eq!(comprehensive_update_response.status(), StatusCode::OK);
    let comprehensive_user =
        read_json(comprehensive_update_response).await["result"]["user"].clone();
    assert_eq!(comprehensive_user["email"], "alice@example.test");
    assert_eq!(comprehensive_user["fiscalYearStart"], 4);
    assert_eq!(comprehensive_user["dateDisplayType"], 2);
    assert_eq!(comprehensive_user["cashAccountId"], "100");
    assert_eq!(comprehensive_user["cashTransferCategoryId"], "200");
    assert_eq!(comprehensive_user["importLearningEnabled"], true);

    let restore_profile_baseline_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({
                "calendarDisplayType": 1,
                "cashAccountId": "",
                "importLearningEnabled": false
            }),
        ))
        .await?;
    assert_eq!(restore_profile_baseline_response.status(), StatusCode::OK);
    assert_profile_db_values(fixture.db_path())?;

    let malformed_numeric_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"fiscalYearStart": {"unexpected": true}}),
        ))
        .await?;
    assert_eq!(malformed_numeric_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(malformed_numeric_response).await["message"],
        "fiscalYearStart is invalid"
    );
    assert_profile_db_values(fixture.db_path())?;

    let malformed_bool_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"importLearningEnabled": "yes"}),
        ))
        .await?;
    assert_eq!(malformed_bool_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(malformed_bool_response).await["message"],
        "importLearningEnabled is invalid"
    );
    assert_profile_db_values(fixture.db_path())?;

    let malformed_string_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"language": {"unexpected": true}}),
        ))
        .await?;
    assert_eq!(malformed_string_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(malformed_string_response).await["message"],
        "language is invalid"
    );
    assert_profile_db_values(fixture.db_path())?;

    let cross_user_account_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"defaultAccountId": "101"}),
        ))
        .await?;
    assert_eq!(
        cross_user_account_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(cross_user_account_response).await["message"],
        "defaultAccountId is invalid"
    );

    let malformed_account_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"defaultAccountId": "abc"}),
        ))
        .await?;
    assert_eq!(malformed_account_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(malformed_account_response).await["message"],
        "defaultAccountId is invalid"
    );

    let cross_user_cash_account_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"cashAccountId": "101"}),
        ))
        .await?;
    assert_eq!(
        cross_user_cash_account_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(cross_user_cash_account_response).await["message"],
        "cashAccountId is invalid"
    );

    let cross_user_cash_transfer_category_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"cashTransferCategoryId": "201"}),
        ))
        .await?;
    assert_eq!(
        cross_user_cash_transfer_category_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(cross_user_cash_transfer_category_response).await["message"],
        "cashTransferCategoryId is invalid"
    );

    let malformed_cash_transfer_category_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"cashTransferCategoryId": "abc"}),
        ))
        .await?;
    assert_eq!(
        malformed_cash_transfer_category_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(malformed_cash_transfer_category_response).await["message"],
        "cashTransferCategoryId is invalid"
    );

    let malformed_numeric_bool_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"importLearningEnabled": 2}),
        ))
        .await?;
    assert_eq!(
        malformed_numeric_bool_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(malformed_numeric_bool_response).await["message"],
        "importLearningEnabled is invalid"
    );

    let avatar_in_profile_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"avatar": "data:text/html;base64,PGgxPkJvb208L2gxPg=="}),
        ))
        .await?;
    assert_eq!(avatar_in_profile_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(avatar_in_profile_response).await["message"],
        "Avatar must be updated via /api/profile/avatar"
    );

    let duplicate_email_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"email": "bob@example.test"}),
        ))
        .await?;
    assert_eq!(duplicate_email_response.status(), StatusCode::CONFLICT);
    assert_eq!(
        read_json(duplicate_email_response).await["message"],
        "Email already exists"
    );

    let email_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"email": "alice-new@example.test"}),
        ))
        .await?;
    assert_eq!(email_response.status(), StatusCode::OK);
    let email_body = read_json(email_response).await;
    assert_eq!(
        email_body["result"]["user"]["email"],
        "alice-new@example.test"
    );
    assert_eq!(email_body["result"]["user"]["emailVerified"], false);
    assert_auth_log(fixture.db_path(), "profile_email_changed", true, "")?;
    assert_eq!(
        latest_auth_log_ip(fixture.db_path(), "profile_email_changed")?,
        "198.51.100.13"
    );
    let email_change_metadata =
        latest_auth_log_metadata(fixture.db_path(), "profile_email_changed")?;
    assert_eq!(email_change_metadata["email_changed"], true);
    assert_eq!(email_change_metadata["email_verified_reset"], true);
    assert!(!email_change_metadata
        .to_string()
        .contains("alice-new@example.test"));
    assert!(!email_change_metadata
        .to_string()
        .contains("alice@example.test"));

    let invalid_email_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile",
            &token,
            json!({"email": "not-an-email"}),
        ))
        .await?;
    assert_eq!(invalid_email_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_email_response).await["message"],
        "Invalid email address"
    );

    let png_avatar_payload = b"\x89PNG\r\n\x1A\navatar-bytes";
    let avatar_response = app
        .clone()
        .oneshot(multipart_avatar_request(
            &token,
            png_avatar_payload,
            "image/png",
        ))
        .await?;
    assert_eq!(avatar_response.status(), StatusCode::OK);
    let avatar_body = read_json(avatar_response).await;
    assert_eq!(
        avatar_body["result"]["avatar"],
        format!(
            "data:image/png;base64,{}",
            general_purpose::STANDARD.encode(png_avatar_payload)
        )
    );

    let unsupported_avatar_response = app
        .clone()
        .oneshot(multipart_avatar_request(
            &token,
            b"avatar-bytes",
            "text/plain",
        ))
        .await?;
    assert_eq!(
        unsupported_avatar_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(unsupported_avatar_response).await["message"],
        "Unsupported avatar file type"
    );

    let empty_avatar_response = app
        .clone()
        .oneshot(multipart_avatar_request(&token, b"", "image/png"))
        .await?;
    assert_eq!(empty_avatar_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(empty_avatar_response).await["message"],
        "Avatar file is empty"
    );

    let remove_avatar_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/profile/avatar",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(remove_avatar_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(remove_avatar_response).await["result"]["avatar"],
        ""
    );

    let resend_response = app.clone().oneshot(profile_resend_request(&token)).await?;
    assert_eq!(resend_response.status(), StatusCode::OK);
    assert_eq!(read_json(resend_response).await["result"], true);
    assert_auth_log(
        fixture.db_path(),
        "verification_email_resend_requested",
        true,
        "Mozilla/5.0 (Profile contract)",
    )?;
    let resend_metadata =
        latest_auth_log_metadata(fixture.db_path(), "verification_email_resend_requested")?;
    assert_eq!(resend_metadata["email_present"], true);
    assert!(!resend_metadata
        .to_string()
        .contains("alice-new@example.test"));
    for _ in 0..2 {
        let repeated_resend_response = app.clone().oneshot(profile_resend_request(&token)).await?;
        assert_eq!(repeated_resend_response.status(), StatusCode::OK);
    }
    let throttled_resend_response = app.clone().oneshot(profile_resend_request(&token)).await?;
    assert_eq!(
        throttled_resend_response.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        read_json(throttled_resend_response).await["message"],
        "Too many verification email resend requests"
    );

    let empty_cloud_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/profile/cloud-settings",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(empty_cloud_response.status(), StatusCode::OK);
    assert_eq!(read_json(empty_cloud_response).await["result"], false);

    let missing_settings_response = app
        .clone()
        .oneshot(bearer_request(
            Method::PUT,
            "/api/profile/cloud-settings",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_settings_response.status(), StatusCode::OK);
    assert_eq!(read_json(missing_settings_response).await["result"], true);

    let empty_object_settings_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile/cloud-settings",
            &token,
            json!({}),
        ))
        .await?;
    assert_eq!(empty_object_settings_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(empty_object_settings_response).await["result"],
        true
    );

    let invalid_cloud_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile/cloud-settings",
            &token,
            json!({
                "settings": [{"settingKey": "showAmountInHomePage", "settingValue": "yes"}]
            }),
        ))
        .await?;
    assert_eq!(invalid_cloud_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_cloud_response).await["message"],
        "Invalid boolean value for showAmountInHomePage"
    );

    let update_cloud_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile/cloud-settings",
            &token,
            json!({
                "settings": [
                    {"settingKey": "showAmountInHomePage", "settingValue": "true"},
                    {"settingKey": "itemsCountInTransactionListPage", "settingValue": "50"},
                    {"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"1\":true}"}
                ],
                "fullUpdate": false
            }),
        ))
        .await?;
    assert_eq!(update_cloud_response.status(), StatusCode::OK);
    assert_eq!(read_json(update_cloud_response).await["result"], true);

    let cloud_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/profile/cloud-settings",
            &token,
            Body::empty(),
        ))
        .await?;
    let cloud_body = read_json(cloud_response).await;
    assert_eq!(cloud_body["result"].as_array().expect("settings").len(), 3);
    assert_eq!(
        cloud_body["result"][0]["settingKey"],
        "showAmountInHomePage"
    );

    let clear_without_settings_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile/cloud-settings",
            &token,
            json!({"fullUpdate": true}),
        ))
        .await?;
    assert_eq!(clear_without_settings_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(clear_without_settings_response).await["result"],
        true
    );
    assert_eq!(cloud_setting_count(fixture.db_path(), 42)?, 0);

    let full_update_response = app
        .clone()
        .oneshot(bearer_json_request(
            Method::PUT,
            "/api/profile/cloud-settings",
            &token,
            json!({
                "settings": [{"settingKey": "showAmountInHomePage", "settingValue": "false"}],
                "fullUpdate": true
            }),
        ))
        .await?;
    assert_eq!(full_update_response.status(), StatusCode::OK);
    assert_eq!(cloud_setting_count(fixture.db_path(), 42)?, 1);

    let delete_cloud_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/profile/cloud-settings",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_cloud_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_cloud_response).await["result"], true);
    assert_eq!(cloud_setting_count(fixture.db_path(), 42)?, 0);

    let no_db_app = runtime_router_without_sqlite_path();
    for (method, uri, body) in [
        (Method::GET, "/api/profile", Body::empty()),
        (
            Method::PUT,
            "/api/profile",
            Body::from(r#"{"nickname":"No DB"}"#),
        ),
        (Method::DELETE, "/api/profile/avatar", Body::empty()),
        (Method::GET, "/api/profile/cloud-settings", Body::empty()),
        (Method::DELETE, "/api/profile/cloud-settings", Body::empty()),
        (Method::GET, "/api/profile/external-auths", Body::empty()),
    ] {
        let response = no_db_app
            .clone()
            .oneshot(trusted_request(method, uri, body))
            .await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    Ok(())
}

#[tokio::test]
async fn auth_user_data_statistics_runtime_preserves_flask_contract() -> Result<(), Box<dyn Error>>
{
    let route = ("GET", "/api/data/statistics");
    assert!(AUTH_TOKEN_ROUTE_PATTERNS.iter().any(|item| item == &route));

    let no_db_response = runtime_router_without_sqlite_path()
        .oneshot(trusted_request(
            Method::GET,
            "/api/data/statistics",
            Body::empty(),
        ))
        .await?;
    assert_eq!(no_db_response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    seed_user_data_statistics_rows(fixture.db_path())?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(bearer_request(
            Method::GET,
            "/api/data/statistics",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["result"]["billCount"], 2);
    assert_eq!(body["result"]["accountCount"], 1);
    assert_eq!(body["result"]["categoryCount"], 1);
    assert_eq!(body["result"]["tagCount"], 3);
    assert_eq!(body["result"]["templateCount"], 1);

    Ok(())
}

#[tokio::test]
async fn auth_user_data_export_and_clear_runtime_preserve_flask_contracts(
) -> Result<(), Box<dyn Error>> {
    for route in [
        ("GET", "/api/data/export.{file_type}"),
        ("POST", "/api/data/clear/transactions"),
        ("POST", "/api/data/clear/all"),
    ] {
        assert!(AUTH_TOKEN_ROUTE_PATTERNS.iter().any(|item| item == &route));
    }

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    seed_user_data_management_rows(fixture.db_path())?;
    let app = runtime_router(&fixture);

    let no_db_app = runtime_router_without_sqlite_path();
    let no_db_export = no_db_app
        .clone()
        .oneshot(trusted_request(
            Method::GET,
            "/api/data/export.csv",
            Body::empty(),
        ))
        .await?;
    assert_eq!(no_db_export.status(), StatusCode::SERVICE_UNAVAILABLE);
    let no_db_clear = no_db_app
        .clone()
        .oneshot(trusted_user_json_request(
            42,
            Method::POST,
            "/api/data/clear/transactions",
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(no_db_clear.status(), StatusCode::SERVICE_UNAVAILABLE);
    let missing_user_clear = app
        .clone()
        .oneshot(trusted_user_json_request(
            999,
            Method::POST,
            "/api/data/clear/all",
            json!({"password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(missing_user_clear.status(), StatusCode::NOT_FOUND);

    let unsupported_export = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/data/export.json",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(unsupported_export.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(unsupported_export).await["message"],
        "Unsupported export file type"
    );

    let csv_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/data/export.csv?type=3&account_ids=100&tag_ids=400&category_ids=200",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(csv_response.status(), StatusCode::OK);
    assert!(csv_response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .starts_with("text/csv"));
    assert!(csv_response
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .contains("bill_analyser_export_"));
    let csv_text = read_text(csv_response).await;
    assert!(csv_text.starts_with('\u{feff}'));
    assert!(csv_text.contains("source_account"));
    assert!(csv_text.contains("导出测试账单"));
    assert!(csv_text.contains("'=Cash"));
    assert!(csv_text.contains("'@早餐|工作"));
    assert!(!csv_text.contains("其他用户账单"));

    let invalid_amount_filter = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/data/export.csv?amount_filter=gt:not-a-number",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_amount_filter.status(), StatusCode::OK);
    let invalid_amount_filter_text = read_text(invalid_amount_filter).await;
    assert!(invalid_amount_filter_text.contains("导出测试账单"));
    assert!(!invalid_amount_filter_text.contains("其他用户账单"));

    let tsv_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/data/export.tsv",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(tsv_response.status(), StatusCode::OK);
    let tsv_text = read_text(tsv_response).await;
    assert!(tsv_text.contains("\tsource_account\t"));

    let missing_auth = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/data/clear/transactions",
            &token,
            json!({}),
        ))
        .await?;
    assert_eq!(missing_auth.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_auth).await["message"],
        "Current password or stepUpToken is required"
    );

    {
        let connection = Connection::open(fixture.db_path())?;
        connection.execute(
            "DELETE FROM app_settings WHERE key = 'operation_password'",
            [],
        )?;
    }
    let wrong_password = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/data/clear/transactions",
            &token,
            json!({"password": "Wrong123!"}),
        ))
        .await?;
    assert_eq!(wrong_password.status(), StatusCode::UNAUTHORIZED);
    assert_auth_log(fixture.db_path(), "user_data_clear_auth_failed", false, "")?;
    assert_eq!(table_count(fixture.db_path(), "bills", 42)?, 1);

    let step_up_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "step_up",
        TEST_AUTH_SECRET,
        ChronoDuration::minutes(5),
    );
    let clear_transactions = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/data/clear/transactions",
            &token,
            json!({"stepUpToken": step_up_token}),
        ))
        .await?;
    assert_eq!(clear_transactions.status(), StatusCode::OK);
    let clear_transactions_body = read_json(clear_transactions).await;
    assert_eq!(clear_transactions_body["success"], true);
    assert_eq!(clear_transactions_body["result"], true);
    assert_eq!(clear_transactions_body["deletedCount"], 1);
    assert_eq!(table_count(fixture.db_path(), "bills", 42)?, 0);
    assert_eq!(table_count(fixture.db_path(), "bills", 77)?, 1);
    assert_eq!(table_count(fixture.db_path(), "accounts", 42)?, 1);
    let clear_transactions_audit = latest_user_data_audit(fixture.db_path(), "clear_transactions")?;
    assert_eq!(clear_transactions_audit.0, 42);
    assert_eq!(clear_transactions_audit.1, 1);
    assert_eq!(clear_transactions_audit.2["auth_mode"], "step_up");
    assert_eq!(clear_transactions_audit.2["deleted_count"], 1);

    seed_user_data_clear_all_rows(fixture.db_path())?;
    {
        let connection = Connection::open(fixture.db_path())?;
        connection.execute(
            r#"
            INSERT OR REPLACE INTO app_settings(
                key, value, value_type, description, is_encrypted, created_at, updated_at
            ) VALUES (
                'operation_password', 'operation-secret', 'string',
                'Test operation password', 0,
                '2026-01-01T00:00:00', '2026-01-01T00:00:00'
            )
            "#,
            [],
        )?;
    }
    let operation_password = std::env::var("BILL_ANALYSER_OPERATION_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "operation-secret".to_string());
    let clear_all = app
        .clone()
        .oneshot(bearer_json_request(
            Method::POST,
            "/api/data/clear/all",
            &token,
            json!({"password": operation_password}),
        ))
        .await?;
    assert_eq!(clear_all.status(), StatusCode::OK);
    let clear_all_body = read_json(clear_all).await;
    assert_eq!(clear_all_body["success"], true);
    assert_eq!(clear_all_body["result"], true);
    assert_eq!(clear_all_body["counts"]["accounts"], 2);
    assert_eq!(clear_all_body["counts"]["category_rules"], 1);
    assert_eq!(clear_all_body["counts"]["categories"], 2);
    assert_eq!(table_count(fixture.db_path(), "accounts", 42)?, 0);
    assert_eq!(table_count(fixture.db_path(), "category_rules", 42)?, 0);
    assert_eq!(table_count(fixture.db_path(), "category_rules", 77)?, 1);
    assert_eq!(table_count(fixture.db_path(), "categories", 42)?, 0);
    assert_eq!(table_count(fixture.db_path(), "bill_templates", 42)?, 0);
    assert_eq!(
        table_count(fixture.db_path(), "import_annotation_samples", 42)?,
        0
    );
    assert_eq!(table_count(fixture.db_path(), "llm_memory_events", 42)?, 0);
    assert_eq!(table_count(fixture.db_path(), "bills", 77)?, 1);
    let clear_all_audit = latest_user_data_audit(fixture.db_path(), "clear_all_user_data")?;
    assert_eq!(clear_all_audit.0, 42);
    assert_eq!(clear_all_audit.1, 13);
    assert_eq!(clear_all_audit.2["auth_mode"], "password");
    assert_eq!(clear_all_audit.2["accounts"], 2);

    Ok(())
}

#[tokio::test]
async fn auth_account_recovery_runtime_preserves_flask_contracts() -> Result<(), Box<dyn Error>> {
    for route in [
        ("POST", "/api/auth/email/verify"),
        ("POST", "/api/auth/email/resend-verification"),
        ("POST", "/api/auth/password/forgot"),
        ("POST", "/api/auth/password/reset"),
    ] {
        assert!(AUTH_TOKEN_ROUTE_PATTERNS.iter().any(|item| item == &route));
    }

    let disabled_fixture = RuntimeFixture::new()?;
    seed_auth_db(
        disabled_fixture.db_path(),
        &test_access_token(42, TEST_AUTH_SECRET),
    )?;
    let disabled_app = runtime_router(&disabled_fixture);
    let disabled_response = disabled_app
        .oneshot(json_post(
            "/api/auth/password/forgot",
            json!({"email": "alice@example.test"}),
        ))
        .await?;
    assert_eq!(disabled_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        read_json(disabled_response).await["message"],
        "Forget password is currently disabled"
    );

    let no_db_email_app = runtime_router_without_sqlite_path();
    let no_db_verify_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "verify_email",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let no_db_verify_response = no_db_email_app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/verify",
            json!({"token": no_db_verify_token}),
        ))
        .await?;
    assert_eq!(
        no_db_verify_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let no_db_resend_response = no_db_email_app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/resend-verification",
            json!({"email": "alice@example.test", "password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(
        no_db_resend_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let missing_parent = disabled_fixture
        .db_path()
        .parent()
        .expect("fixture parent")
        .join("missing-parent")
        .join("auth.db");
    let invalid_password_reset_app = runtime_router_with_password_reset(&missing_parent, true);
    let no_db_forgot_response = invalid_password_reset_app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/forgot",
            json!({"email": "alice@example.test"}),
        ))
        .await?;
    assert_eq!(
        no_db_forgot_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let no_db_reset_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "reset_password",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let no_db_reset_response = invalid_password_reset_app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/reset",
            json!({"email": "alice@example.test", "password": "new-password", "token": no_db_reset_token}),
        ))
        .await?;
    assert_eq!(
        no_db_reset_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let fixture = RuntimeFixture::new()?;
    seed_auth_db(fixture.db_path(), &test_access_token(42, TEST_AUTH_SECRET))?;
    set_email_verified(fixture.db_path(), 42, false)?;
    let app = runtime_router_with_password_reset(fixture.db_path(), true);

    let missing_verify_token_response = app
        .clone()
        .oneshot(json_post("/api/auth/email/verify", json!({})))
        .await?;
    assert_eq!(
        missing_verify_token_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_verify_token_response).await["message"],
        "Verification token is required"
    );

    let stale_verify_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "verify_email",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    set_user_email(fixture.db_path(), 42, "alice-new@example.test")?;
    let stale_verify_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/verify",
            json!({"token": stale_verify_token}),
        ))
        .await?;
    assert_eq!(stale_verify_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(stale_verify_response).await["message"],
        "Verification token does not match email"
    );
    assert!(!email_verified(fixture.db_path(), 42)?);
    set_user_email(fixture.db_path(), 42, "alice@example.test")?;

    let missing_user_verify_token = test_action_token(
        999,
        "ghost",
        "ghost@example.test",
        "verify_email",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let missing_user_verify_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/verify",
            json!({"token": missing_user_verify_token}),
        ))
        .await?;
    assert_eq!(missing_user_verify_response.status(), StatusCode::NOT_FOUND);

    let verify_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "verify_email",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let seed_updated_at = user_updated_at(fixture.db_path(), 42)?;
    let verify_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/verify",
            json!({"token": verify_token, "requestNewToken": true}),
        ))
        .await?;
    assert_eq!(verify_response.status(), StatusCode::OK);
    let verify_body = read_json(verify_response).await;
    assert_eq!(verify_body["success"], true);
    assert!(verify_body["result"]["newToken"].as_str().is_some());
    assert_eq!(verify_body["result"]["user"]["emailVerified"], true);
    assert!(email_verified(fixture.db_path(), 42)?);
    assert_ne!(user_updated_at(fixture.db_path(), 42)?, seed_updated_at);
    assert_auth_log(
        fixture.db_path(),
        "email_verified",
        true,
        "Mozilla/5.0 (JSON contract)",
    )?;

    let bad_resend_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/resend-verification",
            json!({"email": "alice@example.test", "password": "wrong-password"}),
        ))
        .await?;
    assert_eq!(bad_resend_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(bad_resend_response).await["message"],
        "Invalid email or password"
    );

    let resend_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/email/resend-verification",
            json!({"email": "alice@example.test", "password": TEST_PASSWORD}),
        ))
        .await?;
    assert_eq!(resend_response.status(), StatusCode::OK);
    assert_eq!(read_json(resend_response).await["result"], true);
    let resend_metadata =
        latest_auth_log_metadata(fixture.db_path(), "verification_email_resend_requested")?;
    assert_eq!(resend_metadata["email"], "alice@example.test");
    assert_eq!(resend_metadata["delivery"], "not_configured_mock_success");
    assert!(resend_metadata["verification_token"].as_str().is_some());

    let forgot_missing_response = app
        .clone()
        .oneshot(json_post("/api/auth/password/forgot", json!({})))
        .await?;
    assert_eq!(forgot_missing_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(forgot_missing_response).await["message"],
        "Email is required"
    );

    let forgot_unknown_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/forgot",
            json!({"email": "nobody@example.test"}),
        ))
        .await?;
    assert_eq!(forgot_unknown_response.status(), StatusCode::OK);
    assert_eq!(read_json(forgot_unknown_response).await["result"], true);

    let forgot_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/forgot",
            json!({"email": "alice@example.test"}),
        ))
        .await?;
    assert_eq!(forgot_response.status(), StatusCode::OK);
    let reset_metadata = latest_auth_log_metadata(fixture.db_path(), "password_reset_requested")?;
    let reset_token = reset_metadata["reset_token"]
        .as_str()
        .expect("reset token")
        .to_string();
    assert_eq!(reset_metadata["email"], "alice@example.test");

    let weak_reset_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/reset",
            json!({"email": "alice@example.test", "password": "short", "token": reset_token}),
        ))
        .await?;
    assert_eq!(weak_reset_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(weak_reset_response).await["error"],
        "Invalid password"
    );

    let mismatch_token = test_action_token(
        42,
        "alice",
        "alice@example.test",
        "reset_password",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let mismatch_reset_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/reset",
            json!({"email": "other@example.test", "password": "new-password", "token": mismatch_token}),
        ))
        .await?;
    assert_eq!(mismatch_reset_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(mismatch_reset_response).await["message"],
        "Reset password token does not match email"
    );

    let missing_user_reset_token = test_action_token(
        999,
        "ghost",
        "ghost@example.test",
        "reset_password",
        TEST_AUTH_SECRET,
        ChronoDuration::hours(1),
    );
    let missing_user_reset_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/reset",
            json!({"email": "ghost@example.test", "password": "new-password", "token": missing_user_reset_token}),
        ))
        .await?;
    assert_eq!(missing_user_reset_response.status(), StatusCode::NOT_FOUND);

    set_user_updated_at(fixture.db_path(), 42, "2026-01-01T00:00:00")?;
    let reset_response = app
        .clone()
        .oneshot(json_post(
            "/api/auth/password/reset",
            json!({"email": "alice@example.test", "password": "new-password", "token": reset_token}),
        ))
        .await?;
    assert_eq!(reset_response.status(), StatusCode::OK);
    assert_eq!(read_json(reset_response).await["result"], true);
    assert_password(fixture.db_path(), "alice", "new-password")?;
    assert_ne!(
        user_updated_at(fixture.db_path(), 42)?,
        "2026-01-01T00:00:00"
    );
    assert_auth_log(
        fixture.db_path(),
        "password_reset_completed",
        true,
        "Mozilla/5.0 (JSON contract)",
    )?;

    Ok(())
}

#[tokio::test]
async fn auth_token_runtime_preserves_flask_error_shapes() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let invalid_id_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens/not-an-int",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_id_response.status(), StatusCode::BAD_REQUEST);
    let invalid_id_body = read_json(invalid_id_response).await;
    assert_eq!(invalid_id_body["success"], false);
    assert_eq!(invalid_id_body["error"], "Invalid request");
    assert_eq!(
        invalid_id_body["message"],
        "tokenId must be a valid integer"
    );

    let trusted_delete_all_response = app
        .clone()
        .oneshot(trusted_request(
            Method::DELETE,
            "/api/tokens",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        trusted_delete_all_response.status(),
        StatusCode::UNAUTHORIZED
    );
    let trusted_delete_all_body = read_json(trusted_delete_all_response).await;
    assert_eq!(trusted_delete_all_body["error"], "Unauthorized");
    assert_eq!(
        trusted_delete_all_body["message"],
        "Current bearer session is required"
    );

    Ok(())
}

#[tokio::test]
async fn auth_token_runtime_generates_personal_and_refresh_tokens() -> Result<(), Box<dyn Error>> {
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tokens/api")));
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tokens/mcp")));
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tokens/refresh")));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let api_response = app
        .clone()
        .oneshot(personal_token_request(
            "/api/tokens/api",
            &token,
            TEST_PASSWORD,
            3600,
        ))
        .await?;
    assert_eq!(api_response.status(), StatusCode::OK);
    let api_body = read_json(api_response).await;
    assert_eq!(api_body["success"], true);
    let api_result = api_body["result"].as_object().expect("api result");
    let api_token = api_result["token"].as_str().expect("api token");
    assert_eq!(api_result["apiBaseUrl"], "https://api.example.test/api");
    let api_payload = jwt_payload(api_token);
    assert_eq!(api_payload["type"], "access");
    assert_eq!(api_payload["token_kind"], "api");
    assert_eq!(api_payload["user_id"], 42);
    assert_eq!(api_payload["username"], "alice");
    assert_eq!(
        api_payload["exp"].as_i64().expect("api exp")
            - api_payload["iat"].as_i64().expect("api iat"),
        3600
    );
    assert_token_session(
        fixture.db_path(),
        api_token,
        "Bill Analyser API Token",
        "198.51.100.10",
    )?;
    assert_auth_log(
        fixture.db_path(),
        "api_token_generate_success",
        true,
        "Bill Analyser API Token",
    )?;
    let api_token_list_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            api_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(api_token_list_response.status(), StatusCode::OK);

    let mcp_response = app
        .clone()
        .oneshot(personal_token_request(
            "/api/tokens/mcp",
            &token,
            TEST_PASSWORD,
            0,
        ))
        .await?;
    assert_eq!(mcp_response.status(), StatusCode::OK);
    let mcp_body = read_json(mcp_response).await;
    let mcp_result = mcp_body["result"].as_object().expect("mcp result");
    let mcp_token = mcp_result["token"].as_str().expect("mcp token");
    assert_eq!(mcp_result["mcpUrl"], "https://api.example.test/mcp");
    assert_eq!(jwt_payload(mcp_token)["token_kind"], "mcp");
    assert_token_session(
        fixture.db_path(),
        mcp_token,
        "Bill Analyser MCP Token",
        "198.51.100.10",
    )?;

    let peer_ip_response = app
        .clone()
        .oneshot(personal_token_request_with_peer(
            "/api/tokens/api",
            &token,
            TEST_PASSWORD,
            120,
            "203.0.113.9:4300".parse()?,
        ))
        .await?;
    assert_eq!(peer_ip_response.status(), StatusCode::OK);
    let peer_ip_body = read_json(peer_ip_response).await;
    let peer_ip_token = peer_ip_body["result"]["token"]
        .as_str()
        .expect("peer ip token");
    assert_token_session(
        fixture.db_path(),
        peer_ip_token,
        "Bill Analyser API Token",
        "203.0.113.9",
    )?;

    let invalid_password_response = app
        .clone()
        .oneshot(personal_token_request(
            "/api/tokens/api",
            &token,
            "wrong-password",
            3600,
        ))
        .await?;
    assert_eq!(invalid_password_response.status(), StatusCode::UNAUTHORIZED);
    let invalid_password_body = read_json(invalid_password_response).await;
    assert_eq!(invalid_password_body["error"], "Invalid credentials");
    assert_eq!(
        invalid_password_body["message"],
        "Current password is incorrect"
    );
    assert_auth_log(
        fixture.db_path(),
        "api_token_generate_failed",
        false,
        "Mozilla/5.0 (Rust contract)",
    )?;
    for octet in 10..14 {
        let mut retry_request =
            personal_token_request("/api/tokens/api", &token, "wrong-password", 3600);
        retry_request.headers_mut().insert(
            "x-forwarded-for",
            HeaderValue::from_str(&format!("192.0.2.{octet}")).expect("xff value"),
        );
        let retry_response = app.clone().oneshot(retry_request).await?;
        assert_eq!(retry_response.status(), StatusCode::UNAUTHORIZED);
    }
    let throttled_response = app
        .clone()
        .oneshot(personal_token_request(
            "/api/tokens/api",
            &token,
            "wrong-password",
            3600,
        ))
        .await?;
    assert_eq!(throttled_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        read_json(throttled_response).await["message"],
        "Too many failed token password attempts, please try again later"
    );

    let missing_password_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/tokens/api",
            &token,
            Body::from(json!({"expiresInSeconds": 3600}).to_string()),
        ))
        .await?;
    assert_eq!(missing_password_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_password_response).await["message"],
        "Current password is required"
    );

    let invalid_expires_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/tokens/api",
            &token,
            Body::from(
                json!({"password": TEST_PASSWORD, "expiresInSeconds": {"bad": true}}).to_string(),
            ),
        ))
        .await?;
    assert_eq!(invalid_expires_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_expires_response).await["message"],
        "expiresInSeconds must be a valid integer"
    );

    let trusted_generate_response = app
        .clone()
        .oneshot(trusted_request(
            Method::POST,
            "/api/tokens/api",
            Body::from(json!({"password": TEST_PASSWORD}).to_string()),
        ))
        .await?;
    assert_eq!(trusted_generate_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(trusted_generate_response).await["message"],
        "Current bearer session is required"
    );

    let refresh_token = test_refresh_token(42, "alice", TEST_AUTH_SECRET, ChronoDuration::days(1));
    seed_refresh_session(fixture.db_path(), &refresh_token)?;
    let refresh_response = app
        .clone()
        .oneshot(refresh_token_request(&refresh_token))
        .await?;
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_body = read_json(refresh_response).await;
    assert_eq!(refresh_body["success"], true);
    let refresh_result = refresh_body["result"].as_object().expect("refresh result");
    let new_access = refresh_result["token"].as_str().expect("new access");
    let new_refresh = refresh_result["refreshToken"]
        .as_str()
        .expect("new refresh");
    assert_eq!(refresh_result["newToken"], new_access);
    let new_access_payload = jwt_payload(new_access);
    let new_refresh_payload = jwt_payload(new_refresh);
    assert_eq!(new_access_payload["type"], "access");
    assert_eq!(new_access_payload.get("token_kind"), None);
    assert_eq!(new_refresh_payload["type"], "refresh");
    assert_eq!(new_refresh_payload["user_id"], 42);
    assert_eq!(refresh_result["user"]["username"], "alice");
    assert_eq!(refresh_result["user"]["nickname"], "Alice A.");
    assert_eq!(
        refresh_result["applicationCloudSettings"][0]["settingKey"],
        "showAmountInHomePage"
    );
    assert_session_token_pair(
        fixture.db_path(),
        new_access,
        new_refresh,
        "Mozilla/5.0 (Refresh contract)",
        "198.51.100.11",
    )?;
    assert_refresh_session_consumed(fixture.db_path(), 1)?;

    let replay_response = app
        .clone()
        .oneshot(refresh_token_request(&refresh_token))
        .await?;
    assert_eq!(replay_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(replay_response).await["message"],
        "Invalid refresh token"
    );

    let access_expired_refresh =
        test_refresh_token(42, "alice", TEST_AUTH_SECRET, ChronoDuration::days(1));
    seed_access_expired_refresh_session(fixture.db_path(), &access_expired_refresh)?;
    let cleanup_trigger_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            new_access,
            Body::empty(),
        ))
        .await?;
    assert_eq!(cleanup_trigger_response.status(), StatusCode::OK);
    let access_expired_response = app
        .clone()
        .oneshot(refresh_token_request(&access_expired_refresh))
        .await?;
    assert_eq!(access_expired_response.status(), StatusCode::OK);

    let missing_refresh_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/tokens/refresh",
            &token,
            Body::from(json!({}).to_string()),
        ))
        .await?;
    assert_eq!(missing_refresh_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_refresh_response).await["message"],
        "Refresh token is required"
    );

    let access_as_refresh_response = app.clone().oneshot(refresh_token_request(&token)).await?;
    assert_eq!(access_as_refresh_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(access_as_refresh_response).await["message"],
        "Not a refresh token"
    );

    let expired_refresh =
        test_refresh_token(42, "alice", TEST_AUTH_SECRET, -ChronoDuration::hours(1));
    let expired_response = app
        .clone()
        .oneshot(refresh_token_request(&expired_refresh))
        .await?;
    assert_eq!(expired_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(read_json(expired_response).await["error"], "Token expired");

    let missing_session_refresh =
        test_refresh_token(42, "alice", TEST_AUTH_SECRET, ChronoDuration::hours(1));
    let missing_session_response = app
        .clone()
        .oneshot(refresh_token_request(&missing_session_refresh))
        .await?;
    assert_eq!(missing_session_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(missing_session_response).await["message"],
        "Invalid refresh token"
    );

    Ok(())
}

#[tokio::test]
async fn auth_token_runtime_covers_configuration_and_db_error_edges() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let missing_auth_response = app
        .clone()
        .oneshot(Request::builder().uri("/api/tokens").body(Body::empty())?)
        .await?;
    assert_eq!(missing_auth_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(missing_auth_response).await["message"],
        "Missing authorization header"
    );

    let no_db_app = runtime_router_without_sqlite_path();
    let no_db_response = no_db_app
        .clone()
        .oneshot(trusted_request(Method::GET, "/api/tokens", Body::empty()))
        .await?;
    assert_eq!(no_db_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(no_db_response).await["message"],
        "Rust auth token runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    let invalid_path = fixture
        .db_path()
        .parent()
        .expect("fixture parent")
        .join("missing-parent")
        .join("auth.db");
    let invalid_path_app = runtime_router_with_db_path(&invalid_path, true);
    let invalid_path_response = invalid_path_app
        .oneshot(trusted_request(Method::GET, "/api/tokens", Body::empty()))
        .await?;
    assert_eq!(
        invalid_path_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let no_jwt_secret_app = runtime_router_with_db_path(fixture.db_path(), false);
    let no_jwt_secret_response = no_jwt_secret_app
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        no_jwt_secret_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        read_json(no_jwt_secret_response).await["error"],
        "Service Unavailable"
    );

    let missing_sessions_fixture = RuntimeFixture::new()?;
    Connection::open(missing_sessions_fixture.db_path())?.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL, email TEXT NOT NULL UNIQUE, is_active INTEGER NOT NULL DEFAULT 1);",
    )?;
    let missing_sessions_app =
        runtime_router_with_db_path(missing_sessions_fixture.db_path(), true);
    let missing_sessions_response = missing_sessions_app
        .oneshot(trusted_request(Method::GET, "/api/tokens", Body::empty()))
        .await?;
    assert_eq!(missing_sessions_response.status(), StatusCode::OK);
    let missing_sessions_body = read_json(missing_sessions_response).await;
    assert_eq!(missing_sessions_body["success"], true);
    assert_eq!(missing_sessions_body["result"].as_array().unwrap().len(), 0);

    let missing_bearer_sessions_fixture = RuntimeFixture::new()?;
    Connection::open(missing_bearer_sessions_fixture.db_path())?.execute_batch(
        "CREATE TABLE users (
             id INTEGER PRIMARY KEY,
             username TEXT NOT NULL,
             email TEXT NOT NULL UNIQUE,
             is_active INTEGER NOT NULL DEFAULT 1
         );
         INSERT INTO users(id, username, email, is_active)
             VALUES (42, 'alice', 'alice@example.test', 1);",
    )?;
    let missing_bearer_sessions_app =
        runtime_router_with_db_path(missing_bearer_sessions_fixture.db_path(), true);
    let missing_bearer_sessions_response = missing_bearer_sessions_app
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        missing_bearer_sessions_response.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        read_json(missing_bearer_sessions_response).await["message"],
        "Invalid or expired session"
    );
    let sessions_created: i64 = Connection::open(missing_bearer_sessions_fixture.db_path())?
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'sessions'",
            [],
            |row| row.get(0),
        )?;
    assert_eq!(sessions_created, 1);

    let bad_list_fixture = RuntimeFixture::new()?;
    seed_auth_db_missing_list_columns(bad_list_fixture.db_path(), &token)?;
    let bad_list_app = runtime_router_with_db_path(bad_list_fixture.db_path(), true);
    let bad_list_response = bad_list_app
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        bad_list_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let login_rollback_fixture = RuntimeFixture::new()?;
    seed_auth_db(login_rollback_fixture.db_path(), &token)?;
    let before_login_session_count = session_count(login_rollback_fixture.db_path())?;
    Connection::open(login_rollback_fixture.db_path())?.execute_batch(
        "CREATE TRIGGER fail_auth_log_insert
         BEFORE INSERT ON auth_logs
         BEGIN
             SELECT RAISE(FAIL, 'forced auth log failure');
         END;",
    )?;
    let login_rollback_app = runtime_router_with_db_path(login_rollback_fixture.db_path(), true);
    let login_rollback_response = login_rollback_app
        .oneshot(login_request("alice", TEST_PASSWORD))
        .await?;
    assert_eq!(
        login_rollback_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        session_count(login_rollback_fixture.db_path())?,
        before_login_session_count
    );
    assert_login_last_ip(login_rollback_fixture.db_path(), 42, "")?;

    let revoke_error_fixture = RuntimeFixture::new()?;
    seed_auth_db(revoke_error_fixture.db_path(), &token)?;
    install_revoke_failure_trigger(revoke_error_fixture.db_path())?;
    let revoke_error_app = runtime_router_with_db_path(revoke_error_fixture.db_path(), true);

    let revoke_other_error_response = revoke_error_app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        revoke_other_error_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let revoke_one_error_response = revoke_error_app
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens/2",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        revoke_one_error_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    Ok(())
}

struct RuntimeFixture {
    _temp_dir: TempDir,
    db_path: std::path::PathBuf,
}

impl RuntimeFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("auth-runtime.db");
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
        })
    }

    fn db_path(&self) -> &Path {
        &self.db_path
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    runtime_router_with_db_path(fixture.db_path(), true)
}

fn runtime_router_without_sqlite_path() -> Router {
    let config = legacy_sqlite_runtime_config()
        .with_trusted_user_header_secret(TEST_AUTH_SECRET)
        .with_auth_jwt_secret(TEST_AUTH_SECRET)
        .with_public_base_url("https://api.example.test");
    let state = HttpAppState::new(config).expect("http app state");
    build_router(state)
}

fn runtime_router_with_db_path(path: &Path, include_jwt_secret: bool) -> Router {
    let mut config = legacy_sqlite_runtime_config()
        .with_sqlite_db_path(path.display().to_string())
        .with_trusted_user_header_secret(TEST_AUTH_SECRET)
        .with_auth_enable_oauth2(true)
        .with_auth_oauth2_provider("google")
        .with_public_base_url("https://api.example.test");
    if include_jwt_secret {
        config = config.with_auth_jwt_secret(TEST_AUTH_SECRET);
    }
    let state = HttpAppState::new(config).expect("http app state");
    build_router(state)
}

fn runtime_router_with_password_reset(path: &Path, enabled: bool) -> Router {
    let config = legacy_sqlite_runtime_config()
        .with_sqlite_db_path(path.display().to_string())
        .with_trusted_user_header_secret(TEST_AUTH_SECRET)
        .with_auth_jwt_secret(TEST_AUTH_SECRET)
        .with_auth_enable_user_forget_password(enabled)
        .with_public_base_url("https://api.example.test");
    let state = HttpAppState::new(config).expect("http app state");
    build_router(state)
}

fn postgres_cutover_runtime_router() -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:59999",
        Duration::from_millis(200),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_database_backend(DatabaseBackend::Postgres)
    .with_require_postgres_after_cutover(true)
    .with_trusted_user_header_secret(TEST_AUTH_SECRET)
    .with_auth_jwt_secret(TEST_AUTH_SECRET)
    .with_public_base_url("https://api.example.test");
    let state = HttpAppState::new(config).expect("http app state");
    build_router(state)
}

fn postgres_auth_runtime_router(postgres_url: &str) -> Result<Router, Box<dyn Error>> {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:59999",
        Duration::from_millis(200),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )?
    .with_database_backend(DatabaseBackend::Postgres)
    .with_require_postgres_after_cutover(true)
    .with_postgres_url(postgres_url)?
    .with_trusted_user_header_secret(TEST_AUTH_SECRET)
    .with_auth_jwt_secret(TEST_AUTH_SECRET)
    .with_auth_enable_user_forget_password(true)
    .with_auth_enable_oauth2(true)
    .with_auth_oauth2_provider("google")
    .with_public_base_url("https://api.example.test");
    let state = HttpAppState::new(config).expect("http app state");
    Ok(build_router(state))
}

fn legacy_sqlite_runtime_config() -> HttpShellConfig {
    HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:59999",
        Duration::from_millis(200),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_database_backend(DatabaseBackend::Sqlite)
    .with_require_postgres_after_cutover(false)
    .with_legacy_sqlite_runtime_for_tests()
}

fn seed_auth_db(path: &Path, token: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            nickname TEXT,
            avatar TEXT,
            default_account_id INTEGER,
            transaction_edit_scope INTEGER DEFAULT 0,
            language TEXT DEFAULT 'zh_Hans',
            default_currency TEXT DEFAULT 'CNY',
            first_day_of_week INTEGER DEFAULT 1,
            fiscal_year_start INTEGER DEFAULT 1,
            calendar_display_type INTEGER DEFAULT 0,
            date_display_type INTEGER DEFAULT 0,
            long_date_format INTEGER DEFAULT 0,
            short_date_format INTEGER DEFAULT 0,
            long_time_format INTEGER DEFAULT 0,
            short_time_format INTEGER DEFAULT 0,
            fiscal_year_format INTEGER DEFAULT 0,
            currency_display_type INTEGER DEFAULT 0,
            numeral_system INTEGER DEFAULT 0,
            decimal_separator INTEGER DEFAULT 0,
            digit_grouping_symbol INTEGER DEFAULT 0,
            digit_grouping INTEGER DEFAULT 0,
            coordinate_display_type INTEGER DEFAULT 0,
            expense_amount_color INTEGER DEFAULT 0,
            income_amount_color INTEGER DEFAULT 0,
            cash_account_id INTEGER,
            cash_transfer_category_id INTEGER,
            import_learning_enabled INTEGER DEFAULT 1,
            investment_platform_keywords TEXT,
            investment_product_keywords TEXT,
            investment_exclude_keywords TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            two_factor_enabled INTEGER DEFAULT 0,
            two_factor_secret TEXT,
            failed_login_attempts INTEGER DEFAULT 0,
            locked_until TEXT,
            last_login_at TEXT,
            last_login_ip TEXT,
            email_verified INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT '2026-01-01T00:00:00',
            updated_at TEXT NOT NULL DEFAULT '2026-01-01T00:00:00'
        );
        CREATE TABLE sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token_hash TEXT NOT NULL,
            refresh_token_hash TEXT,
            expires_at TEXT NOT NULL,
            refresh_expires_at TEXT,
            user_agent TEXT,
            ip_address TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            last_activity_at TEXT,
            created_at TEXT
        );
        CREATE TABLE auth_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER,
            username TEXT,
            event_type TEXT NOT NULL,
            ip_address TEXT,
            user_agent TEXT,
            success INTEGER NOT NULL,
            error_message TEXT,
            metadata TEXT,
            created_at TEXT NOT NULL
        );
        CREATE TABLE audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            operation_type TEXT NOT NULL,
            operation_target TEXT NOT NULL,
            target_id INTEGER,
            details TEXT,
            affected_count INTEGER DEFAULT 0,
            ip_address TEXT,
            user_agent TEXT,
            session_id TEXT,
            status TEXT NOT NULL DEFAULT 'success',
            error_message TEXT,
            created_at TEXT NOT NULL
        );
        CREATE TABLE app_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key TEXT NOT NULL UNIQUE,
            value TEXT,
            value_type TEXT DEFAULT 'string',
            description TEXT,
            is_encrypted BOOLEAN DEFAULT 0,
            updated_at TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE user_two_factor_recovery_codes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            code_hash TEXT NOT NULL,
            used_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, code_hash)
        );
        CREATE TABLE user_application_cloud_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, setting_key)
        );
        CREATE TABLE user_external_auths (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            external_auth_category TEXT NOT NULL,
            external_auth_type TEXT NOT NULL,
            external_user_id TEXT,
            external_username TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, external_auth_type)
        );
        INSERT INTO app_settings(key, value, value_type, description, is_encrypted, created_at, updated_at)
        VALUES ('operation_password', 'operation-secret', 'string', 'Test operation password', 0, '2026-01-01T00:00:00', '2026-01-01T00:00:00');
        CREATE TABLE categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            type INTEGER DEFAULT 1,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            description TEXT,
            priority INTEGER DEFAULT 0,
            keywords TEXT,
            hidden INTEGER DEFAULT 0,
            icon TEXT,
            color TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(user_id, main_category, sub_category)
        );
        CREATE TABLE category_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            category_id INTEGER NOT NULL,
            name TEXT NOT NULL DEFAULT '',
            priority INTEGER NOT NULL DEFAULT 100,
            rule_expression TEXT NOT NULL,
            regex_enabled INTEGER DEFAULT 0,
            enabled INTEGER DEFAULT 1,
            applied_count INTEGER DEFAULT 0,
            last_applied_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            type INTEGER NOT NULL,
            category INTEGER,
            currency TEXT DEFAULT 'CNY',
            icon TEXT,
            color TEXT,
            balance REAL DEFAULT 0,
            initial_balance REAL DEFAULT 0,
            hidden INTEGER DEFAULT 0,
            display_order INTEGER DEFAULT 0,
            comment TEXT,
            aliases TEXT,
            parent_id INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX idx_sessions_token_hash ON sessions(token_hash);
        "#,
    )?;
    let password_hash = hash(TEST_PASSWORD, 4)?;
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, nickname, is_active, email_verified) VALUES (42, 'alice', 'alice@example.test', ?1, 'Alice A.', 1, 1)",
        [&password_hash],
    )?;
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, is_active) VALUES (77, 'bob', 'bob@example.test', ?1, 1)",
        [&password_hash],
    )?;
    connection.execute(
        "INSERT INTO accounts(id, user_id, name, type, created_at, updated_at) VALUES (100, 42, 'Cash', 1, '2026-01-01T00:00:00', '2026-01-01T00:00:00')",
        [],
    )?;
    connection.execute(
        "INSERT INTO accounts(id, user_id, name, type, created_at, updated_at) VALUES (101, 77, 'Other Cash', 1, '2026-01-01T00:00:00', '2026-01-01T00:00:00')",
        [],
    )?;
    connection.execute(
        "INSERT INTO categories(id, user_id, main_category, sub_category, created_at) VALUES (200, 42, 'Transfer', '', '2026-01-01T00:00:00')",
        [],
    )?;
    connection.execute(
        "INSERT INTO categories(id, user_id, main_category, sub_category, created_at) VALUES (201, 77, 'Other Transfer', '', '2026-01-01T00:00:00')",
        [],
    )?;
    connection.execute(
        r#"
        INSERT INTO user_external_auths(
            user_id, external_auth_category, external_auth_type,
            external_user_id, external_username, created_at, updated_at
        ) VALUES (42, 'oauth2', 'github', 'gh-42', 'alice-gh', '2026-01-02T00:00:00', '2026-01-02T00:00:00')
        "#,
        [],
    )?;
    connection.execute(
        r#"
        INSERT INTO user_external_auths(
            user_id, external_auth_category, external_auth_type,
            external_user_id, external_username, created_at, updated_at
        ) VALUES (77, 'oauth2', 'github', 'gh-77', 'bob-gh', '2026-01-03T00:00:00', '2026-01-03T00:00:00')
        "#,
        [],
    )?;

    let active_expires_at = (Local::now().naive_local() + ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    let current_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
    let rows = [
        (
            1,
            42,
            current_hash.as_str(),
            active_expires_at.as_str(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120 Safari/537.36",
            "127.0.0.1",
            1,
            "2026-01-01T00:00:00",
            "2026-01-01T00:00:00",
        ),
        (
            2,
            42,
            "api-hash",
            active_expires_at.as_str(),
            "Bill Analyser API Token",
            "127.0.0.2",
            1,
            "2026-01-02T00:00:00",
            "2026-01-02T00:00:00",
        ),
        (
            3,
            42,
            "mcp-hash",
            active_expires_at.as_str(),
            "Bill Analyser MCP Token",
            "127.0.0.3",
            1,
            "2026-01-03T00:00:00",
            "2026-01-03T00:00:00",
        ),
        (
            4,
            77,
            "other-user-hash",
            active_expires_at.as_str(),
            "Other user",
            "127.0.0.4",
            1,
            "2026-01-04T00:00:00",
            "2026-01-04T00:00:00",
        ),
        (
            5,
            42,
            "expired-hash",
            "2020-01-01T00:00:00",
            "Expired",
            "127.0.0.5",
            1,
            "2020-01-01T00:00:00",
            "2020-01-01T00:00:00",
        ),
        (
            6,
            42,
            "empty-date-hash",
            active_expires_at.as_str(),
            "",
            "",
            1,
            "",
            "",
        ),
    ];
    for row in rows {
        connection.execute(
            r#"
            INSERT INTO sessions (
                id, user_id, token_hash, expires_at, refresh_expires_at,
                user_agent, ip_address, is_active, last_activity_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9)
            "#,
            row,
        )?;
    }
    Ok(())
}

fn seed_user_data_statistics_rows(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        r#"
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL
        );
        CREATE TABLE tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE bill_templates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        INSERT INTO bills(user_id) VALUES (42), (42), (77);
        INSERT INTO tags(user_id, name, created_at, updated_at)
        VALUES
            (42, 'food', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
            (42, 'travel', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
            (42, 'work', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
            (77, 'other', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
        INSERT INTO bill_templates(user_id, name, created_at, updated_at)
        VALUES
            (42, 'monthly rent', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
            (77, 'other template', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
        "#,
    )?;
    Ok(())
}

fn seed_user_data_management_rows(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        r#"
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
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
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0,
            created_from_template INTEGER,
            created_from_recurring INTEGER,
            import_history_id INTEGER
        );
        CREATE TABLE tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            color TEXT,
            icon TEXT,
            display_order INTEGER DEFAULT 0,
            hidden INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE bill_tags (
            bill_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE bill_templates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE recurring_bills (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL);
        CREATE TABLE budgets (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL);
        CREATE TABLE budget_history (id INTEGER PRIMARY KEY AUTOINCREMENT, budget_id INTEGER NOT NULL);
        CREATE TABLE bills_preview (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL);
        CREATE TABLE bills_parser_template (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL);
        CREATE TABLE import_annotation_samples (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL);
        CREATE TABLE llm_memory_events (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL);
        CREATE TABLE import_sessions (id TEXT PRIMARY KEY, user_id INTEGER NOT NULL);
        CREATE TABLE saved_filters (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL);
        CREATE TABLE account_transfers (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL);
        CREATE TABLE account_types (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL);
        CREATE TABLE bill_pair_links (user_id INTEGER NOT NULL, left_bill_id INTEGER, right_bill_id INTEGER);
        CREATE TABLE bill_transfer_pair_suppressions (user_id INTEGER NOT NULL, left_bill_id INTEGER, right_bill_id INTEGER);
        CREATE TABLE bill_investment_pair_suppressions (user_id INTEGER NOT NULL, left_bill_id INTEGER, right_bill_id INTEGER);
        CREATE TABLE bill_learning_rule_suppressions (user_id INTEGER NOT NULL, bill_id INTEGER);

        INSERT INTO tags(id, user_id, name, created_at, updated_at)
        VALUES
            (400, 42, '@早餐', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
            (401, 42, '工作', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
            (402, 77, '其他标签', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
        UPDATE accounts SET name = '=Cash' WHERE id = 100 AND user_id = 42;
        INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description, payment_method,
            main_category, sub_category, created_at, updated_at, source_account_id,
            destination_account_id, destination_amount
        ) VALUES
            (300, 42, '2026-01-02 12:34:56', '支出', -12.34, '测试商户', '导出测试账单', '支付宝',
             'Transfer', '', '2026-01-02T00:00:00', '2026-01-02T00:00:00', 100, 0, 0),
            (301, 77, '2026-01-02 12:34:56', '支出', -88.00, '其他商户', '其他用户账单', '现金',
             'Other Transfer', '', '2026-01-02T00:00:00', '2026-01-02T00:00:00', 101, 0, 0);
        INSERT INTO bill_tags(bill_id, tag_id, created_at)
        VALUES (300, 400, 'now'), (300, 401, 'now'), (301, 402, 'now');
        INSERT INTO bill_templates(id, user_id, name, created_at, updated_at)
        VALUES (500, 42, '模板', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
        INSERT INTO bill_pair_links(user_id, left_bill_id, right_bill_id) VALUES (42, 300, 302);
        INSERT INTO bill_transfer_pair_suppressions(user_id, left_bill_id, right_bill_id) VALUES (42, 300, 302);
        INSERT INTO bill_investment_pair_suppressions(user_id, left_bill_id, right_bill_id) VALUES (42, 300, 302);
        INSERT INTO bill_learning_rule_suppressions(user_id, bill_id) VALUES (42, 300);
        "#,
    )?;
    Ok(())
}

fn seed_user_data_clear_all_rows(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        r#"
        INSERT INTO accounts(id, user_id, name, type, created_at, updated_at)
        VALUES (102, 42, 'Clear All Cash', 1, '2026-01-01T00:00:00', '2026-01-01T00:00:00');
        INSERT INTO categories(id, user_id, main_category, sub_category, created_at)
        VALUES (202, 42, '清理分类', '', '2026-01-01T00:00:00');
        INSERT INTO category_rules(
            id, user_id, category_id, name, priority, rule_expression, created_at, updated_at
        ) VALUES
            (203, 42, 202, '清理规则', 1, 'counterparty:工资', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
            (204, 77, 201, '其他规则', 1, 'counterparty:其他', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
        INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description, payment_method,
            main_category, sub_category, created_at, updated_at, source_account_id,
            destination_account_id, destination_amount
        ) VALUES
            (302, 42, '2026-01-03 12:34:56', '收入', 66.00, '工资', '清理账单', '银行卡',
             '清理分类', '', '2026-01-03T00:00:00', '2026-01-03T00:00:00', 102, 0, 0);
        INSERT INTO tags(id, user_id, name, created_at, updated_at)
        VALUES (403, 42, '清理标签', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
        INSERT INTO bill_tags(bill_id, tag_id, created_at) VALUES (302, 403, 'now');
        INSERT INTO bill_templates(id, user_id, name, created_at, updated_at)
        VALUES (501, 42, '清理模板', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
        INSERT INTO recurring_bills(id, user_id) VALUES (600, 42);
        INSERT INTO budgets(id, user_id) VALUES (700, 42);
        INSERT INTO budget_history(id, budget_id) VALUES (701, 700);
        INSERT INTO bills_preview(id, user_id) VALUES (800, 42);
        INSERT INTO bills_parser_template(id, user_id) VALUES (801, 42);
        INSERT INTO import_annotation_samples(id, user_id) VALUES (802, 42);
        INSERT INTO llm_memory_events(id, user_id) VALUES (803, 42);
        INSERT INTO import_sessions(id, user_id) VALUES ('clear-all-session', 42);
        INSERT INTO saved_filters(id, user_id) VALUES (900, 42);
        INSERT INTO account_transfers(id, user_id) VALUES (901, 42);
        INSERT INTO account_types(id, user_id) VALUES (902, 42);
        "#,
    )?;
    Ok(())
}

fn seed_login_edge_users(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let password_hash = hash(TEST_PASSWORD, 4)?;
    let future_lock = (Utc::now().naive_utc() + ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    let expired_lock = (Utc::now().naive_utc() - ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, is_active, two_factor_enabled) VALUES (78, 'twofa', 'twofa@example.test', ?1, 1, 1)",
        [&password_hash],
    )?;
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, is_active, locked_until, failed_login_attempts) VALUES (79, 'locked', 'locked@example.test', ?1, 1, ?2, 5)",
        (&password_hash, &future_lock),
    )?;
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, is_active) VALUES (80, 'inactive', 'inactive@example.test', ?1, 0)",
        [&password_hash],
    )?;
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, is_active, locked_until, failed_login_attempts) VALUES (81, 'nearlylocked', 'nearlylocked@example.test', ?1, 1, NULL, 4)",
        [&password_hash],
    )?;
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, is_active, locked_until, failed_login_attempts) VALUES (82, 'expiredlock', 'expiredlock@example.test', ?1, 1, ?2, 5)",
        (&password_hash, &expired_lock),
    )?;
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, is_active, locked_until, failed_login_attempts) VALUES (83, 'expiredwrong', 'expiredwrong@example.test', ?1, 1, ?2, 5)",
        (&password_hash, &expired_lock),
    )?;
    connection.execute(
        r#"
        INSERT INTO user_application_cloud_settings (
            user_id, setting_key, setting_value, created_at, updated_at
        ) VALUES (42, 'showAmountInHomePage', 'true', '2026-01-01T00:00:00', '2026-01-01T00:00:00')
        "#,
        [],
    )?;
    Ok(())
}

fn seed_auth_db_missing_list_columns(path: &Path, token: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            is_active INTEGER NOT NULL DEFAULT 1
        );
        CREATE TABLE sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token_hash TEXT NOT NULL,
            refresh_token_hash TEXT,
            expires_at TEXT NOT NULL,
            refresh_expires_at TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            last_activity_at TEXT,
            created_at TEXT
        );
        CREATE INDEX idx_sessions_token_hash ON sessions(token_hash);
        INSERT INTO users(id, username, email, is_active) VALUES (42, 'alice', 'alice@example.test', 1);
        "#,
    )?;
    let active_expires_at = (Local::now().naive_local() + ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    connection.execute(
        r#"
        INSERT INTO sessions (
            id, user_id, token_hash, expires_at, refresh_expires_at,
            is_active, last_activity_at, created_at
        ) VALUES (1, 42, ?1, ?2, NULL, 1, '2026-01-01T00:00:00', '2026-01-01T00:00:00')
        "#,
        (
            format!("{:x}", Sha256::digest(token.as_bytes())),
            active_expires_at,
        ),
    )?;
    Ok(())
}

fn seed_refresh_session(path: &Path, refresh_token: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let refresh_hash = format!("{:x}", Sha256::digest(refresh_token.as_bytes()));
    let refresh_expires_at = (Local::now().naive_local() + ChronoDuration::hours(2))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    connection.execute(
        "UPDATE sessions SET refresh_token_hash = ?1, refresh_expires_at = ?2 WHERE id = 1",
        (&refresh_hash, &refresh_expires_at),
    )?;
    connection.execute(
        r#"
        INSERT INTO user_application_cloud_settings (
            user_id, setting_key, setting_value, created_at, updated_at
        ) VALUES (42, 'showAmountInHomePage', 'true', '2026-01-01T00:00:00', '2026-01-01T00:00:00')
        "#,
        [],
    )?;
    Ok(())
}

fn seed_access_expired_refresh_session(
    path: &Path,
    refresh_token: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let refresh_hash = format!("{:x}", Sha256::digest(refresh_token.as_bytes()));
    let refresh_expires_at = (Local::now().naive_local() + ChronoDuration::hours(2))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    connection.execute(
        r#"
        INSERT INTO sessions (
            id, user_id, token_hash, refresh_token_hash, expires_at, refresh_expires_at,
            user_agent, ip_address, is_active, last_activity_at, created_at
        ) VALUES (
            88, 42, 'access-expired-refresh-backed', ?1,
            '2020-01-01T00:00:00', ?2,
            'Refresh backed', '127.0.0.88', 1, '2020-01-01T00:00:00', '2020-01-01T00:00:00'
        )
        "#,
        (&refresh_hash, &refresh_expires_at),
    )?;
    Ok(())
}

fn install_revoke_failure_trigger(path: &Path) -> Result<(), Box<dyn Error>> {
    Connection::open(path)?.execute_batch(
        r#"
        CREATE TRIGGER fail_session_revoke
        BEFORE UPDATE OF is_active ON sessions
        WHEN NEW.is_active = 0
        BEGIN
            SELECT RAISE(ABORT, 'forced revoke failure');
        END;
        "#,
    )?;
    Ok(())
}

fn session_is_active(path: &Path, session_id: i64) -> Result<bool, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let active = connection.query_row(
        "SELECT COUNT(*) FROM sessions WHERE id = ?1 AND is_active = 1",
        [session_id],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(active > 0)
}

fn assert_token_session(
    path: &Path,
    token: &str,
    expected_user_agent: &str,
    expected_ip_address: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
    let (user_agent, ip_address, refresh_hash, is_active): (String, String, Option<String>, i64) =
        connection.query_row(
            r#"
            SELECT user_agent, ip_address, refresh_token_hash, is_active
            FROM sessions
            WHERE token_hash = ?1
            "#,
            [token_hash],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
    assert_eq!(user_agent, expected_user_agent);
    assert_eq!(ip_address, expected_ip_address);
    assert_eq!(refresh_hash, None);
    assert_eq!(is_active, 1);
    Ok(())
}

fn assert_session_token_pair(
    path: &Path,
    access_token: &str,
    refresh_token: &str,
    expected_user_agent: &str,
    expected_ip_address: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let token_hash = format!("{:x}", Sha256::digest(access_token.as_bytes()));
    let expected_refresh_hash = format!("{:x}", Sha256::digest(refresh_token.as_bytes()));
    let (user_agent, ip_address, refresh_hash, is_active): (String, String, String, i64) =
        connection.query_row(
            r#"
            SELECT user_agent, ip_address, refresh_token_hash, is_active
            FROM sessions
            WHERE token_hash = ?1
            "#,
            [token_hash],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
    assert_eq!(user_agent, expected_user_agent);
    assert_eq!(ip_address, expected_ip_address);
    assert_eq!(refresh_hash, expected_refresh_hash);
    assert_eq!(is_active, 1);
    Ok(())
}

fn assert_refresh_session_consumed(path: &Path, session_id: i64) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (refresh_hash, is_active): (Option<String>, i64) = connection.query_row(
        "SELECT refresh_token_hash, is_active FROM sessions WHERE id = ?1",
        [session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(refresh_hash, None);
    assert_eq!(is_active, 0);
    Ok(())
}

fn assert_auth_log(
    path: &Path,
    expected_event: &str,
    expected_success: bool,
    expected_user_agent: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (success, user_agent): (i64, String) = connection.query_row(
        r#"
        SELECT success, user_agent
        FROM auth_logs
        WHERE event_type = ?1
        ORDER BY id DESC
        LIMIT 1
        "#,
        [expected_event],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(success == 1, expected_success);
    assert_eq!(user_agent, expected_user_agent);
    Ok(())
}

fn latest_audit_log_details(
    path: &Path,
    expected_operation: &str,
    expected_target_id: i64,
) -> Result<Value, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (target_id, status, affected_count, details): (i64, String, i64, String) = connection
        .query_row(
            r#"
            SELECT target_id, status, affected_count, COALESCE(details, '{}')
            FROM audit_logs
            WHERE operation_type = ?1
            ORDER BY id DESC
            LIMIT 1
            "#,
            [expected_operation],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
    assert_eq!(target_id, expected_target_id);
    assert_eq!(status, "success");
    assert_eq!(affected_count, 1);
    Ok(serde_json::from_str(&details)?)
}

fn latest_user_data_audit(
    path: &Path,
    expected_operation: &str,
) -> Result<(i64, i64, Value), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (target_id, target, status, affected_count, details): (i64, String, String, i64, String) =
        connection.query_row(
            r#"
            SELECT target_id, operation_target, status, affected_count, COALESCE(details, '{}')
            FROM audit_logs
            WHERE operation_type = ?1
            ORDER BY id DESC
            LIMIT 1
            "#,
            [expected_operation],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
    assert_eq!(target, "user_data");
    assert_eq!(status, "success");
    Ok((target_id, affected_count, serde_json::from_str(&details)?))
}

fn latest_auth_log_metadata(path: &Path, expected_event: &str) -> Result<Value, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let metadata: String = connection.query_row(
        r#"
        SELECT COALESCE(metadata, '{}')
        FROM auth_logs
        WHERE event_type = ?1
        ORDER BY id DESC
        LIMIT 1
        "#,
        [expected_event],
        |row| row.get(0),
    )?;
    Ok(serde_json::from_str(&metadata)?)
}

fn latest_auth_log_ip(path: &Path, expected_event: &str) -> Result<String, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    Ok(connection.query_row(
        r#"
        SELECT ip_address
        FROM auth_logs
        WHERE event_type = ?1
        ORDER BY id DESC
        LIMIT 1
        "#,
        [expected_event],
        |row| row.get(0),
    )?)
}

fn assert_login_state(
    path: &Path,
    user_id: i64,
    failed_attempts: i64,
    ip_address: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (attempts, last_login_ip): (i64, String) = connection.query_row(
        "SELECT failed_login_attempts, last_login_ip FROM users WHERE id = ?1",
        [user_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(attempts, failed_attempts);
    assert_eq!(last_login_ip, ip_address);
    Ok(())
}

fn assert_login_last_ip(
    path: &Path,
    user_id: i64,
    expected_ip_address: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let last_login_ip = connection.query_row(
        "SELECT COALESCE(last_login_ip, '') FROM users WHERE id = ?1",
        [user_id],
        |row| row.get::<_, String>(0),
    )?;
    assert_eq!(last_login_ip, expected_ip_address);
    Ok(())
}

fn session_count(path: &Path) -> Result<i64, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    Ok(connection.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?)
}

fn table_count(path: &Path, table_name: &str, user_id: i64) -> Result<i64, Box<dyn Error>> {
    assert!(table_name
        .chars()
        .all(|value| value.is_ascii_alphanumeric() || value == '_'));
    let connection = Connection::open(path)?;
    Ok(connection.query_row(
        &format!("SELECT COUNT(*) FROM {table_name} WHERE user_id = ?1"),
        [user_id],
        |row| row.get(0),
    )?)
}

fn assert_login_failure_count(
    path: &Path,
    user_id: i64,
    expected_attempts: i64,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let attempts = connection.query_row(
        "SELECT failed_login_attempts FROM users WHERE id = ?1",
        [user_id],
        |row| row.get::<_, i64>(0),
    )?;
    assert_eq!(attempts, expected_attempts);
    Ok(())
}

fn assert_login_locked(
    path: &Path,
    user_id: i64,
    expected_attempts: i64,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (attempts, locked_until): (i64, String) = connection.query_row(
        "SELECT failed_login_attempts, COALESCE(locked_until, '') FROM users WHERE id = ?1",
        [user_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(attempts, expected_attempts);
    assert!(!locked_until.trim().is_empty());
    Ok(())
}

fn assert_login_unlocked(path: &Path, user_id: i64) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let locked_until = connection.query_row(
        "SELECT COALESCE(locked_until, '') FROM users WHERE id = ?1",
        [user_id],
        |row| row.get::<_, String>(0),
    )?;
    assert_eq!(locked_until, "");
    Ok(())
}

fn seed_two_factor_recovery_code(
    path: &Path,
    user_id: i64,
    recovery_code: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let code_hash =
        hash_two_factor_recovery_code(recovery_code).expect("non-empty recovery code hashes");
    connection.execute(
        r#"
        INSERT INTO user_two_factor_recovery_codes(
            user_id, code_hash, created_at, updated_at
        ) VALUES (?1, ?2, '2026-01-01T00:00:00', '2026-01-01T00:00:00')
        "#,
        (user_id, code_hash),
    )?;
    Ok(())
}

fn recovery_code_is_used(
    path: &Path,
    user_id: i64,
    recovery_code: &str,
) -> Result<bool, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let code_hash =
        hash_two_factor_recovery_code(recovery_code).expect("non-empty recovery code hashes");
    Ok(connection.query_row(
        r#"
        SELECT used_at IS NOT NULL
        FROM user_two_factor_recovery_codes
        WHERE user_id = ?1 AND code_hash = ?2
        "#,
        (user_id, code_hash),
        |row| row.get::<_, i64>(0),
    )? != 0)
}

fn active_recovery_code_count(path: &Path, user_id: i64) -> Result<i64, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    Ok(connection.query_row(
        r#"
        SELECT COUNT(*)
        FROM user_two_factor_recovery_codes
        WHERE user_id = ?1 AND used_at IS NULL
        "#,
        [user_id],
        |row| row.get(0),
    )?)
}

fn two_factor_state(path: &Path, user_id: i64) -> Result<(bool, String), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (enabled, secret): (i64, String) = connection.query_row(
        "SELECT COALESCE(two_factor_enabled, 0), COALESCE(two_factor_secret, '') FROM users WHERE id = ?1",
        [user_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok((enabled != 0, secret))
}

fn set_two_factor_enabled(path: &Path, user_id: i64, enabled: bool) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "UPDATE users SET two_factor_enabled = ?1 WHERE id = ?2",
        (if enabled { 1 } else { 0 }, user_id),
    )?;
    Ok(())
}

fn set_two_factor_state(
    path: &Path,
    user_id: i64,
    enabled: bool,
    secret: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "UPDATE users SET two_factor_enabled = ?1, two_factor_secret = ?2 WHERE id = ?3",
        (if enabled { 1 } else { 0 }, secret, user_id),
    )?;
    Ok(())
}

fn set_user_active(path: &Path, user_id: i64, active: bool) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "UPDATE users SET is_active = ?1 WHERE id = ?2",
        (if active { 1 } else { 0 }, user_id),
    )?;
    Ok(())
}

fn delete_user(path: &Path, user_id: i64) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute("DELETE FROM users WHERE id = ?1", [user_id])?;
    Ok(())
}

fn seed_minimal_user_with_malformed_two_factor_status(
    path: &Path,
    user_id: i64,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "
        INSERT INTO users(id, username, email, password_hash, two_factor_enabled)
        VALUES (?1, 'malformed-2fa', 'malformed-2fa@example.test', 'hash', 'not-a-number')
        ",
        [user_id],
    )?;
    Ok(())
}

fn assert_registered_user_defaults(
    path: &Path,
    username: &str,
    password: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (user_id, password_hash, nickname, first_day_of_week, email_verified): (
        i64,
        String,
        String,
        i64,
        i64,
    ) = connection.query_row(
        "SELECT id, password_hash, nickname, first_day_of_week, email_verified FROM users WHERE username = ?1",
        [username],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )?;
    assert!(verify(password, &password_hash)?);
    assert_eq!(nickname, username);
    assert_eq!(first_day_of_week, 2);
    assert_eq!(email_verified, 1);

    let (default_account_id, cash_account_id): (i64, i64) = connection.query_row(
        "SELECT default_account_id, cash_account_id FROM users WHERE id = ?1",
        [user_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(default_account_id, cash_account_id);
    assert_eq!(
        connection.query_row(
            "SELECT COUNT(*) FROM accounts WHERE user_id = ?1",
            [user_id],
            |row| row.get::<_, i64>(0),
        )?,
        5
    );
    assert_eq!(
        connection.query_row(
            "SELECT COUNT(*) FROM categories WHERE user_id = ?1 AND main_category = '自定义'",
            [user_id],
            |row| row.get::<_, i64>(0),
        )?,
        2
    );
    assert!(connection.query_row(
        "SELECT COUNT(*) FROM categories WHERE user_id = ?1 AND main_category = '餐饮' AND sub_category = '外卖'",
        [user_id],
        |row| row.get::<_, i64>(0),
    )? > 0);
    assert_eq!(
        connection.query_row(
            "SELECT type FROM categories WHERE user_id = ?1 AND main_category = '账户互转' AND sub_category = ''",
            [user_id],
            |row| row.get::<_, i64>(0),
        )?,
        4
    );
    assert!(
        connection.query_row(
            "SELECT COUNT(*) FROM category_rules WHERE user_id = ?1 AND name = 'default:餐饮/外卖'",
            [user_id],
            |row| row.get::<_, i64>(0),
        )? > 0
    );
    Ok(())
}

fn assert_profile_db_values(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (
        nickname,
        language,
        default_currency,
        first_day_of_week,
        default_account_id,
        transaction_edit_scope,
        calendar_display_type,
        cash_account_id,
        import_learning_enabled,
        platform_keywords,
    ): (
        String,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        Option<i64>,
        i64,
        String,
    ) = connection.query_row(
        r#"
        SELECT
            nickname, language, default_currency, first_day_of_week,
            default_account_id, transaction_edit_scope, calendar_display_type,
            cash_account_id, import_learning_enabled, investment_platform_keywords
        FROM users WHERE id = 42
        "#,
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
            ))
        },
    )?;
    assert_eq!(nickname, "Alice Profile");
    assert_eq!(language, "en");
    assert_eq!(default_currency, "USD");
    assert_eq!(first_day_of_week, 2);
    assert_eq!(default_account_id, 100);
    assert_eq!(transaction_edit_scope, 3);
    assert_eq!(calendar_display_type, 1);
    assert_eq!(cash_account_id, None);
    assert_eq!(import_learning_enabled, 0);
    assert_eq!(platform_keywords, r#"["蚂蚁财富","雪球"]"#);
    Ok(())
}

fn assert_password(path: &Path, username: &str, password: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let password_hash: String = connection.query_row(
        "SELECT password_hash FROM users WHERE username = ?1",
        [username],
        |row| row.get(0),
    )?;
    assert!(verify(password, &password_hash)?);
    Ok(())
}

fn set_email_verified(path: &Path, user_id: i64, verified: bool) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "UPDATE users SET email_verified = ?1 WHERE id = ?2",
        (if verified { 1 } else { 0 }, user_id),
    )?;
    Ok(())
}

fn set_user_email(path: &Path, user_id: i64, email: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "UPDATE users SET email = ?1 WHERE id = ?2",
        (email, user_id),
    )?;
    Ok(())
}

fn set_user_updated_at(path: &Path, user_id: i64, updated_at: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "UPDATE users SET updated_at = ?1 WHERE id = ?2",
        (updated_at, user_id),
    )?;
    Ok(())
}

fn user_updated_at(path: &Path, user_id: i64) -> Result<String, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    Ok(connection.query_row(
        "SELECT updated_at FROM users WHERE id = ?1",
        [user_id],
        |row| row.get(0),
    )?)
}

fn email_verified(path: &Path, user_id: i64) -> Result<bool, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    Ok(connection.query_row(
        "SELECT email_verified FROM users WHERE id = ?1",
        [user_id],
        |row| Ok(row.get::<_, i64>(0)? != 0),
    )?)
}

fn cloud_setting_count(path: &Path, user_id: i64) -> Result<i64, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM user_application_cloud_settings WHERE user_id = ?1",
        [user_id],
        |row| row.get(0),
    )?)
}

fn external_auth_count(
    path: &Path,
    user_id: i64,
    external_auth_type: &str,
) -> Result<i64, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM user_external_auths WHERE user_id = ?1 AND external_auth_type = ?2",
        (user_id, external_auth_type),
        |row| row.get(0),
    )?)
}

fn token_by_id<'a>(tokens: &'a [Value], token_id: &str) -> &'a Value {
    tokens
        .iter()
        .find(|token| token["tokenId"] == token_id)
        .expect("token id exists")
}

fn login_request(login_name: &str, password: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .header("user-agent", "Mozilla/5.0 (Login contract)")
        .body(Body::from(
            json!({
                "loginName": login_name,
                "password": password,
            })
            .to_string(),
        ))
        .expect("login request builds");
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.30:4300"
            .parse::<SocketAddr>()
            .expect("peer addr"),
    ));
    request
}

fn login_request_with_origin_and_forwarded_ip(
    login_name: &str,
    password: &str,
    origin: &'static str,
    forwarded_for: &'static str,
) -> Request<Body> {
    let mut request = login_request(login_name, password);
    request
        .headers_mut()
        .insert(header::ORIGIN, HeaderValue::from_static(origin));
    request
        .headers_mut()
        .insert("x-forwarded-for", HeaderValue::from_static(forwarded_for));
    request
}

fn register_request(payload: Value) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .header("user-agent", "Mozilla/5.0 (Register contract)")
        .header(header::ORIGIN, "http://localhost:8081")
        .header("x-forwarded-for", "203.0.113.45")
        .body(Body::from(payload.to_string()))
        .expect("register request builds");
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.31:4300"
            .parse::<SocketAddr>()
            .expect("peer addr"),
    ));
    request
}

fn personal_token_request(
    uri: &str,
    token: &str,
    password: &str,
    expires_in_seconds: i64,
) -> Request<Body> {
    let mut request = personal_token_request_with_peer(
        uri,
        token,
        password,
        expires_in_seconds,
        "198.51.100.10:4300".parse().expect("test peer addr"),
    );
    request
        .headers_mut()
        .insert("x-forwarded-proto", HeaderValue::from_static("https"));
    request.headers_mut().insert(
        "x-forwarded-host",
        HeaderValue::from_static("attacker.example.test"),
    );
    request.headers_mut().insert(
        "x-forwarded-for",
        HeaderValue::from_static("192.0.2.99, 192.0.2.10"),
    );
    request
}

fn personal_token_request_with_peer(
    uri: &str,
    token: &str,
    password: &str,
    expires_in_seconds: i64,
    peer_addr: SocketAddr,
) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("host", "api.example.test")
        .header("user-agent", "Mozilla/5.0 (Rust contract)")
        .body(Body::from(
            json!({
                "password": password,
                "expiresInSeconds": expires_in_seconds,
            })
            .to_string(),
        ))
        .expect("personal token peer request builds");
    request.extensions_mut().insert(ConnectInfo(peer_addr));
    request
}

fn refresh_token_request(refresh_token: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/tokens/refresh")
        .header("content-type", "application/json")
        .header("user-agent", "Mozilla/5.0 (Refresh contract)")
        .body(Body::from(
            json!({ "refreshToken": refresh_token }).to_string(),
        ))
        .expect("refresh token request builds");
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.11:4300"
            .parse::<SocketAddr>()
            .expect("test peer addr"),
    ));
    request
}

fn logout_request(token: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/logout")
        .header("authorization", format!("Bearer {token}"))
        .header("user-agent", "Mozilla/5.0 (Logout contract)")
        .body(Body::empty())
        .expect("logout request builds");
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.12:4300"
            .parse::<SocketAddr>()
            .expect("test peer addr"),
    ));
    request
}

fn bearer_request(method: Method, uri: &str, token: &str, body: Body) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(body)
        .expect("request builds");
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.13:4300"
            .parse::<SocketAddr>()
            .expect("test bearer peer addr"),
    ));
    request
}

fn bearer_json_request(method: Method, uri: &str, token: &str, payload: Value) -> Request<Body> {
    bearer_request(method, uri, token, Body::from(payload.to_string()))
}

fn json_post(uri: &str, payload: Value) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .header("user-agent", "Mozilla/5.0 (JSON contract)")
        .body(Body::from(payload.to_string()))
        .expect("json post builds");
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.14:4300"
            .parse::<SocketAddr>()
            .expect("test json peer addr"),
    ));
    request
}

fn multipart_avatar_request(token: &str, content: &[u8], mime_type: &str) -> Request<Body> {
    let boundary = "profile-avatar-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"avatar\"; filename=\"avatar.txt\"\r\n",
    );
    body.extend_from_slice(format!("Content-Type: {mime_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Request::builder()
        .method(Method::POST)
        .uri("/api/profile/avatar")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .expect("multipart avatar request builds")
}

fn profile_resend_request(token: &str) -> Request<Body> {
    let mut request = bearer_request(
        Method::POST,
        "/api/profile/email/resend-verification",
        token,
        Body::empty(),
    );
    request.headers_mut().insert(
        "user-agent",
        HeaderValue::from_static("Mozilla/5.0 (Profile contract)"),
    );
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.13:4300"
            .parse::<SocketAddr>()
            .expect("test peer addr"),
    ));
    request
}

fn trusted_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-user-id", "42")
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(body)
        .expect("request builds")
}

fn trusted_user_json_request(
    user_id: i64,
    method: Method,
    uri: &str,
    payload: Value,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-user-id", user_id.to_string())
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(Body::from(payload.to_string()))
        .expect("trusted user request builds")
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}

async fn read_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    String::from_utf8(bytes.to_vec()).expect("utf-8 body")
}

fn jwt_payload(token: &str) -> Value {
    let payload = token.split('.').nth(1).expect("jwt payload segment exists");
    let bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("payload decodes");
    serde_json::from_slice(&bytes).expect("payload json")
}

fn jwt_lifetime_seconds(payload: &Value) -> i64 {
    payload["exp"].as_i64().expect("jwt exp") - payload["iat"].as_i64().expect("jwt iat")
}

fn test_access_token(user_id: i64, secret: &str) -> String {
    test_jwt_token(
        user_id,
        &format!("user-{user_id}"),
        "access",
        secret,
        ChronoDuration::hours(1),
    )
}

fn test_refresh_token(
    user_id: i64,
    username: &str,
    secret: &str,
    expires_in: ChronoDuration,
) -> String {
    test_jwt_token(user_id, username, "refresh", secret, expires_in)
}

fn test_action_token(
    user_id: i64,
    username: &str,
    email: &str,
    token_type: &str,
    secret: &str,
    expires_in: ChronoDuration,
) -> String {
    let now = Local::now();
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "user_id": user_id,
        "username": username,
        "email": email,
        "type": token_type,
        "iat": now.timestamp(),
        "exp": (now + expires_in).timestamp(),
        "nonce": "auth-action-route-test"
    });
    let encoded_header =
        general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header json"));
    let encoded_payload = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).expect("payload json"));
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let signature = hmac::sign(&key, signing_input.as_bytes());
    let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
    format!("{signing_input}.{encoded_signature}")
}

fn test_jwt_token(
    user_id: i64,
    username: &str,
    token_type: &str,
    secret: &str,
    expires_in: ChronoDuration,
) -> String {
    let now = Local::now();
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "user_id": user_id,
        "username": username,
        "type": token_type,
        "iat": now.timestamp(),
        "exp": (now + expires_in).timestamp(),
        "nonce": "auth-token-route-test"
    });
    let encoded_header =
        general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header json"));
    let encoded_payload = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).expect("payload json"));
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let signature = hmac::sign(&key, signing_input.as_bytes());
    let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
    format!("{signing_input}.{encoded_signature}")
}

fn totp_passcode(secret: &str, timestamp: i64) -> String {
    let counter = u64::try_from(timestamp.max(0) / 30).expect("nonnegative totp counter");
    let key_bytes = base32_decode(secret).expect("valid base32 secret");
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, &key_bytes);
    let digest = hmac::sign(&key, &counter.to_be_bytes());
    let bytes = digest.as_ref();
    let offset = usize::from(bytes[bytes.len() - 1] & 0x0f);
    let binary = ((u32::from(bytes[offset]) & 0x7f) << 24)
        | (u32::from(bytes[offset + 1]) << 16)
        | (u32::from(bytes[offset + 2]) << 8)
        | u32::from(bytes[offset + 3]);
    format!("{:06}", binary % 1_000_000)
}

fn base32_decode(secret: &str) -> Option<Vec<u8>> {
    let mut buffer = 0_u32;
    let mut bit_count = 0_u8;
    let mut output = Vec::new();
    for byte in secret.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a',
            b'2'..=b'7' => byte - b'2' + 26,
            b'=' | b' ' => continue,
            _ => return None,
        };
        buffer = (buffer << 5) | u32::from(value);
        bit_count += 5;
        while bit_count >= 8 {
            bit_count -= 8;
            output.push(((buffer >> bit_count) & 0xff) as u8);
        }
    }
    Some(output)
}
