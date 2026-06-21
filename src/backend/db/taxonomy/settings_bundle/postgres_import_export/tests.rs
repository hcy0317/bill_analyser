#[cfg(test)]
mod postgres_settings_template_import_tests {
    use super::*;

    fn ref_maps() -> (
        BTreeMap<String, i64>,
        BTreeMap<String, i64>,
        BTreeMap<String, i64>,
    ) {
        let account_ref_map = BTreeMap::from([
            ("account:1".to_string(), 11),
            ("accountName:现金".to_string(), 22),
        ]);
        let category_ref_map = BTreeMap::from([("category:3".to_string(), 33)]);
        let tag_ref_map = BTreeMap::from([("tag:7".to_string(), 77)]);
        (account_ref_map, category_ref_map, tag_ref_map)
    }

    #[test]
    fn settings_template_values_resolve_refs_and_preserve_minor_units() {
        let (account_ref_map, category_ref_map, tag_ref_map) = ref_maps();
        let mut section = SectionCounts::default();
        let mut warnings = Vec::new();

        let values = settings_template_values_from_item(
            &json!({
                "name": "午餐模板",
                "type": 3,
                "categoryRef": "category:3",
                "sourceAccountRef": "account:1",
                "destinationAccountName": "现金",
                "sourceAmountCents": 1235,
                "destinationAmountCents": 88,
                "hideAmount": true,
                "tagRefs": ["tag:7"],
                "comment": "常用午餐",
                "displayOrder": 9,
                "hidden": true,
                "utcOffset": 480
            }),
            1,
            5,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        )
        .expect("valid template values");

        assert_eq!(values.template_type, 1);
        assert_eq!(values.category_id.as_deref(), Some("33"));
        assert_eq!(values.source_account_id, "11");
        assert_eq!(values.destination_account_id, "22");
        assert_eq!(values.source_amount_minor_units, 1235);
        assert_eq!(values.destination_amount_minor_units, 88);
        assert_eq!(values.tag_ids, json!(["77"]));
        assert_eq!(values.display_order, 9);
        assert!(values.hide_amount);
        assert!(values.hidden);
        assert!(warnings.is_empty());
        assert_eq!(section.skipped, 0);
    }

    #[test]
    fn settings_scheduled_template_values_force_section_type_and_next_date() {
        let (account_ref_map, category_ref_map, tag_ref_map) = ref_maps();
        let mut section = SectionCounts::default();
        let mut warnings = Vec::new();

        let values = settings_template_values_from_item(
            &json!({
                "templateType": 1,
                "name": "房租计划",
                "sourceAccountRef": "account:1",
                "sourceAmountCents": 250000,
                "scheduledFrequencyType": 3,
                "scheduledFrequency": "1",
                "scheduledStartDate": "2026-06-01",
                "enabled": false,
                "autoCreate": true
            }),
            2,
            12,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        )
        .expect("valid scheduled template values");

        assert_eq!(values.template_type, 2);
        assert_eq!(values.source_amount_minor_units, 250000);
        assert_eq!(values.scheduled_frequency_type, Some(3));
        assert_eq!(values.scheduled_frequency.as_deref(), Some("1"));
        assert_eq!(values.scheduled_start_date.as_deref(), Some("2026-06-01"));
        assert_eq!(values.scheduled_next_date.as_deref(), Some("2026-06-01"));
        assert!(!values.enabled);
        assert!(values.auto_create);
        assert_eq!(values.display_order, 12);
        assert!(warnings.is_empty());
    }

    #[test]
    fn settings_template_values_skip_unresolved_refs_without_unsupported_warning() {
        let (account_ref_map, category_ref_map, tag_ref_map) = ref_maps();
        let mut section = SectionCounts::default();
        let mut warnings = Vec::new();

        let values = settings_template_values_from_item(
            &json!({
                "name": "缺失分类",
                "categoryRef": "category:missing",
                "sourceAccountRef": "account:1",
                "sourceAmountCents": 1999
            }),
            1,
            1,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        );

        assert!(values.is_none());
        assert_eq!(section.skipped, 1);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("unresolved category reference")));
        assert!(warnings
            .iter()
            .all(|warning| !warning.contains("not supported")));
    }

    #[test]
    fn settings_template_values_skip_invalid_explicit_minor_units() {
        let (account_ref_map, category_ref_map, tag_ref_map) = ref_maps();
        let mut section = SectionCounts::default();
        let mut warnings = Vec::new();

        let values = settings_template_values_from_item(
            &json!({
                "name": "坏金额模板",
                "categoryRef": "category:3",
                "sourceAccountRef": "account:1",
                "sourceAmountCents": true,
                "destinationAmountCents": "12.34"
            }),
            1,
            1,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        );

        assert!(values.is_none());
        assert_eq!(section.skipped, 1);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("invalid sourceAmountCents")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("invalid destinationAmountCents")));
    }

    #[test]
    fn exported_template_sections_build_importable_roundtrip_values() {
        let sections = export_taxonomy_sections(&json!({
            "accounts": [{
                "id": 1,
                "name": "现金",
                "type": 1,
                "currency": "CNY"
            }],
            "categories": [{
                "id": 3,
                "type": 3,
                "main_category": "餐饮",
                "sub_category": "午餐"
            }],
            "tags": [{
                "id": 7,
                "name": "项目"
            }],
            "templates": [{
                "id": "8",
                "templateType": 1,
                "name": "午餐模板",
                "type": 3,
                "categoryId": "3",
                "sourceAccountId": "1",
                "destinationAccountId": "0",
                "sourceAmountCents": 1234,
                "destinationAmountCents": 0,
                "hideAmount": false,
                "tagIds": ["7"],
                "comment": "工作日午餐",
                "displayOrder": 4
            }],
            "scheduled": [{
                "id": "9",
                "templateType": 2,
                "name": "房租计划",
                "type": 3,
                "categoryId": "3",
                "sourceAccountId": "1",
                "destinationAccountId": "0",
                "sourceAmountCents": 250000,
                "destinationAmountCents": 0,
                "hideAmount": false,
                "tagIds": ["7"],
                "scheduledFrequencyType": 3,
                "scheduledFrequency": "1",
                "scheduledStartDate": "2026-06-01",
                "displayOrder": 5
            }]
        }))
        .expect("taxonomy sections export");
        let template = &sections["transactionTemplates"][0];
        let scheduled = &sections["scheduledTransactions"][0];
        assert_eq!(template["sourceAccountRef"], "account:1");
        assert_eq!(template["categoryRef"], "category:3");
        assert_eq!(template["tagRefs"], json!(["tag:7"]));
        assert!(template.get("id").is_none());
        assert!(template.get("tagIds").is_none());

        let account_ref_map = BTreeMap::from([("account:1".to_string(), 101)]);
        let category_ref_map = BTreeMap::from([("category:3".to_string(), 303)]);
        let tag_ref_map = BTreeMap::from([("tag:7".to_string(), 707)]);
        let mut section = SectionCounts::default();
        let mut warnings = Vec::new();

        let template_values = settings_template_values_from_item(
            template,
            1,
            1,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        )
        .expect("exported transaction template values");
        let scheduled_values = settings_template_values_from_item(
            scheduled,
            2,
            2,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        )
        .expect("exported scheduled template values");

        assert_eq!(template_values.source_account_id, "101");
        assert_eq!(template_values.category_id.as_deref(), Some("303"));
        assert_eq!(template_values.source_amount_minor_units, 1234);
        assert_eq!(template_values.tag_ids, json!(["707"]));
        assert_eq!(scheduled_values.template_type, 2);
        assert_eq!(scheduled_values.source_amount_minor_units, 250000);
        assert_eq!(
            scheduled_values.scheduled_start_date.as_deref(),
            Some("2026-06-01")
        );
        assert!(warnings.is_empty());
        assert_eq!(section.skipped, 0);
    }

    #[test]
    fn settings_template_minor_units_are_strict_integers() {
        assert_eq!(settings_strict_minor_units(Some(&json!(1999))), 1999);
        assert_eq!(settings_strict_minor_units(Some(&json!("1999"))), 1999);
        assert_eq!(settings_strict_minor_units(Some(&json!("18.5"))), 0);
        assert_eq!(settings_strict_minor_units(Some(&json!(true))), 0);
    }
}
