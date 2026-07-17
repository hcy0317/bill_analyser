use std::collections::BTreeMap;

use bill_analyser_core::{
    normalize_import_config_format, normalize_import_config_headers, normalize_import_config_name,
    suggest_import_config, ImportConfigDraft, ImportConfigDto, ImportConfigMatchDto,
    ImportConfigValidationError,
};
use serde_json::json;

#[test]
fn import_config_normalization_preserves_header_order() {
    assert_eq!(normalize_import_config_format(" CSV ").unwrap(), "csv");
    assert_eq!(normalize_import_config_format(" excel ").unwrap(), "excel");
    assert!(normalize_import_config_format("json").is_err());
    assert_eq!(
        normalize_import_config_name("  Monthly   Card  ").unwrap(),
        "monthly card"
    );
    assert_eq!(
        normalize_import_config_headers(&[" Amount ".to_string(), " Paid   At ".to_string()])
            .unwrap(),
        vec!["amount", "paid at"]
    );
    assert_eq!(
        normalize_import_config_headers(&["paid at".to_string(), "amount".to_string()]).unwrap(),
        vec!["paid at", "amount"],
        "same set in another order must remain distinguishable"
    );
    assert!(normalize_import_config_headers(&[]).is_err());
    assert!(normalize_import_config_headers(&["   ".to_string()]).is_err());
}

#[test]
fn suggestion_is_exact_deterministic_and_fully_shaped() {
    let headers = vec![
        "备注".to_string(),
        "金额".to_string(),
        "交易时间".to_string(),
        "收支类型".to_string(),
    ];
    let rows = vec![
        vec![
            "早餐".to_string(),
            "12.00".to_string(),
            "2026-07-14 08:00".to_string(),
            "支出".to_string(),
        ],
        vec![
            "工资".to_string(),
            "100.00".to_string(),
            "2026-07-14 09:00".to_string(),
            "收入".to_string(),
        ],
    ];

    let first = suggest_import_config(&headers, Some(&rows)).unwrap();
    let second = suggest_import_config(&headers, Some(&rows)).unwrap();
    assert_eq!(first, second);
    assert!(first.include_header);
    assert_eq!(first.column_mapping.get("1"), Some(&2));
    assert_eq!(first.column_mapping.get("3"), Some(&3));
    assert_eq!(first.column_mapping.get("8"), Some(&1));
    assert_eq!(first.column_mapping.get("14"), Some(&0));
    assert_eq!(first.transaction_type_mapping.get("支出"), Some(&3));
    assert_eq!(first.transaction_type_mapping.get("收入"), Some(&2));
    assert!(first
        .suggestions
        .windows(2)
        .all(|items| items[0].sort_key() <= items[1].sort_key()));
    assert!(first
        .suggestions
        .iter()
        .all(|item| item.column_index >= 0 && (0.0..=1.0).contains(&item.score)));

    let empty = suggest_import_config(&["unknown".to_string()], None).unwrap();
    assert_eq!(empty.column_mapping, BTreeMap::new());
    assert_eq!(empty.transaction_type_mapping, BTreeMap::new());
    assert!(empty.suggestions.is_empty());
}

#[test]
fn exact_dto_and_match_serialization_have_only_frozen_required_keys() {
    let dto = ImportConfigDto {
        id: 7,
        name: "Card".to_string(),
        file_format: "csv".to_string(),
        description: "Card mapping".to_string(),
        field_mappings: json!({}),
        sample_headers: vec!["time".to_string()],
        date_format: "%Y-%m-%d".to_string(),
        delimiter: None,
        encoding: "utf-8".to_string(),
        skip_rows: 0,
        has_header: true,
        custom_rules: json!({}),
        is_default: true,
        created_at: "2026-07-14T00:00:00Z".to_string(),
        updated_at: "2026-07-14T00:00:00Z".to_string(),
    };
    let value = serde_json::to_value(&dto).unwrap();
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            "createdAt",
            "customRules",
            "dateFormat",
            "delimiter",
            "description",
            "encoding",
            "fieldMappings",
            "fileFormat",
            "hasHeader",
            "id",
            "isDefault",
            "name",
            "sampleHeaders",
            "skipRows",
            "updatedAt",
        ]
    );
    assert!(value.get("delimiter").unwrap().is_null());

    let matched = serde_json::to_value(ImportConfigMatchDto::exact(dto)).unwrap();
    for key in [
        "descriptionSummary",
        "defaultRecommendation",
        "matchScore",
        "matchReason",
    ] {
        assert!(matched.get(key).is_some(), "missing match key {key}");
    }
    assert_eq!(matched.as_object().unwrap().len(), 19);
    assert_eq!(matched["defaultRecommendation"], false);
    assert_eq!(matched["matchReason"], "exact_headers");
}

#[test]
fn draft_validation_rejects_each_invalid_shape_and_normalizes_valid_input() {
    let draft = ImportConfigDraft {
        id: Some(7),
        name: "  Monthly   Card  ".to_string(),
        file_format: " CSV ".to_string(),
        description: "  card export  ".to_string(),
        field_mappings: json!({}),
        sample_headers: vec![" Paid   At ".to_string()],
        date_format: " %Y-%m-%d ".to_string(),
        delimiter: Some(" , ".to_string()),
        encoding: " UTF-8 ".to_string(),
        skip_rows: 0,
        has_header: true,
        custom_rules: json!({}),
        is_default: false,
    };

    let normalized = draft.validate_and_normalize().unwrap();
    assert_eq!(normalized.name, "Monthly Card");
    assert_eq!(normalized.normalized_name().unwrap(), "monthly card");
    assert_eq!(normalized.file_format, "csv");
    assert_eq!(normalized.sample_headers, ["Paid At"]);
    assert_eq!(normalized.delimiter.as_deref(), Some(","));
    assert_eq!(normalized.encoding, "utf-8");

    for (candidate, expected) in [
        (
            ImportConfigDraft {
                id: Some(0),
                ..draft.clone()
            },
            ImportConfigValidationError::InvalidId,
        ),
        (
            ImportConfigDraft {
                name: "   ".to_string(),
                ..draft.clone()
            },
            ImportConfigValidationError::InvalidName,
        ),
        (
            ImportConfigDraft {
                field_mappings: json!([]),
                ..draft.clone()
            },
            ImportConfigValidationError::InvalidFieldMappings,
        ),
        (
            ImportConfigDraft {
                custom_rules: json!([]),
                ..draft.clone()
            },
            ImportConfigValidationError::InvalidCustomRules,
        ),
        (
            ImportConfigDraft {
                skip_rows: -1,
                ..draft.clone()
            },
            ImportConfigValidationError::InvalidSkipRows,
        ),
        (
            ImportConfigDraft {
                encoding: "   ".to_string(),
                ..draft.clone()
            },
            ImportConfigValidationError::InvalidEncoding,
        ),
        (
            ImportConfigDraft {
                delimiter: Some("||".to_string()),
                ..draft.clone()
            },
            ImportConfigValidationError::InvalidDelimiter,
        ),
        (
            ImportConfigDraft {
                sample_headers: vec!["   ".to_string()],
                ..draft.clone()
            },
            ImportConfigValidationError::InvalidHeaders,
        ),
    ] {
        assert_eq!(candidate.validate_and_normalize().unwrap_err(), expected);
    }
}

#[test]
fn empty_match_description_falls_back_to_headers_and_dereferences_config() {
    let dto = ImportConfigDto {
        id: 8,
        name: "Fallback".to_string(),
        file_format: "csv".to_string(),
        description: "  ".to_string(),
        field_mappings: json!({}),
        sample_headers: vec!["date".to_string(), "amount".to_string()],
        date_format: String::new(),
        delimiter: Some(",".to_string()),
        encoding: "utf-8".to_string(),
        skip_rows: 0,
        has_header: true,
        custom_rules: json!({}),
        is_default: true,
        created_at: String::new(),
        updated_at: String::new(),
    };

    let matched = ImportConfigMatchDto::default_fallback(dto);
    assert_eq!(matched.description_summary, "date / amount");
    assert_eq!(matched.name, "Fallback");
}
