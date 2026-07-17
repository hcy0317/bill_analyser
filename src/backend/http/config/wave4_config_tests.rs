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
