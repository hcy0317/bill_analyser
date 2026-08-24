CREATE OR REPLACE FUNCTION import_preview_parser_evidence(payload JSONB)
RETURNS BOOLEAN
LANGUAGE plpgsql
IMMUTABLE
PARALLEL SAFE
AS $$
DECLARE
    feedback JSONB := CASE WHEN jsonb_typeof(payload->'preview_matching_feedback') = 'object'
                           THEN payload->'preview_matching_feedback' ELSE '{}'::jsonb END;
    parser_section JSONB := feedback->'parser';
BEGIN
    RETURN import_preview_signal_trim(payload->>'preview_parser_id') <> ''
        OR EXISTS (
            SELECT 1
            FROM jsonb_array_elements_text(
                CASE WHEN jsonb_typeof(payload->'preview_parser_tags') = 'array'
                     THEN payload->'preview_parser_tags' ELSE '[]'::jsonb END
            ) parser_tag(value)
            WHERE import_preview_signal_trim(parser_tag.value) <> ''
        )
        OR (
            jsonb_typeof(parser_section) = 'object'
            AND (
                import_preview_signal_trim(parser_section->>'id') <> ''
                OR import_preview_signal_trim(parser_section->>'parser_id') <> ''
                OR EXISTS (
                    SELECT 1
                    FROM jsonb_array_elements_text(
                        CASE WHEN jsonb_typeof(parser_section->'tags') = 'array'
                             THEN parser_section->'tags' ELSE '[]'::jsonb END
                    ) parser_tag(value)
                    WHERE import_preview_signal_trim(parser_tag.value) <> ''
                )
                OR EXISTS (
                    SELECT 1
                    FROM jsonb_array_elements_text(
                        CASE WHEN jsonb_typeof(parser_section->'parser_tags') = 'array'
                             THEN parser_section->'parser_tags' ELSE '[]'::jsonb END
                    ) parser_tag(value)
                    WHERE import_preview_signal_trim(parser_tag.value) <> ''
                )
            )
        );
END
$$;

CREATE OR REPLACE FUNCTION import_preview_signal_flags(payload JSONB)
RETURNS JSONB
LANGUAGE plpgsql
IMMUTABLE
PARALLEL SAFE
AS $$
DECLARE
    feedback JSONB := CASE WHEN jsonb_typeof(payload->'preview_matching_feedback') = 'object'
                           THEN payload->'preview_matching_feedback' ELSE '{}'::jsonb END;
    transfer_section JSONB := feedback->'transfer';
    reconciliation_section JSONB := feedback->'reconciliation';
    transfer_status TEXT := import_preview_feedback_status(transfer_section);
    reconciliation_status TEXT := import_preview_feedback_status(reconciliation_section);
    canonical_statuses TEXT[] := ARRAY[
        'pending', 'none', 'suppressed', 'accepted', 'rejected', 'skipped',
        'auto_applied', 'auto-applied'
    ];
    parser_source BOOLEAN;
    platform_duplicate BOOLEAN;
    transfer_signal BOOLEAN;
    history_signal BOOLEAN;
    learning_signal BOOLEAN;
    llm_signal BOOLEAN;
BEGIN
    platform_duplicate := lower(import_preview_signal_trim(COALESCE(
        NULLIF(import_preview_signal_trim(payload->>'dedup_type'), ''),
        feedback#>>'{dedup,type}'
    ))) = 'platform_bank';
    transfer_signal := jsonb_typeof(transfer_section) = 'object'
        AND NOT import_preview_signal_truthy(transfer_section->'suppressed')
        AND transfer_status NOT IN ('none', 'suppressed')
        AND (
            transfer_status = ANY(ARRAY['pending', 'accepted', 'auto_applied', 'auto-applied'])
            OR (
                transfer_status = ''
                AND lower(import_preview_signal_trim(payload->>'preview_type')) NOT IN ('转账', 'transfer', '4')
                AND (
                    (jsonb_typeof(transfer_section->'candidate_type') = 'string' AND import_preview_signal_trim(transfer_section->>'candidate_type') <> '')
                    OR (jsonb_typeof(transfer_section->'reason') = 'string' AND import_preview_signal_trim(transfer_section->>'reason') <> '')
                    OR import_preview_signal_positive_number(transfer_section->'score')
                )
            )
        );
    history_signal := (reconciliation_status = '' OR reconciliation_status = ANY(canonical_statuses))
        AND (
            import_preview_signal_trim(reconciliation_section->>'planned_operation') = ANY(
                ARRAY['update_history', 'merge_transfer_history', 'update_current_bill', 'merge_current_bill_transfer', 'merge_transfer']
            )
            OR import_preview_signal_truthy(reconciliation_section->'destructive_ack_required')
        );
    learning_signal := import_preview_meaningful_feedback(feedback->'learning', 'learning')
        OR (
            jsonb_typeof(transfer_section) = 'object'
            AND NOT import_preview_signal_truthy(transfer_section->'suppressed')
            AND jsonb_typeof(transfer_section->'candidate_type') = 'string'
            AND import_preview_signal_trim(transfer_section->>'candidate_type') <> ''
            AND jsonb_typeof(transfer_section->'learning_level') = 'string'
            AND import_preview_signal_trim(transfer_section->>'learning_level') = ANY(ARRAY['yellow', 'green', 'blue'])
            AND transfer_status = ANY(canonical_statuses)
            AND transfer_status NOT IN ('none', 'suppressed')
        );
    llm_signal := import_preview_meaningful_feedback(feedback->'llm', 'llm');
    parser_source := import_preview_parser_evidence(payload);
    RETURN jsonb_build_object(
        'parser', parser_source AND NOT (platform_duplicate OR transfer_signal OR history_signal OR learning_signal OR llm_signal),
        'platform_duplicate', platform_duplicate,
        'transfer', transfer_signal,
        'history', history_signal,
        'learning', learning_signal,
        'llm', llm_signal
    );
END
$$;
