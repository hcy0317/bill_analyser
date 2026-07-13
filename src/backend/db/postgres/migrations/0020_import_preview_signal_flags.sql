CREATE OR REPLACE FUNCTION import_preview_signal_trim(value TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT btrim(
        COALESCE(value, ''),
        E' \t\n\v\f\r\u0085\u00A0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200A\u2028\u2029\u202F\u205F\u3000'
    )
$$;

CREATE OR REPLACE FUNCTION import_preview_feedback_status(section JSONB)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT COALESCE(
        NULLIF(lower(import_preview_signal_trim(section->>'review_status')), ''),
        NULLIF(lower(import_preview_signal_trim(section->>'status')), ''),
        NULLIF(lower(import_preview_signal_trim(section->>'lifecycle_status')), ''),
        NULLIF(lower(import_preview_signal_trim(section->>'signal_state')), ''),
        ''
    )
$$;

CREATE OR REPLACE FUNCTION import_preview_signal_truthy(value JSONB)
RETURNS BOOLEAN
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT CASE jsonb_typeof(value)
        WHEN 'boolean' THEN value = 'true'::jsonb
        WHEN 'number' THEN (value #>> '{}')::numeric <> 0
        WHEN 'string' THEN
            lower(import_preview_signal_trim(value #>> '{}')) IN ('true', '1', 'yes', 'y')
            OR (
                import_preview_signal_trim(value #>> '{}') ~ '^[+-]?([0-9]+([.][0-9]+)?|[.][0-9]+)$'
                AND import_preview_signal_trim(value #>> '{}') ~ '[1-9]'
            )
        ELSE false
    END
$$;

CREATE OR REPLACE FUNCTION import_preview_signal_positive_number(value JSONB)
RETURNS BOOLEAN
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT import_preview_signal_trim(value #>> '{}') ~ '^[+-]?([0-9]+([.][0-9]+)?|[.][0-9]+)$'
       AND import_preview_signal_trim(value #>> '{}') !~ '^-'
       AND import_preview_signal_trim(value #>> '{}') ~ '[1-9]'
$$;

CREATE OR REPLACE FUNCTION import_preview_signal_meaningful_text(value JSONB)
RETURNS BOOLEAN
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT jsonb_typeof(value) = 'string'
       AND import_preview_signal_trim(value #>> '{}') <> ''
       AND lower(import_preview_signal_trim(value #>> '{}')) NOT IN ('none', 'suppressed', 'null')
       AND (
           import_preview_signal_trim(value #>> '{}') !~ '^[+-]?([0-9]+([.][0-9]+)?|[.][0-9]+)$'
           OR import_preview_signal_positive_number(value)
       )
$$;

CREATE OR REPLACE FUNCTION import_preview_meaningful_feedback(section JSONB, family TEXT)
RETURNS BOOLEAN
LANGUAGE plpgsql
IMMUTABLE
PARALLEL SAFE
AS $$
DECLARE
    resolved_status TEXT;
    numeric_fields TEXT[];
    text_fields TEXT[];
    field_name TEXT;
    canonical_statuses TEXT[];
BEGIN
    IF jsonb_typeof(section) <> 'object'
       OR import_preview_signal_truthy(section->'suppressed') THEN
        RETURN false;
    END IF;
    resolved_status := import_preview_feedback_status(section);
    IF resolved_status IN ('none', 'suppressed') THEN
        RETURN false;
    END IF;
    IF family = 'learning' THEN
        canonical_statuses := ARRAY['pending', 'needs_review', 'none', 'suppressed', 'accepted', 'rejected', 'skipped', 'auto_applied', 'auto-applied'];
        numeric_fields := ARRAY['rule_id', 'score', 'confidence', 'margin', 'accepted_count', 'rejected_count', 'auto_applied_count'];
        text_fields := ARRAY['reason', 'recommended_type', 'summary', 'source', 'mode', 'model_version', 'recommendation_key'];
    ELSIF family = 'llm' THEN
        canonical_statuses := ARRAY['pending', 'none', 'suppressed', 'accepted', 'rejected', 'skipped', 'auto_applied', 'auto-applied'];
        numeric_fields := ARRAY['confidence', 'suggested_category_id'];
        text_fields := ARRAY['suggested_type', 'suggested_main_category', 'suggested_sub_category', 'suggested_source_account', 'suggested_destination_account', 'reason'];
    ELSE
        RETURN false;
    END IF;
    IF resolved_status <> '' AND NOT (resolved_status = ANY(canonical_statuses)) THEN
        RETURN false;
    END IF;
    IF resolved_status = ANY(ARRAY['accepted', 'rejected', 'skipped', 'auto_applied', 'auto-applied']) THEN
        RETURN true;
    END IF;
    IF resolved_status <> '' AND NOT (resolved_status = ANY(ARRAY['pending', 'none', 'suppressed', 'accepted', 'rejected', 'skipped', 'auto_applied', 'auto-applied'])) THEN
        RETURN true;
    END IF;
    FOREACH field_name IN ARRAY numeric_fields LOOP
        IF import_preview_signal_positive_number(section->field_name) THEN
            RETURN true;
        END IF;
    END LOOP;
    FOREACH field_name IN ARRAY text_fields LOOP
        IF import_preview_signal_meaningful_text(section->field_name) THEN
            RETURN true;
        END IF;
    END LOOP;
    RETURN false;
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
    history_signal := import_preview_signal_trim(reconciliation_section->>'planned_operation') = ANY(
            ARRAY['update_history', 'merge_transfer_history', 'update_current_bill', 'merge_current_bill_transfer', 'merge_transfer']
        )
        OR import_preview_signal_truthy(reconciliation_section->'destructive_ack_required');
    learning_signal := import_preview_meaningful_feedback(feedback->'learning', 'learning')
        OR (
            jsonb_typeof(transfer_section) = 'object'
            AND NOT import_preview_signal_truthy(transfer_section->'suppressed')
            AND jsonb_typeof(transfer_section->'candidate_type') = 'string'
            AND import_preview_signal_trim(transfer_section->>'candidate_type') <> ''
            AND jsonb_typeof(transfer_section->'learning_level') = 'string'
            AND import_preview_signal_trim(transfer_section->>'learning_level') = ANY(ARRAY['yellow', 'green', 'blue'])
            AND transfer_status <> ''
            AND transfer_status NOT IN ('none', 'suppressed')
        );
    llm_signal := import_preview_meaningful_feedback(feedback->'llm', 'llm');
    parser_source := import_preview_signal_trim(payload->>'preview_parser_id') <> ''
        OR EXISTS (
            SELECT 1
            FROM jsonb_array_elements_text(
                CASE WHEN jsonb_typeof(payload->'preview_parser_tags') = 'array'
                     THEN payload->'preview_parser_tags' ELSE '[]'::jsonb END
            ) parser_tag(value)
            WHERE import_preview_signal_trim(parser_tag.value) <> ''
        )
        OR feedback ? 'parser';
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
