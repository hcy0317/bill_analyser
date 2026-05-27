use std::{collections::HashMap, time::Duration};

use bill_analyser_core::auth::PasswordPolicy;
use bill_analyser_http::{
    config::{
        DatabaseBackend, MigrationMode, DEFAULT_AUTH_JWT_ALGORITHM,
        DEFAULT_AUTH_JWT_EXPIRATION_DAYS, DEFAULT_AUTH_LOCKOUT_DURATION_MINUTES,
        DEFAULT_AUTH_MAX_LOGIN_ATTEMPTS, DEFAULT_AUTH_PASSWORD_MIN_LENGTH,
        DEFAULT_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS, DEFAULT_BACKUP_DIR, DEFAULT_BODY_LIMIT_BYTES,
        DEFAULT_DATA_DIR, DEFAULT_TIMEOUT_MS, DEFAULT_UPLOADS_DIR,
    },
    http_shell_health, runtime_log_filter_from_directives, HttpAppState, HttpShellConfig,
    HttpShellConfigError, ImportRouteMode, RouteRepositoryBackend, DEFAULT_WEAVIATE_BATCH_SIZE,
    DEFAULT_WEAVIATE_RETRY_ATTEMPTS, DEFAULT_WEAVIATE_TIMEOUT_MS,
};

#[test]
fn http_shell_config_ignores_legacy_upstream_and_uses_rust_runtime_defaults() {
    let config = HttpShellConfig::new("not-a-url", Duration::from_millis(250), 4096).unwrap();

    assert_eq!(config.timeout, Duration::from_millis(250));
    assert_eq!(config.body_limit_bytes, 4096);
    assert_eq!(config.uploads_dir, DEFAULT_UPLOADS_DIR);
    assert_eq!(config.data_dir, DEFAULT_DATA_DIR);
    assert_eq!(config.backup_dir, DEFAULT_BACKUP_DIR);
    assert_eq!(config.backup_encryption_key, None);
    assert_eq!(config.import_route_mode, ImportRouteMode::ImportDbRuntime);
    assert_eq!(config.sqlite_db_path, None);
    assert_eq!(config.sqlite_legacy_path, None);
    assert_eq!(config.postgres_url, None);
    assert_eq!(config.database_backend, DatabaseBackend::Sqlite);
    assert_eq!(config.database_backend.as_str(), "sqlite");
    assert!(!config.database_backend.uses_postgres());
    assert_eq!(config.migration_mode, MigrationMode::Disabled);
    assert_eq!(config.migration_mode.as_str(), "disabled");
    assert!(!config.require_postgres_after_cutover);
    assert!(!config.postgres_configured());
    assert_eq!(config.redacted_postgres_url(), None);
    assert_eq!(config.trusted_user_header_secret, None);
    assert_eq!(config.auth_jwt_secret, None);
    assert_eq!(config.auth_jwt_algorithm, DEFAULT_AUTH_JWT_ALGORITHM);
    assert_eq!(
        config.auth_jwt_expiration_days,
        DEFAULT_AUTH_JWT_EXPIRATION_DAYS
    );
    assert_eq!(
        config.auth_refresh_token_expiration_days,
        DEFAULT_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS
    );
    assert_eq!(
        config.auth_max_login_attempts,
        DEFAULT_AUTH_MAX_LOGIN_ATTEMPTS
    );
    assert_eq!(
        config.auth_lockout_duration_minutes,
        DEFAULT_AUTH_LOCKOUT_DURATION_MINUTES
    );
    assert!(config.auth_enable_user_registration);
    assert!(!config.auth_require_email_verification);
    assert!(!config.auth_enable_user_forget_password);
    assert!(!config.auth_enable_oauth2);
    assert_eq!(config.auth_oauth2_provider, "");
    assert_eq!(
        config.auth_password_policy.min_length,
        DEFAULT_AUTH_PASSWORD_MIN_LENGTH
    );
    assert_eq!(config.public_base_url, None);
    assert!(!config.weaviate.enabled);
    assert_eq!(config.weaviate.endpoint, None);
    assert_eq!(config.weaviate.api_key, None);
    assert_eq!(config.weaviate.collection_prefix, "BillAnalyser");
    assert_eq!(
        config.weaviate.timeout,
        Duration::from_millis(DEFAULT_WEAVIATE_TIMEOUT_MS)
    );
    assert_eq!(
        config.weaviate.retry_attempts,
        DEFAULT_WEAVIATE_RETRY_ATTEMPTS
    );
    assert_eq!(config.weaviate.batch_size, DEFAULT_WEAVIATE_BATCH_SIZE);
    assert_eq!(config.weaviate.vector_dimensions, 96);
    assert_eq!(config.weaviate.status_without_probe(), "disabled");
    assert!(config.import_route_mode.intercepts_import_routes());
    assert_eq!(config.import_route_mode.as_str(), "import_db_runtime");
}

#[test]
fn runtime_logging_defaults_to_info_and_keeps_debug_opt_in() {
    let default_filter = runtime_log_filter_from_directives(Some(""));
    assert_eq!(default_filter.to_string(), "info");

    let debug_filter = runtime_log_filter_from_directives(Some("bill_analyser_http=debug"));
    assert!(debug_filter
        .to_string()
        .contains("bill_analyser_http=debug"));
}

#[test]
fn http_shell_config_builder_trims_optional_paths_and_auth_settings() {
    let policy = PasswordPolicy {
        min_length: 14,
        require_uppercase: true,
        require_lowercase: true,
        require_digit: true,
        require_special: true,
    };

    let config = HttpShellConfig::default()
        .with_sqlite_db_path("data/app.db")
        .with_sqlite_legacy_path(" data/legacy.db ")
        .with_database_backend(DatabaseBackend::Postgres)
        .with_migration_mode(MigrationMode::Validate)
        .with_require_postgres_after_cutover(true)
        .with_uploads_dir(" ")
        .with_data_dir(" data/runtime ")
        .with_backup_dir(" ")
        .with_backup_encryption_key(" secret-key ")
        .with_trusted_user_header_secret("trusted-secret")
        .with_auth_jwt_secret("jwt-secret")
        .with_auth_jwt_algorithm("HS512")
        .with_auth_jwt_expiration_days(3)
        .with_auth_refresh_token_expiration_days(8)
        .with_auth_max_login_attempts(9)
        .with_auth_lockout_duration_minutes(10)
        .with_auth_enable_user_registration(false)
        .with_auth_require_email_verification(true)
        .with_auth_enable_user_forget_password(true)
        .with_auth_enable_oauth2(true)
        .with_auth_oauth2_provider(" github ")
        .with_auth_password_policy(policy)
        .with_public_base_url(" https://example.test/api/ ")
        .with_postgres_url(" postgres://bill:secret@localhost:5432/bill_analyser?sslmode=disable ")
        .unwrap();

    assert_eq!(config.sqlite_db_path.as_deref(), Some("data/app.db"));
    assert_eq!(config.sqlite_legacy_path.as_deref(), Some("data/legacy.db"));
    assert_eq!(config.database_backend, DatabaseBackend::Postgres);
    assert!(config.database_backend.uses_postgres());
    assert_eq!(config.migration_mode, MigrationMode::Validate);
    assert!(config.require_postgres_after_cutover);
    assert_eq!(
        config.redacted_postgres_url().as_deref(),
        Some("postgres://bill:***@localhost:5432/bill_analyser")
    );
    assert_eq!(config.uploads_dir, DEFAULT_UPLOADS_DIR);
    assert_eq!(config.data_dir, "data/runtime");
    assert_eq!(config.backup_dir, DEFAULT_BACKUP_DIR);
    assert_eq!(config.backup_encryption_key.as_deref(), Some("secret-key"));
    assert_eq!(
        config.trusted_user_header_secret.as_deref(),
        Some("trusted-secret")
    );
    assert_eq!(config.auth_jwt_secret.as_deref(), Some("jwt-secret"));
    assert_eq!(config.auth_jwt_algorithm, "HS512");
    assert_eq!(config.auth_jwt_expiration_days, 3);
    assert_eq!(config.auth_refresh_token_expiration_days, 8);
    assert_eq!(config.auth_max_login_attempts, 9);
    assert_eq!(config.auth_lockout_duration_minutes, 10);
    assert!(!config.auth_enable_user_registration);
    assert!(config.auth_require_email_verification);
    assert!(config.auth_enable_user_forget_password);
    assert!(config.auth_enable_oauth2);
    assert_eq!(config.auth_oauth2_provider, "github");
    assert_eq!(config.auth_password_policy, policy);
    assert_eq!(
        config.public_base_url.as_deref(),
        Some("https://example.test/api")
    );

    let no_public_url = HttpShellConfig::default().with_public_base_url("   ");
    assert_eq!(no_public_url.public_base_url, None);

    let no_backup_key = HttpShellConfig::default().with_backup_encryption_key("   ");
    assert_eq!(no_backup_key.backup_encryption_key, None);
}

#[test]
fn http_shell_config_from_env_with_reads_rust_runtime_settings_and_legacy_auth_aliases() {
    let env = HashMap::from([
        ("BILL_ANALYSER_HTTP_TIMEOUT_MS", "1234"),
        ("BILL_ANALYSER_HTTP_BODY_LIMIT_BYTES", "2048"),
        ("BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE", "runtime"),
        ("BILL_ANALYSER_SQLITE_DB_PATH", " data/app.db "),
        ("BILL_ANALYSER_SQLITE_LEGACY_PATH", " data/legacy.db "),
        (
            "BILL_ANALYSER_POSTGRES_URL",
            " postgres://bill:secret@localhost:5432/bill_analyser?sslmode=disable ",
        ),
        ("BILL_ANALYSER_DATABASE_BACKEND", "postgresql"),
        ("BILL_ANALYSER_MIGRATION_MODE", "apply"),
        ("BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER", "true"),
        ("BILL_ANALYSER_UPLOADS_DIR", " data/uploads-custom "),
        ("BILL_ANALYSER_DATA_DIR", " data/runtime "),
        ("BILL_ANALYSER_BACKUP_DIR", " backup/runtime "),
        ("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", " backup-secret "),
        (
            "BILL_ANALYSER_TRUSTED_USER_HEADER_SECRET",
            " trusted-secret ",
        ),
        ("JWT_SECRET_KEY", " jwt-secret "),
        ("JWT_ALGORITHM", "HS512"),
        ("JWT_EXPIRATION_DAYS", "11"),
        ("REFRESH_TOKEN_EXPIRATION_DAYS", "22"),
        ("MAX_LOGIN_ATTEMPTS", "7"),
        ("LOCKOUT_DURATION_MINUTES", "9"),
        ("ENABLE_USER_REGISTRATION", "off"),
        ("REQUIRE_EMAIL_VERIFICATION", "on"),
        ("ENABLE_USER_FORGET_PASSWORD", "yes"),
        ("ENABLE_OAUTH2", "true"),
        ("OAUTH2_PROVIDER", " github "),
        ("PASSWORD_MIN_LENGTH", "13"),
        ("PASSWORD_REQUIRE_UPPERCASE", "1"),
        ("PASSWORD_REQUIRE_LOWERCASE", "true"),
        ("PASSWORD_REQUIRE_DIGIT", "yes"),
        ("PASSWORD_REQUIRE_SPECIAL", "on"),
        (
            "BILL_ANALYSER_PUBLIC_BASE_URL",
            " https://public.example.test/ ",
        ),
        ("BILL_ANALYSER_WEAVIATE_ENABLED", "true"),
        (
            "BILL_ANALYSER_WEAVIATE_ENDPOINT",
            " http://localhost:8080/ ",
        ),
        ("BILL_ANALYSER_WEAVIATE_API_KEY", " weaviate-secret "),
        ("BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX", "BillDev"),
        ("BILL_ANALYSER_WEAVIATE_TIMEOUT_MS", "1500"),
        ("BILL_ANALYSER_WEAVIATE_RETRY_ATTEMPTS", "3"),
        ("BILL_ANALYSER_WEAVIATE_BATCH_SIZE", "17"),
        ("BILL_ANALYSER_WEAVIATE_VECTOR_DIMENSIONS", "32"),
    ]);

    let config =
        HttpShellConfig::from_env_with(|name| env.get(name).map(|value| value.to_string()))
            .unwrap();

    assert_eq!(config.timeout, Duration::from_millis(1234));
    assert_eq!(config.body_limit_bytes, 2048);
    assert_eq!(config.import_route_mode, ImportRouteMode::ImportDbRuntime);
    assert_eq!(config.sqlite_db_path.as_deref(), Some("data/app.db"));
    assert_eq!(config.sqlite_legacy_path.as_deref(), Some("data/legacy.db"));
    assert_eq!(
        config.postgres_url.as_deref(),
        Some("postgres://bill:secret@localhost:5432/bill_analyser?sslmode=disable")
    );
    assert_eq!(
        config.redacted_postgres_url().as_deref(),
        Some("postgres://bill:***@localhost:5432/bill_analyser")
    );
    assert_eq!(config.database_backend, DatabaseBackend::Postgres);
    assert_eq!(config.migration_mode, MigrationMode::Apply);
    assert!(config.require_postgres_after_cutover);
    assert_eq!(config.uploads_dir, "data/uploads-custom");
    assert_eq!(config.data_dir, "data/runtime");
    assert_eq!(config.backup_dir, "backup/runtime");
    assert_eq!(
        config.backup_encryption_key.as_deref(),
        Some("backup-secret")
    );
    assert_eq!(
        config.trusted_user_header_secret.as_deref(),
        Some("trusted-secret")
    );
    assert_eq!(config.auth_jwt_secret.as_deref(), Some("jwt-secret"));
    assert_eq!(config.auth_jwt_algorithm, "HS512");
    assert_eq!(config.auth_jwt_expiration_days, 11);
    assert_eq!(config.auth_refresh_token_expiration_days, 22);
    assert_eq!(config.auth_max_login_attempts, 7);
    assert_eq!(config.auth_lockout_duration_minutes, 9);
    assert!(!config.auth_enable_user_registration);
    assert!(config.auth_require_email_verification);
    assert!(config.auth_enable_user_forget_password);
    assert!(config.auth_enable_oauth2);
    assert_eq!(config.auth_oauth2_provider, "github");
    assert_eq!(config.auth_password_policy.min_length, 13);
    assert!(config.auth_password_policy.require_uppercase);
    assert!(config.auth_password_policy.require_lowercase);
    assert!(config.auth_password_policy.require_digit);
    assert!(config.auth_password_policy.require_special);
    assert_eq!(
        config.public_base_url.as_deref(),
        Some("https://public.example.test")
    );
    assert!(config.weaviate.enabled);
    assert_eq!(
        config.weaviate.endpoint.as_deref(),
        Some("http://localhost:8080")
    );
    assert_eq!(config.weaviate.redacted_endpoint(), "http://localhost:8080");
    assert_eq!(config.weaviate.api_key.as_deref(), Some("weaviate-secret"));
    assert!(config.weaviate.api_key_configured());
    assert_eq!(config.weaviate.collection_prefix, "BillDev");
    assert_eq!(config.weaviate.timeout, Duration::from_millis(1500));
    assert_eq!(config.weaviate.retry_attempts, 3);
    assert_eq!(config.weaviate.batch_size, 17);
    assert_eq!(config.weaviate.vector_dimensions, 32);
    assert_eq!(config.weaviate.status_without_probe(), "configured");
}

#[test]
fn http_shell_config_from_env_with_keeps_empty_optional_values_at_defaults() {
    let env = HashMap::from([
        ("BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE", "import_db_runtime"),
        ("BILL_ANALYSER_SQLITE_DB_PATH", "   "),
        ("BILL_ANALYSER_SQLITE_LEGACY_PATH", "   "),
        ("BILL_ANALYSER_POSTGRES_URL", "   "),
        ("BILL_ANALYSER_DATABASE_BACKEND", "   "),
        ("BILL_ANALYSER_MIGRATION_MODE", "   "),
        ("BILL_ANALYSER_UPLOADS_DIR", "   "),
        ("BILL_ANALYSER_DATA_DIR", "   "),
        ("BILL_ANALYSER_BACKUP_DIR", "   "),
        ("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", "   "),
        ("BILL_ANALYSER_TRUSTED_USER_HEADER_SECRET", "   "),
        ("BILL_ANALYSER_AUTH_JWT_SECRET", "   "),
        ("BILL_ANALYSER_AUTH_JWT_ALGORITHM", "   "),
        ("BILL_ANALYSER_AUTH_OAUTH2_PROVIDER", "   "),
        ("BILL_ANALYSER_WEAVIATE_ENABLED", "false"),
        ("BILL_ANALYSER_WEAVIATE_ENDPOINT", "   "),
        ("BILL_ANALYSER_WEAVIATE_API_KEY", "   "),
        ("BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX", "   "),
    ]);

    let config =
        HttpShellConfig::from_env_with(|name| env.get(name).map(|value| value.to_string()))
            .unwrap();

    assert_eq!(config.timeout, Duration::from_millis(DEFAULT_TIMEOUT_MS));
    assert_eq!(config.body_limit_bytes, DEFAULT_BODY_LIMIT_BYTES);
    assert_eq!(config.sqlite_db_path, None);
    assert_eq!(config.sqlite_legacy_path, None);
    assert_eq!(config.postgres_url, None);
    assert_eq!(config.database_backend, DatabaseBackend::Sqlite);
    assert_eq!(config.migration_mode, MigrationMode::Disabled);
    assert_eq!(config.uploads_dir, DEFAULT_UPLOADS_DIR);
    assert_eq!(config.data_dir, DEFAULT_DATA_DIR);
    assert_eq!(config.backup_dir, DEFAULT_BACKUP_DIR);
    assert_eq!(config.backup_encryption_key, None);
    assert_eq!(config.trusted_user_header_secret, None);
    assert_eq!(config.auth_jwt_secret, None);
    assert_eq!(config.auth_jwt_algorithm, DEFAULT_AUTH_JWT_ALGORITHM);
    assert_eq!(config.auth_oauth2_provider, "");
    assert!(!config.weaviate.enabled);
    assert_eq!(config.weaviate.endpoint, None);
    assert_eq!(config.weaviate.api_key, None);
    assert_eq!(config.weaviate.collection_prefix, "BillAnalyser");
}

#[test]
fn http_shell_config_reports_invalid_runtime_env_values() {
    let invalids = [
        (
            "BILL_ANALYSER_HTTP_TIMEOUT_MS",
            "soon",
            HttpShellConfigError::InvalidInteger("BILL_ANALYSER_HTTP_TIMEOUT_MS"),
        ),
        (
            "BILL_ANALYSER_HTTP_BODY_LIMIT_BYTES",
            "large",
            HttpShellConfigError::InvalidInteger("BILL_ANALYSER_HTTP_BODY_LIMIT_BYTES"),
        ),
        (
            "BILL_ANALYSER_HTTP_BODY_LIMIT_BYTES",
            "0",
            HttpShellConfigError::InvalidBodyLimit,
        ),
        (
            "BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE",
            "python_proxy",
            HttpShellConfigError::InvalidImportRouteMode,
        ),
        (
            "BILL_ANALYSER_DATABASE_BACKEND",
            "mongo",
            HttpShellConfigError::InvalidDatabaseBackend,
        ),
        (
            "BILL_ANALYSER_MIGRATION_MODE",
            "rewrite",
            HttpShellConfigError::InvalidMigrationMode,
        ),
        (
            "BILL_ANALYSER_POSTGRES_URL",
            "sqlite://data/app.db",
            HttpShellConfigError::InvalidPostgresUrl,
        ),
        (
            "BILL_ANALYSER_AUTH_JWT_EXPIRATION_DAYS",
            "0",
            HttpShellConfigError::InvalidInteger("BILL_ANALYSER_AUTH_JWT_EXPIRATION_DAYS"),
        ),
        (
            "BILL_ANALYSER_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS",
            "366",
            HttpShellConfigError::InvalidInteger(
                "BILL_ANALYSER_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS",
            ),
        ),
        (
            "BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS",
            "101",
            HttpShellConfigError::InvalidInteger("BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS"),
        ),
        (
            "BILL_ANALYSER_AUTH_LOCKOUT_DURATION_MINUTES",
            "0",
            HttpShellConfigError::InvalidInteger("BILL_ANALYSER_AUTH_LOCKOUT_DURATION_MINUTES"),
        ),
        (
            "BILL_ANALYSER_AUTH_ENABLE_OAUTH2",
            "maybe",
            HttpShellConfigError::InvalidBoolean("BILL_ANALYSER_AUTH_ENABLE_OAUTH2"),
        ),
        (
            "BILL_ANALYSER_AUTH_PASSWORD_MIN_LENGTH",
            "0",
            HttpShellConfigError::InvalidInteger("BILL_ANALYSER_AUTH_PASSWORD_MIN_LENGTH"),
        ),
        (
            "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_SPECIAL",
            "sometimes",
            HttpShellConfigError::InvalidBoolean("BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_SPECIAL"),
        ),
        (
            "BILL_ANALYSER_PUBLIC_BASE_URL",
            "public.example.test",
            HttpShellConfigError::InvalidUpstream,
        ),
        (
            "BILL_ANALYSER_WEAVIATE_ENABLED",
            "sometimes",
            HttpShellConfigError::InvalidBoolean("BILL_ANALYSER_WEAVIATE_ENABLED"),
        ),
        (
            "BILL_ANALYSER_WEAVIATE_ENDPOINT",
            "ftp://localhost:8080",
            HttpShellConfigError::InvalidWeaviateEndpoint,
        ),
        (
            "BILL_ANALYSER_WEAVIATE_ENDPOINT",
            "http://user:secret@localhost:8080",
            HttpShellConfigError::InvalidWeaviateEndpoint,
        ),
        (
            "BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX",
            "bill-dev",
            HttpShellConfigError::InvalidWeaviateCollectionPrefix,
        ),
        (
            "BILL_ANALYSER_WEAVIATE_TIMEOUT_MS",
            "0",
            HttpShellConfigError::InvalidInteger("BILL_ANALYSER_WEAVIATE_TIMEOUT_MS"),
        ),
        (
            "BILL_ANALYSER_WEAVIATE_BATCH_SIZE",
            "0",
            HttpShellConfigError::InvalidInteger("BILL_ANALYSER_WEAVIATE_BATCH_SIZE"),
        ),
    ];

    for (name, value, expected) in invalids {
        let actual = HttpShellConfig::from_env_with(|lookup_name| {
            (lookup_name == name).then(|| value.to_string())
        })
        .unwrap_err();
        assert_eq!(actual, expected, "{name} should reject {value}");
    }

    assert_eq!(
        HttpShellConfig::new("", Duration::from_millis(1), 0).unwrap_err(),
        HttpShellConfigError::InvalidBodyLimit
    );
}

#[test]
fn http_shell_health_exposes_database_status_without_postgres_secret() {
    let config = HttpShellConfig::default()
        .with_sqlite_db_path("data/app.db")
        .with_sqlite_legacy_path("data/legacy.db")
        .with_database_backend(DatabaseBackend::Postgres)
        .with_migration_mode(MigrationMode::Apply)
        .with_require_postgres_after_cutover(true)
        .with_postgres_url("postgres://bill:secret@localhost:5432/bill_analyser?sslmode=disable")
        .unwrap();

    let health = http_shell_health(&config);

    assert_eq!(health.status, "unhealthy");
    assert_eq!(health.details["database_backend"], "postgres");
    assert_eq!(
        health.details["route_repository_backend"],
        "postgres_pending_repositories"
    );
    assert_eq!(
        health.details["postgres_cutover_status"],
        "blocked:postgres_repositories_pending"
    );
    assert_eq!(health.details["sqlite_db_path_configured"], "true");
    assert_eq!(health.details["sqlite_legacy_path_configured"], "true");
    assert_eq!(health.details["postgres_configured"], "true");
    assert_eq!(
        health.details["postgres_url_redacted"],
        "postgres://bill:***@localhost:5432/bill_analyser"
    );
    assert!(!health.details["postgres_url_redacted"].contains("secret"));
    assert_eq!(health.details["migration_mode"], "apply");
    assert_eq!(
        health.details["migration_status"],
        "placeholder:not_started"
    );
    assert_eq!(health.details["weaviate_status"], "disabled");
    assert_eq!(health.details["weaviate_endpoint_redacted"], "unconfigured");
    assert_eq!(health.details["weaviate_api_key_configured"], "false");
    assert_eq!(health.details["weaviate_collection_prefix"], "BillAnalyser");
    assert_eq!(health.details["require_postgres_after_cutover"], "true");
}

#[test]
fn http_shell_health_reports_config_gated_weaviate_without_secret_leakage() {
    let env = HashMap::from([
        ("BILL_ANALYSER_WEAVIATE_ENABLED", "true"),
        (
            "BILL_ANALYSER_WEAVIATE_ENDPOINT",
            "https://weaviate.example.test:8080",
        ),
        ("BILL_ANALYSER_WEAVIATE_API_KEY", "super-secret"),
        ("BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX", "BillProd"),
    ]);
    let config =
        HttpShellConfig::from_env_with(|name| env.get(name).map(|value| value.to_string()))
            .unwrap();
    let health = http_shell_health(&config);

    assert_eq!(health.details["weaviate_status"], "configured");
    assert_eq!(
        health.details["weaviate_endpoint_redacted"],
        "https://weaviate.example.test:8080"
    );
    assert_eq!(health.details["weaviate_api_key_configured"], "true");
    assert_eq!(health.details["weaviate_collection_prefix"], "BillProd");
    assert!(!serde_json::to_string(&health)
        .unwrap()
        .contains("super-secret"));

    let missing_endpoint = HttpShellConfig::from_env_with(|name| {
        (name == "BILL_ANALYSER_WEAVIATE_ENABLED").then(|| "true".to_string())
    })
    .unwrap();
    assert_eq!(
        http_shell_health(&missing_endpoint).details["weaviate_status"],
        "degraded:missing_endpoint"
    );
}

#[test]
fn http_app_state_exposes_repository_boundary_without_postgres_cutover() {
    let config = HttpShellConfig::default()
        .with_database_backend(DatabaseBackend::Postgres)
        .with_postgres_url("postgres://bill:secret@localhost:5432/bill_analyser")
        .unwrap();

    let health = http_shell_health(&config);
    assert_eq!(health.status, "unhealthy");
    assert_eq!(
        health.details["postgres_cutover_status"],
        "blocked:postgres_repositories_pending"
    );

    let state = HttpAppState::new(config).unwrap();

    let boundary = state.database_runtime_boundary();
    assert_eq!(
        boundary.route_repository_backend,
        RouteRepositoryBackend::PostgresPending
    );
    assert!(!boundary.sqlite_path_configured);
    assert!(boundary.postgres_url_configured);

    let error = match state.open_sqlite_repository_runtime("taxonomy") {
        Ok(_) => panic!("postgres-pending route boundary should reject sqlite runtime opens"),
        Err(error) => error,
    };
    assert_eq!(error.http_status_code(), 503);
    assert!(error.to_string().contains("not wired for PostgreSQL yet"));
}

#[test]
fn http_app_state_rejects_sqlite_runtime_after_postgres_cutover() {
    let config = HttpShellConfig::default()
        .with_sqlite_db_path("data/app.db")
        .with_require_postgres_after_cutover(true);
    let health = http_shell_health(&config);

    assert_eq!(health.status, "unhealthy");
    assert_eq!(
        health.details["route_repository_backend"],
        "postgres_required_after_cutover"
    );
    assert_eq!(
        health.details["postgres_cutover_status"],
        "blocked:database_backend_not_postgres"
    );

    let state = HttpAppState::new(config).unwrap();
    let boundary = state.database_runtime_boundary();
    assert_eq!(
        boundary.route_repository_backend,
        RouteRepositoryBackend::PostgresRequiredAfterCutover
    );

    let error = match state.open_sqlite_repository_runtime("import") {
        Ok(_) => panic!("cutover mode must not open sqlite business runtimes"),
        Err(error) => error,
    };
    assert_eq!(error.http_status_code(), 503);
    assert!(error
        .to_string()
        .contains("BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER=true"));
    assert!(error
        .to_string()
        .contains("blocked:database_backend_not_postgres"));
}

#[test]
fn http_shell_health_reports_cutover_missing_postgres_url() {
    let config = HttpShellConfig::default()
        .with_database_backend(DatabaseBackend::Postgres)
        .with_require_postgres_after_cutover(true);
    let health = http_shell_health(&config);

    assert_eq!(health.status, "unhealthy");
    assert_eq!(
        health.details["route_repository_backend"],
        "postgres_required_after_cutover"
    );
    assert_eq!(
        health.details["postgres_cutover_status"],
        "blocked:postgres_url_unconfigured"
    );
}

#[test]
fn http_app_state_keeps_sqlite_runtime_path_error_behind_boundary() {
    let state = HttpAppState::new(HttpShellConfig::default()).unwrap();

    let boundary = state.database_runtime_boundary();
    assert_eq!(
        boundary.route_repository_backend,
        RouteRepositoryBackend::SqliteLegacy
    );
    assert!(!boundary.sqlite_path_configured);

    let error = match state.open_sqlite_repository_runtime("bills") {
        Ok(_) => panic!("missing sqlite path should reject runtime opens"),
        Err(error) => error,
    };
    assert_eq!(error.http_status_code(), 503);
    assert!(error
        .to_string()
        .contains("Rust bills DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"));
}
