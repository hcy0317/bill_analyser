#[test]
fn preview_page_result_builder_filters_sorts_and_pages_rows() {
    let mut first = preview_row(1);
    first.preview_selected = true;
    first.preview_amount_cents = 3000;
    first.preview_description = "保留 3".to_string();

    let mut second = preview_row(2);
    second.preview_selected = false;
    second.preview_amount_cents = 4000;
    second.preview_description = "过滤".to_string();

    let mut third = preview_row(3);
    third.preview_selected = true;
    third.preview_amount_cents = 1000;
    third.preview_description = "保留 1".to_string();

    let request = ImportPreviewPageRequest {
        page: 2,
        page_size: 1,
        sort_by: "sourceAmountCents".to_string(),
        sort_direction: "desc".to_string(),
        filters: ImportPreviewQueryFilters {
            selected_only: true,
            ..ImportPreviewQueryFilters::default()
        },
        ..ImportPreviewPageRequest::default()
    };

    let result = build_preview_page_result_from_rows(vec![third, second, first], &request);

    assert_eq!(result.total, 2);
    assert_eq!(result.page, 2);
    assert_eq!(result.page_size, 1);
    assert_eq!(
        result
            .rows
            .into_iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![3]
    );
    assert_eq!(result.metadata.counts.total, 2);
}

#[test]
fn bill_create_fields_from_preview_uses_category_id_as_category_authority() {
    let mut row = preview_row(1);
    row.category_id = Some(42);
    row.preview_main_category = "/".to_string();
    row.preview_sub_category = "民生银行储蓄卡(6332)".to_string();

    let fields = bill_create_fields_from_preview(&row);

    assert_eq!(fields.get("category_id"), Some(&json!(42)));
    assert_eq!(fields.get("main_category"), None);
    assert_eq!(fields.get("sub_category"), None);
}
