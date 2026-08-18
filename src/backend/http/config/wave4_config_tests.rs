use super::*;
use std::collections::HashMap;

fn config_from(
    values: &[(&'static str, &'static str)],
) -> Result<HttpShellConfig, HttpShellConfigError> {
    let values = values.iter().copied().collect::<HashMap<_, _>>();
    HttpShellConfig::from_env_with(|name| values.get(name).map(|value| (*value).to_string()))
}

#[test]
fn missing_or_empty_postgres_url_fails_closed_without_explicit_development_profile() {
    assert_eq!(
        config_from(&[]).expect_err("generic runtime must reject a missing PostgreSQL URL"),
        HttpShellConfigError::MissingPostgresUrl
    );
    assert_eq!(
        config_from(&[("BILL_ANALYSER_POSTGRES_URL", "  ")])
            .expect_err("blank PostgreSQL URL is missing"),
        HttpShellConfigError::MissingPostgresUrl
    );
    assert_eq!(
        config_from(&[("BILL_ANALYSER_RUNTIME_PROFILE", "production")])
            .expect_err("production must not use development credentials"),
        HttpShellConfigError::MissingPostgresUrl
    );
}

#[test]
fn explicit_development_profile_can_use_documented_local_postgres_url() {
    let config = config_from(&[("BILL_ANALYSER_RUNTIME_PROFILE", "development")])
        .expect("explicit development profile enables the local default");
    assert_eq!(
        config.postgres_url.as_deref(),
        Some(DEFAULT_LOCAL_POSTGRES_URL)
    );
}

#[test]
fn confirm_receipt_read_source_is_metadata_by_default_and_typed_only_when_explicit() {
    let default = config_from(&[(
        "BILL_ANALYSER_POSTGRES_URL",
        "postgres://bill_analyser:test@127.0.0.1:5432/bill_analyser",
    )])
    .expect("default config");
    assert_eq!(
        default.confirm_receipt_read_source,
        ConfirmReceiptReadSource::Metadata
    );

    let typed = config_from(&[
        (
            "BILL_ANALYSER_POSTGRES_URL",
            "postgres://bill_analyser:test@127.0.0.1:5432/bill_analyser",
        ),
        (
            "BILL_ANALYSER_IMPORT_CONFIRM_RECEIPT_READ_SOURCE",
            "typed_v1",
        ),
    ])
    .expect("explicit typed receipt reader");
    assert_eq!(
        typed.confirm_receipt_read_source,
        ConfirmReceiptReadSource::TypedV1
    );

    assert_eq!(
        config_from(&[
            (
                "BILL_ANALYSER_POSTGRES_URL",
                "postgres://bill_analyser:test@127.0.0.1:5432/bill_analyser",
            ),
            (
                "BILL_ANALYSER_IMPORT_CONFIRM_RECEIPT_READ_SOURCE",
                "fallback",
            ),
        ])
        .expect_err("unknown receipt source must fail startup"),
        HttpShellConfigError::InvalidConfirmReceiptReadSource
    );
}
