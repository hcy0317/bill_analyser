use bill_analyser_core::{
    build_weaviate_batch_upsert_payload, build_weaviate_collection_names,
    build_weaviate_delete_path, build_weaviate_derived_object, build_weaviate_graphql_query,
    build_weaviate_required_metadata, build_weaviate_schema_classes,
    derive_weaviate_feature_vector, deterministic_weaviate_object_id,
    validate_weaviate_collection_prefix, WeaviateDerivedClass, WeaviateFilterValue,
    WeaviateMetadataFilter, FEATURE_SCHEMA_VERSION, WEAVIATE_DEFAULT_COLLECTION_PREFIX,
    WEAVIATE_DEFAULT_VECTOR_DIMENSIONS,
};
use serde_json::json;

#[test]
fn weaviate_collection_names_are_prefixed_and_validated() {
    assert!(validate_weaviate_collection_prefix(
        WEAVIATE_DEFAULT_COLLECTION_PREFIX
    ));
    assert!(validate_weaviate_collection_prefix("BillDev_01"));
    assert!(!validate_weaviate_collection_prefix(""));
    assert!(!validate_weaviate_collection_prefix("1Bill"));
    assert!(!validate_weaviate_collection_prefix("Bill-Dev"));

    let names = build_weaviate_collection_names("BillDev").expect("valid names");
    assert_eq!(
        names.name_for(WeaviateDerivedClass::ImportLearningSample),
        "BillDevImportLearningSample"
    );
    assert_eq!(
        names.name_for(WeaviateDerivedClass::CounterpartyFeature),
        "BillDevCounterpartyFeature"
    );
    assert!(build_weaviate_collection_names("bad-prefix").is_none());
}

#[test]
fn schema_classes_use_self_provided_vectors_and_required_metadata() {
    let classes = build_weaviate_schema_classes("BillDev").expect("schema");
    assert_eq!(classes.len(), 4);
    let sample = classes
        .iter()
        .find(|class| class["class"] == "BillDevImportLearningSample")
        .expect("sample class");

    assert_eq!(sample["vectorizer"], "none");
    let properties = sample["properties"].as_array().expect("properties");
    let property_names = properties
        .iter()
        .filter_map(|property| property["name"].as_str())
        .collect::<Vec<_>>();

    for required in [
        "userId",
        "featureSchemaVersion",
        "postgresSourceId",
        "recommendationKey",
        "featureKey",
        "payloadJson",
    ] {
        assert!(
            property_names.contains(&required),
            "schema should include {required}"
        );
    }
}

#[test]
fn object_id_and_feature_vectors_are_deterministic_and_schema_scoped() {
    let id = deterministic_weaviate_object_id("BillDevCounterpartyFeature", "feature:42");
    assert_eq!(id.len(), 36);
    assert_eq!(&id[14..15], "5");
    assert_eq!(
        id,
        deterministic_weaviate_object_id("BillDevCounterpartyFeature", "feature:42")
    );
    assert_ne!(
        id,
        deterministic_weaviate_object_id("BillDevDescriptionFeature", "feature:42")
    );

    let payload = json!({
        "counterparty": "coffee shop",
        "payment_method": "card",
    });
    let vector = derive_weaviate_feature_vector(&payload, WEAVIATE_DEFAULT_VECTOR_DIMENSIONS);
    assert_eq!(vector.len(), WEAVIATE_DEFAULT_VECTOR_DIMENSIONS);
    assert_eq!(
        vector,
        derive_weaviate_feature_vector(&payload, WEAVIATE_DEFAULT_VECTOR_DIMENSIONS)
    );
    assert!(vector.iter().all(|value| (-1.0..=1.0).contains(value)));
}

#[test]
fn batch_delete_and_graphql_payloads_preserve_user_metadata_filters() {
    let mut properties = build_weaviate_required_metadata(42, "feature:42");
    properties.insert("recommendationKey".to_string(), json!("rk-42"));
    properties.insert("featureKey".to_string(), json!("counterparty"));
    properties.insert("ruleState".to_string(), json!("green"));
    properties.insert(
        "payloadJson".to_string(),
        json!("{\"counterparty\":\"coffee\"}"),
    );

    let object = build_weaviate_derived_object(
        "BillDevCounterpartyFeature",
        "feature:42",
        properties,
        vec![0.1, -0.2, 0.3],
    );
    let batch = build_weaviate_batch_upsert_payload(std::slice::from_ref(&object));

    assert_eq!(batch["objects"][0]["class"], "BillDevCounterpartyFeature");
    assert_eq!(batch["objects"][0]["properties"]["userId"], 42);
    assert_eq!(
        batch["objects"][0]["properties"]["featureSchemaVersion"],
        FEATURE_SCHEMA_VERSION
    );
    assert_eq!(
        build_weaviate_delete_path(&object.class, &object.id),
        format!("/v1/objects/{}/{}", object.class, object.id)
    );

    let query = build_weaviate_graphql_query(
        "BillDevCounterpartyFeature",
        &[0.1, -0.2, 0.3],
        7,
        &[
            WeaviateMetadataFilter {
                path: "userId".to_string(),
                value: WeaviateFilterValue::Int(42),
            },
            WeaviateMetadataFilter {
                path: "featureSchemaVersion".to_string(),
                value: WeaviateFilterValue::Text(FEATURE_SCHEMA_VERSION.to_string()),
            },
            WeaviateMetadataFilter {
                path: "ruleState".to_string(),
                value: WeaviateFilterValue::Text("green".to_string()),
            },
        ],
    );
    let query_text = query["query"].as_str().expect("query text");
    assert!(query_text.contains("nearVector"));
    assert!(query_text.contains("limit: 7"));
    assert!(query_text.contains("operator: And"));
    assert!(query_text.contains("path: [\"userId\"]"));
    assert!(query_text.contains("valueInt: 42"));
    assert!(query_text.contains("_additional { id distance }"));
}
