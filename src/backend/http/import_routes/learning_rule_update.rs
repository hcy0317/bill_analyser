// 中文导读：学习规则更新端点，负责 DTO 校验、用户隔离和当前响应投影。
// 维护重点：HTTP 层只编排仓储调用；字段持久化和 user-scope SQL 由 taxonomy repository 负责。
// 不变式：跨用户与不存在规则都返回同形 404，recommendation_key 不因人工编辑而改变。

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum NullableUpdate<T> {
    #[default]
    Missing,
    Value(Option<T>),
}

fn deserialize_nullable_update<'de, D, T>(deserializer: D) -> Result<NullableUpdate<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(NullableUpdate::Value)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LearningRuleUpdateRequest {
    #[serde(rename = "matchValue", alias = "match_value")]
    match_value: Option<String>,
    #[serde(rename = "learnedType", alias = "learned_type")]
    learned_type: Option<String>,
    #[serde(
        rename = "learnedCategoryId",
        alias = "learned_category_id",
        deserialize_with = "deserialize_nullable_update"
    )]
    learned_category_id: NullableUpdate<i64>,
    enabled: Option<bool>,
}

#[tracing::instrument(level = "debug", skip_all)]
/// 更新当前用户导入学习规则，保持规则字段和状态投影一致。
pub async fn learning_rule_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
    payload: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "learning_rule_update_runtime_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    if rule_id <= 0 {
        return route_response(learning_error_response(400, "invalid_rule_id"));
    }
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(_) => return route_response(learning_error_response(400, "invalid_payload")),
    };
    let payload = match serde_json::from_value::<LearningRuleUpdateRequest>(payload) {
        Ok(payload) => payload,
        Err(_) => return route_response(learning_error_response(400, "invalid_payload")),
    };
    let has_edit = payload.match_value.is_some()
        || payload.learned_type.is_some()
        || payload.learned_category_id != NullableUpdate::Missing
        || payload.enabled.is_some();
    if !has_edit {
        return route_response(learning_error_response(400, "no_fields_to_update"));
    }

    let match_value = match payload.match_value {
        Some(value) if value.trim().is_empty() => {
            return route_response(learning_error_response(400, "invalid_match_value"));
        }
        Some(value) => Some(value.trim().to_string()),
        None => None,
    };
    let learned_type = match payload.learned_type {
        Some(value) if value.trim().is_empty() => {
            return route_response(learning_error_response(400, "invalid_learned_type"));
        }
        Some(value) => Some(value.trim().to_string()),
        None => None,
    };
    let learned_category_id = match payload.learned_category_id {
        NullableUpdate::Missing => None,
        NullableUpdate::Value(Some(value)) if value <= 0 => {
            return route_response(learning_error_response(400, "invalid_learned_category_id"));
        }
        NullableUpdate::Value(value) => Some(value),
    };
    let match_features = match_value
        .as_ref()
        .and_then(|value| bill_analyser_core::parse_composite_match_value(Some(&json!(value))))
        .map(|features| json!(features));
    let user_id = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    let update = bill_analyser_db::taxonomy::postgres_reads::PostgresLearningRuleUpdate {
        match_value,
        match_features,
        learned_type,
        learned_category_id,
        enabled: payload.enabled,
    };
    match bill_analyser_db::taxonomy::postgres_reads::update_postgres_learning_rule(
        runtime.pool(),
        user_id,
        rule_id,
        &update,
    )
    .await
    {
        Ok(Some(rule)) => route_response(learning_data_response(rule)),
        Ok(None) => route_response(learning_error_response(404, "rule_not_found")),
        Err(DbError::InvalidOperation(message))
            if message == "invalid_match_value" || message == "invalid_learned_category_id" =>
        {
            route_response(learning_error_response(400, &message))
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

#[cfg(test)]
mod learning_rule_update_tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use bill_analyser_db::{run_postgres_migrations, PostgresPool};
    use serde_json::json;
    use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, Executor, Row};
    use std::{error::Error, str::FromStr};
    use tower::ServiceExt;
    use url::Url;

    struct IsolatedLearningPostgres {
        pool: PostgresPool,
        admin_pool: PostgresPool,
        database_url: String,
        db_name: String,
    }

    impl IsolatedLearningPostgres {
        async fn cleanup(self) -> Result<(), Box<dyn Error>> {
            self.pool.close().await;
            self.admin_pool
                .execute(
                    format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, self.db_name).as_str(),
                )
                .await?;
            Ok(())
        }
    }

    async fn isolated_learning_postgres() -> Result<Option<IsolatedLearningPostgres>, Box<dyn Error>>
    {
        let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
            eprintln!(
                "skipping learning-rule PostgreSQL contract: BILL_ANALYSER_TEST_POSTGRES_URL is not set"
            );
            return Ok(None);
        };
        let unique = Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_default()
            .unsigned_abs();
        let db_name = format!("learning_rule_update_{unique}");
        let base_options = PgConnectOptions::from_str(&postgres_url)?;
        let admin_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(base_options.clone().database("postgres"))
            .await?;
        admin_pool
            .execute(format!(r#"CREATE DATABASE "{db_name}""#).as_str())
            .await?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(base_options.database(&db_name))
            .await?;
        run_postgres_migrations(&pool).await?;
        let mut database_url = Url::parse(&postgres_url)?;
        database_url.set_path(&format!("/{db_name}"));

        Ok(Some(IsolatedLearningPostgres {
            pool,
            admin_pool,
            database_url: database_url.into(),
            db_name,
        }))
    }

    fn learning_update_router(state: HttpAppState) -> Router {
        Router::new()
            .route(
                "/api/learning/rules",
                get(learning_rules_list_runtime_handler),
            )
            .route(
                "/api/learning/rules/:rule_id/toggle",
                put(learning_rule_toggle_runtime_handler),
            )
            .route(
                "/api/learning/rules/:rule_id",
                put(learning_rule_update_runtime_handler).delete(learning_rule_delete_runtime_handler),
            )
            .with_state(state)
    }

    fn learning_update_request(
        rule_id: impl std::fmt::Display,
        user_id: i64,
        body: Value,
    ) -> Request<Body> {
        Request::builder()
            .method("PUT")
            .uri(format!("/api/learning/rules/{rule_id}"))
            .header("content-type", "application/json")
            .header(TRUSTED_USER_SECRET_HEADER, "learning-test-secret")
            .header("x-user-id", user_id.to_string())
            .body(Body::from(body.to_string()))
            .expect("learning update request")
    }

    fn learning_list_request(user_id: i64) -> Request<Body> {
        learning_list_request_with_query(user_id, "limit=500")
    }

    fn learning_list_request_with_query(user_id: i64, query: &str) -> Request<Body> {
        Request::builder()
            .method("GET")
            .uri(format!("/api/learning/rules?{query}"))
            .header(TRUSTED_USER_SECRET_HEADER, "learning-test-secret")
            .header("x-user-id", user_id.to_string())
            .body(Body::empty())
            .expect("learning list request")
    }

    fn learning_toggle_request(rule_id: i64, user_id: i64, enabled: bool) -> Request<Body> {
        Request::builder()
            .method("PUT")
            .uri(format!("/api/learning/rules/{rule_id}/toggle"))
            .header("content-type", "application/json")
            .header(TRUSTED_USER_SECRET_HEADER, "learning-test-secret")
            .header("x-user-id", user_id.to_string())
            .body(Body::from(json!({"enabled": enabled}).to_string()))
            .expect("learning toggle request")
    }

    fn learning_delete_request(rule_id: i64, user_id: i64) -> Request<Body> {
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/learning/rules/{rule_id}"))
            .header(TRUSTED_USER_SECRET_HEADER, "learning-test-secret")
            .header("x-user-id", user_id.to_string())
            .body(Body::empty())
            .expect("learning delete request")
    }

    async fn response_json(response: Response) -> Value {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        serde_json::from_slice(&body).expect("response JSON")
    }

    async fn insert_user(pool: &PostgresPool, username: &str) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
            .bind(username)
            .fetch_one(pool)
            .await
    }

    async fn insert_category(
        pool: &PostgresPool,
        user_id: i64,
        name: &str,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "INSERT INTO categories (user_id, name, category_type) VALUES ($1, $2, 'expense') RETURNING id",
        )
        .bind(user_id)
        .bind(name)
        .fetch_one(pool)
        .await
    }

    async fn insert_learning_rule(
        pool: &PostgresPool,
        user_id: i64,
        suffix: &str,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            r#"
            INSERT INTO import_learning_lifecycle (
                user_id, recommendation_key, recommendation_type, status,
                accepted_count, auto_apply_enabled, metadata
            ) VALUES ($1, $2, 'expense', 'green', 3, true, $3)
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(format!("stable-key-{suffix}"))
        .bind(json!({
            "match_type": "composite",
            "match_value": "p=alipay|c=old-shop",
            "match_features": {"parser_id": "alipay", "counterparty": "old-shop"},
            "learned_type": "expense"
        }))
        .fetch_one(pool)
        .await
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn update_learning_rule_persists_current_projection_and_user_scope(
    ) -> Result<(), Box<dyn Error>> {
        let Some(test_db) = isolated_learning_postgres().await? else {
            return Ok(());
        };
        let owner_id = insert_user(&test_db.pool, "learning-owner").await?;
        let other_id = insert_user(&test_db.pool, "learning-other").await?;
        let category_id = insert_category(&test_db.pool, owner_id, "Coffee").await?;
        let other_category_id = insert_category(&test_db.pool, other_id, "Other Coffee").await?;
        let owner_rule_id = insert_learning_rule(&test_db.pool, owner_id, "owner").await?;
        let other_rule_id = insert_learning_rule(&test_db.pool, other_id, "other").await?;
        let state = HttpAppState::new(
            HttpShellConfig::default()
                .with_postgres_url(test_db.database_url.clone())?
                .with_trusted_user_header_secret("learning-test-secret"),
        )?;
        let app = learning_update_router(state);

        let initial_list = app
            .clone()
            .oneshot(learning_list_request(owner_id))
            .await?;
        assert_eq!(initial_list.status(), StatusCode::OK);
        let initial_list_body = response_json(initial_list).await;
        assert_eq!(initial_list_body["data"]["total"], 1);
        assert_eq!(initial_list_body["data"]["items"][0]["id"], owner_rule_id);
        assert_eq!(
            initial_list_body["data"]["items"][0]["match_value"],
            "p=alipay|c=old-shop"
        );

        let updated = app
            .clone()
            .oneshot(learning_update_request(
                owner_rule_id,
                owner_id,
                json!({
                    "matchValue": "p=wechat|c=new-shop",
                    "learnedType": "income",
                    "learnedCategoryId": category_id,
                    "enabled": false
                }),
            ))
            .await?;
        assert_eq!(updated.status(), StatusCode::OK);
        let updated_body = response_json(updated).await;
        assert_eq!(updated_body["success"], true);
        assert_eq!(updated_body["data"]["id"], owner_rule_id);
        assert_eq!(updated_body["data"]["match_value"], "p=wechat|c=new-shop");
        assert_eq!(updated_body["data"]["learned_type"], "income");
        assert_eq!(updated_body["data"]["learned_category_id"], category_id);
        assert_eq!(updated_body["data"]["enabled"], false);

        let owner_row = sqlx::query(
            r#"
            SELECT recommendation_key, recommendation_type, status, metadata, version,
                   updated_at > created_at AS timestamp_advanced
            FROM import_learning_lifecycle
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(owner_rule_id)
        .bind(owner_id)
        .fetch_one(&test_db.pool)
        .await?;
        assert_eq!(
            owner_row.try_get::<String, _>("recommendation_key")?,
            "stable-key-owner"
        );
        assert_eq!(
            owner_row.try_get::<String, _>("recommendation_type")?,
            "income"
        );
        assert_eq!(owner_row.try_get::<String, _>("status")?, "disabled");
        assert_eq!(owner_row.try_get::<i64, _>("version")?, 2);
        assert!(owner_row.try_get::<bool, _>("timestamp_advanced")?);
        let metadata: Value = owner_row.try_get("metadata")?;
        assert_eq!(metadata["match_value"], "p=wechat|c=new-shop");
        assert_eq!(metadata["match_features"]["parser_id"], "wechat");
        assert_eq!(metadata["match_features"]["counterparty"], "new-shop");
        assert_eq!(metadata["learned_type"], "income");
        assert_eq!(metadata["learned_category_id"], category_id);

        let invalid_category = app
            .clone()
            .oneshot(learning_update_request(
                owner_rule_id,
                owner_id,
                json!({"learnedCategoryId": other_category_id}),
            ))
            .await?;
        assert_eq!(invalid_category.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(invalid_category).await,
            json!({"success": false, "error": "invalid_learned_category_id"})
        );
        let version_after_invalid_category: i64 =
            sqlx::query_scalar("SELECT version FROM import_learning_lifecycle WHERE id = $1")
                .bind(owner_rule_id)
                .fetch_one(&test_db.pool)
                .await?;
        assert_eq!(version_after_invalid_category, 2);

        let reenabled = app
            .clone()
            .oneshot(learning_update_request(
                owner_rule_id,
                owner_id,
                json!({"learnedCategoryId": null, "enabled": true}),
            ))
            .await?;
        assert_eq!(reenabled.status(), StatusCode::OK);
        let reenabled_body = response_json(reenabled).await;
        assert_eq!(reenabled_body["data"]["learned_category_id"], Value::Null);
        assert_eq!(reenabled_body["data"]["enabled"], true);
        let reenabled_row = sqlx::query(
            "SELECT status, auto_apply_enabled, metadata, version FROM import_learning_lifecycle WHERE id = $1",
        )
        .bind(owner_rule_id)
        .fetch_one(&test_db.pool)
        .await?;
        assert_eq!(reenabled_row.try_get::<String, _>("status")?, "green");
        assert!(reenabled_row.try_get::<bool, _>("auto_apply_enabled")?);
        assert_eq!(reenabled_row.try_get::<i64, _>("version")?, 3);
        let reenabled_metadata: Value = reenabled_row.try_get("metadata")?;
        assert_eq!(reenabled_metadata["learned_category_id"], Value::Null);
        assert!(reenabled_metadata.get("disabled_from_status").is_none());
        assert!(reenabled_metadata
            .get("disabled_from_auto_apply_enabled")
            .is_none());

        let updated_list = app
            .clone()
            .oneshot(learning_list_request(owner_id))
            .await?;
        assert_eq!(updated_list.status(), StatusCode::OK);
        let updated_list_body = response_json(updated_list).await;
        assert_eq!(updated_list_body["data"]["total"], 1);
        assert_eq!(
            updated_list_body["data"]["items"][0]["match_value"],
            "p=wechat|c=new-shop"
        );
        assert_eq!(
            updated_list_body["data"]["items"][0]["learned_type"],
            "income"
        );
        assert_eq!(updated_list_body["data"]["items"][0]["enabled"], true);
        assert_eq!(
            updated_list_body["data"]["items"][0]["learned_category_id"],
            Value::Null
        );

        let cross_user = app
            .clone()
            .oneshot(learning_update_request(
                other_rule_id,
                owner_id,
                json!({"learnedType": "transfer"}),
            ))
            .await?;
        assert_eq!(cross_user.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response_json(cross_user).await,
            json!({"success": false, "error": "rule_not_found"})
        );
        let other_row = sqlx::query(
            "SELECT recommendation_type, version FROM import_learning_lifecycle WHERE id = $1",
        )
        .bind(other_rule_id)
        .fetch_one(&test_db.pool)
        .await?;
        assert_eq!(
            other_row.try_get::<String, _>("recommendation_type")?,
            "expense"
        );
        assert_eq!(other_row.try_get::<i64, _>("version")?, 1);

        let missing = app
            .clone()
            .oneshot(learning_update_request(
                9_999_999_i64,
                owner_id,
                json!({"enabled": true}),
            ))
            .await?;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response_json(missing).await,
            json!({"success": false, "error": "rule_not_found"})
        );

        for (rule_id, payload) in [
            (owner_rule_id.to_string(), json!({})),
            (owner_rule_id.to_string(), json!({"matchValue": 42})),
            (owner_rule_id.to_string(), json!({"unexpected": true})),
            ("0".to_string(), json!({"enabled": true})),
            ("invalid".to_string(), json!({"enabled": true})),
        ] {
            let invalid = app
                .clone()
                .oneshot(learning_update_request(rule_id, owner_id, payload))
                .await?;
            assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
        }

        let cross_user_toggle = app
            .clone()
            .oneshot(learning_toggle_request(other_rule_id, owner_id, false))
            .await?;
        assert_eq!(cross_user_toggle.status(), StatusCode::NOT_FOUND);
        let cross_user_delete = app
            .clone()
            .oneshot(learning_delete_request(other_rule_id, owner_id))
            .await?;
        assert_eq!(cross_user_delete.status(), StatusCode::NOT_FOUND);

        let disabled = app
            .clone()
            .oneshot(learning_toggle_request(owner_rule_id, owner_id, false))
            .await?;
        assert_eq!(disabled.status(), StatusCode::OK);
        let disabled_body = response_json(disabled).await;
        assert_eq!(disabled_body["data"]["ruleId"], owner_rule_id);
        assert_eq!(disabled_body["data"]["enabled"], false);
        let enabled_only_list = app
            .clone()
            .oneshot(learning_list_request_with_query(
                owner_id,
                "enabled_only=true&limit=500",
            ))
            .await?;
        let enabled_only_body = response_json(enabled_only_list).await;
        assert_eq!(enabled_only_body["data"]["total"], 0);

        let deleted = app
            .clone()
            .oneshot(learning_delete_request(owner_rule_id, owner_id))
            .await?;
        assert_eq!(deleted.status(), StatusCode::OK);
        assert_eq!(response_json(deleted).await, json!({"success": true}));
        let empty_list = app
            .clone()
            .oneshot(learning_list_request(owner_id))
            .await?;
        assert_eq!(response_json(empty_list).await["data"]["total"], 0);

        test_db.cleanup().await?;
        Ok(())
    }

    #[test]
    fn adjacent_learning_rule_handlers_remain_available() {
        let _ = learning_rules_list_runtime_handler;
        let _ = learning_rule_toggle_runtime_handler;
        let _ = learning_rule_delete_runtime_handler;
    }
}
