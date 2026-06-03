#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::HeaderValue;
    use bill_analyser_core::UserId;

    use super::*;
    use crate::config::HttpShellConfig;

    fn test_state(config: HttpShellConfig) -> HttpAppState {
        HttpAppState::new(config).expect("http app state")
    }

    #[test]
    fn helper_edges_cover_origin_ip_and_expires_parsing() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.10"));
        assert_eq!(client_ip(&headers, None), "203.0.113.10");
        assert_eq!(
            client_ip(&headers, Some("198.51.100.20:4300".parse().expect("addr"))),
            "198.51.100.20"
        );
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("192.0.2.99, 198.51.100.2"),
        );
        assert_eq!(
            client_ip(&headers, Some("198.51.100.20:4300".parse().expect("addr"))),
            "198.51.100.20"
        );
        assert_eq!(
            login_client_ip(&headers, Some("198.51.100.20:4300".parse().expect("addr"))),
            "192.0.2.99"
        );
        assert_eq!(client_ip(&HeaderMap::new(), None), FALLBACK_CLIENT_IP);

        assert!(request_origin(&test_state(HttpShellConfig::default())).is_err());
        assert_eq!(
            request_origin(&test_state(
                HttpShellConfig::default().with_public_base_url("https://public.test/")
            ))
            .ok()
            .as_deref(),
            Some("https://public.test")
        );

        let mut body = Map::new();
        assert_eq!(parse_expires_in_seconds(&body).expect("missing expires"), 0);
        body.insert("expiresInSeconds".to_string(), Value::Null);
        assert_eq!(parse_expires_in_seconds(&body).expect("null expires"), 0);
        body.insert("expiresInSeconds".to_string(), Value::Bool(true));
        assert_eq!(parse_expires_in_seconds(&body).expect("bool expires"), 1);
        body.insert("expiresInSeconds".to_string(), json!(12.8));
        assert_eq!(parse_expires_in_seconds(&body).expect("float expires"), 12);
        body.insert(
            "expiresInSeconds".to_string(),
            Value::String("30".to_string()),
        );
        assert_eq!(parse_expires_in_seconds(&body).expect("string expires"), 30);

        let warning = logout_session_not_found_warning_payload("0123456789abcdefdeadbeefcafebabe");
        assert_eq!(warning["event"], "logout_session_not_found");
        assert_eq!(warning["token_hash_prefix"], "0123456789abcdef");
        assert!(!warning.to_string().contains("deadbeef"));
    }

    #[test]
    fn issue_token_and_auth_error_edges_are_pinned() {
        let user_id = UserId::new(7).expect("user id");
        assert!(issue_access_token(
            user_id,
            "alice",
            &test_state(HttpShellConfig::default()),
            TokenKind::Api,
            60,
        )
        .is_err());
        assert!(issue_access_token(
            user_id,
            "alice",
            &test_state(
                HttpShellConfig::default()
                    .with_auth_jwt_secret("secret")
                    .with_auth_jwt_algorithm("none")
            ),
            TokenKind::Api,
            60,
        )
        .is_err());

        let issued = issue_access_token(
            user_id,
            "alice",
            &test_state(
                HttpShellConfig::new("http://127.0.0.1:5001", Duration::from_millis(100), 1024)
                    .expect("config")
                    .with_auth_jwt_secret("secret")
                    .with_auth_jwt_algorithm("HS512"),
            ),
            TokenKind::Session,
            0,
        )
        .expect("long-lived token");
        assert!(!issued.access_token.is_empty());
        assert!(issued.expires_at.contains('T'));

        assert_eq!(
            auth_error_response(RustRouteAuthError {
                status: 500,
                message: "internal".to_string(),
            })
            .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            auth_error_response(RustRouteAuthError {
                status: 418,
                message: "teapot".to_string(),
            })
            .status(),
            StatusCode::IM_A_TEAPOT
        );
    }

    #[test]
    fn profile_helper_edges_cover_updates_multipart_and_cloud_validation() {
        let body = json!({
            "nickname": "Alice",
            "email": "alice@example.test",
            "language": "en",
            "defaultCurrency": "USD",
            "firstDayOfWeek": 2,
            "defaultAccountId": "",
            "transactionEditScope": "3",
            "fiscalYearStart": 4,
            "calendarDisplayType": 5,
            "dateDisplayType": "6",
            "longDateFormat": 7,
            "shortDateFormat": 8,
            "longTimeFormat": 9,
            "shortTimeFormat": 10,
            "fiscalYearFormat": 11,
            "currencyDisplayType": 12,
            "numeralSystem": 13,
            "decimalSeparator": 14,
            "digitGroupingSymbol": 15,
            "digitGrouping": 16,
            "coordinateDisplayType": 17,
            "expenseAmountColor": 18,
            "incomeAmountColor": 19,
            "cashAccountId": null,
            "cashTransferCategoryId": "200",
            "importLearningEnabled": "0",
            "investmentPlatformKeywords": ["蚂蚁财富", "雪球"],
            "investmentProductKeywords": "基金",
            "investmentExcludeKeywords": ["还款"]
        });
        let updates = profile_updates_from_body(body.as_object().expect("object"))
            .expect("profile updates parse");
        assert_eq!(updates.len(), 29);
        assert!(updates.contains(&AuthUserProfileUpdate::Nickname("Alice".to_string())));
        assert!(!updates
            .iter()
            .any(|update| matches!(update, AuthUserProfileUpdate::Avatar(_))));
        assert_eq!(
            profile_updates_from_body(json!({ "avatar": true }).as_object().expect("object"))
                .expect_err("profile avatar update rejected")
                .message,
            "Avatar must be updated via /api/profile/avatar"
        );
        assert!(updates.contains(&AuthUserProfileUpdate::DefaultAccountId(None)));
        assert!(updates.contains(&AuthUserProfileUpdate::TransactionEditScope(3)));
        assert!(updates.contains(&AuthUserProfileUpdate::FiscalYearStart(4)));
        assert!(updates.contains(&AuthUserProfileUpdate::CashTransferCategoryId(Some(200))));
        assert!(updates.contains(&AuthUserProfileUpdate::ImportLearningEnabled(false)));
        assert_eq!(
            profile_string(&json!("en"), "language").expect("profile string"),
            "en"
        );
        assert!(profile_string(&Value::Null, "language").is_err());
        assert!(valid_email_address("alice@example.test"));
        assert!(!valid_email_address("not-an-email"));
        assert!(!valid_email_address("a@b@c.com"));
        assert!(!valid_email_address("alice @example.test"));
        assert!(!valid_email_address("alice@example .test"));
        assert!(!valid_email_address("alice@-example.test"));
        assert!(!valid_email_address("alice@example-.test"));
        assert_eq!(
            profile_email_changed(
                &[AuthUserProfileUpdate::Email("new@example.test".to_string())],
                "old@example.test",
            )
            .as_deref(),
            Some("new@example.test")
        );
        assert!(profile_email_changed(
            &[AuthUserProfileUpdate::Email(
                "same@example.test".to_string()
            )],
            "same@example.test",
        )
        .is_none());

        assert!(profile_i64(&Value::Bool(true), "firstDayOfWeek").is_err());
        assert_eq!(
            profile_i64(&json!("42"), "firstDayOfWeek").expect("numeric string"),
            42
        );
        assert!(profile_i64(
            &Value::Number(serde_json::Number::from(u64::MAX)),
            "firstDayOfWeek"
        )
        .is_err());
        assert!(profile_i64(&json!({"unexpected": true}), "firstDayOfWeek").is_err());
        assert!(profile_bool(&Value::Bool(true), "importLearningEnabled").expect("bool"));
        assert!(profile_bool(&json!(1), "importLearningEnabled").expect("one"));
        assert!(profile_bool(&json!("yes"), "importLearningEnabled").is_err());
        assert!(!profile_bool(&json!("false"), "importLearningEnabled").expect("false string"));
        assert!(!profile_bool(&json!("0"), "importLearningEnabled").expect("zero string"));
        assert!(profile_bool(&Value::Null, "importLearningEnabled").is_err());
        assert_eq!(
            optional_profile_id(&Value::Null, "defaultAccountId").expect("null clears"),
            None
        );
        assert_eq!(
            optional_profile_id(&json!(""), "defaultAccountId").expect("empty clears"),
            None
        );
        assert!(optional_profile_id(&json!("-1"), "defaultAccountId").is_err());
        assert!(optional_profile_id(&json!("0"), "defaultAccountId").is_err());
        assert!(optional_profile_id(&json!("abc"), "defaultAccountId").is_err());
        assert_eq!(
            optional_profile_id(&json!("7"), "defaultAccountId").expect("positive id"),
            Some(7)
        );

        let mut headers = HeaderMap::new();
        assert!(avatar_data_url_from_multipart(&headers, b"").is_err());
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=\"quoted\""),
        );
        assert_eq!(
            multipart_boundary(
                headers
                    .get(header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
            )
            .as_deref(),
            Some("quoted")
        );

        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=plain"),
        );
        let no_avatar =
            b"--plain\r\nContent-Disposition: form-data; name=\"other\"\r\n\r\nvalue\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, no_avatar).is_err());
        let empty_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.txt\"\r\n\r\n\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, empty_avatar).is_err());
        let text_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, text_avatar).is_err());
        let png_payload = b"\x89PNG\r\n\x1A\navatar";
        let png_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG\r\n\x1A\navatar\r\n--plain--\r\n";
        assert_eq!(
            avatar_data_url_from_multipart(&headers, png_avatar).expect("png avatar"),
            format!(
                "data:image/png;base64,{}",
                general_purpose::STANDARD.encode(png_payload)
            )
        );
        let mismatched_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\r\nContent-Type: image/jpeg\r\n\r\n\x89PNG\r\n\x1A\navatar\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, mismatched_avatar).is_err());
        assert!(avatar_data_url_from_payload(
            &vec![0_u8; MAX_AVATAR_BYTES + 1],
            Some("image/png".to_string())
        )
        .is_err());

        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=lf"),
        );
        let lf_avatar =
            b"--lf\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.webp\"\n\nRIFFxxxxWEBPdata\n--lf--\n";
        assert_eq!(
            avatar_data_url_from_multipart(&headers, lf_avatar).expect("lf avatar"),
            "data:image/webp;base64,UklGRnh4eHhXRUJQZGF0YQ=="
        );
        assert_eq!(find_bytes(b"abc", b""), None);
        assert_eq!(find_bytes(b"abc", b"abcd"), None);

        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "autoSaveTransactionDraft", "settingValue": "draft"})
            )
            .expect("string setting")
            .setting_value,
            "draft"
        );
        assert!(validate_application_cloud_setting(
            &json!({"settingKey": "itemsCountInTransactionListPage", "settingValue": "25"})
        )
        .is_ok());
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "itemsCountInTransactionListPage", "settingValue": "NaN?"})
            )
            .expect_err("invalid number")
            .message,
            "Invalid number value for itemsCountInTransactionListPage"
        );
        assert!(validate_application_cloud_setting(
            &json!({"settingKey": "showAmountInHomePage", "settingValue": "true"})
        )
        .is_ok());
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "showAmountInHomePage", "settingValue": "1"})
            )
            .expect_err("invalid boolean")
            .message,
            "Invalid boolean value for showAmountInHomePage"
        );
        assert!(validate_application_cloud_setting(
            &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"1\":true}"})
        )
        .is_ok());
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{"})
            )
            .expect_err("invalid json")
            .message,
            "Invalid JSON value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "[]"})
            )
            .expect_err("invalid map")
            .message,
            "Invalid map value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"\":false}"})
            )
            .expect_err("empty map key")
            .message,
            "Invalid map value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"1\":\"yes\"}"})
            )
            .expect_err("non boolean map value")
            .message,
            "Invalid map value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(&json!({"settingKey": "", "settingValue": "x"}))
                .expect_err("empty key")
                .message,
            "settingKey is required"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "autoSaveTransactionDraft", "settingValue": true})
            )
            .expect_err("non string value")
            .message,
            "Invalid setting value for autoSaveTransactionDraft"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "unsupported", "settingValue": "x"})
            )
            .expect_err("unsupported key")
            .message,
            "Unsupported setting key: unsupported"
        );
        assert_eq!(application_cloud_setting_type("unknown"), None);
    }
}
