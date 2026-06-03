use bill_analyser_core::{runtime_health, runtime_identity_json, RuntimeStatus};
use serde_json::Value;

#[test]
fn runtime_health_identifies_postgres_weaviate_runtime_boundary() {
    let health = runtime_health();

    assert_eq!(health.status, RuntimeStatus::Ok);
    assert_eq!(health.identity.crate_name, "bill-analyser-core");
    assert_eq!(
        health.identity.runtime_boundary,
        "rust-core:postgres-authority+weaviate-required"
    );
    assert_eq!(
        health.details.get("database"),
        Some(&"postgres".to_string())
    );
    assert_eq!(
        health.details.get("vector_index"),
        Some(&"weaviate".to_string())
    );
}

#[test]
fn runtime_identity_json_is_stable_and_contains_no_business_domain_claim() {
    let identity_json = runtime_identity_json().expect("identity serializes");
    let value: Value = serde_json::from_str(&identity_json).expect("identity is valid json");

    assert_eq!(value["crate_name"], "bill-analyser-core");
    assert_eq!(
        value["runtime_boundary"],
        "rust-core:postgres-authority+weaviate-required"
    );
    assert!(value["version"].as_str().is_some());
}
