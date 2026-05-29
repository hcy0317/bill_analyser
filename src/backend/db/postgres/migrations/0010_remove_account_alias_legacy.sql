WITH metadata_aliases AS (
    SELECT
        accounts.id AS account_id,
        accounts.user_id,
        trim(alias_value) AS alias_value
    FROM accounts
    CROSS JOIN LATERAL (
        SELECT jsonb_array_elements_text(accounts.metadata->'aliases') AS alias_value
        WHERE jsonb_typeof(accounts.metadata->'aliases') = 'array'
        UNION ALL
        SELECT regexp_split_to_table(accounts.metadata->>'aliases', '[,;|\n]+') AS alias_value
        WHERE jsonb_typeof(accounts.metadata->'aliases') = 'string'
    ) aliases
    WHERE accounts.metadata ? 'aliases'
),
distinct_aliases AS (
    SELECT DISTINCT ON (user_id, account_id, lower(alias_value))
        user_id,
        account_id,
        alias_value
    FROM metadata_aliases
    WHERE alias_value <> ''
    ORDER BY user_id, account_id, lower(alias_value), alias_value
)
INSERT INTO account_rules (
    user_id,
    account_id,
    name,
    account_role_scope,
    transaction_type_scope,
    field_scope,
    rule_expression,
    regex_enabled,
    priority,
    enabled,
    source,
    source_key,
    match_count
)
SELECT
    user_id,
    account_id,
    'recovered account rule: ' || alias_value,
    'any',
    'all',
    '["counterparty", "payment_method", "description", "parser"]'::jsonb,
    jsonb_build_object(
        'operator', 'contains_any',
        'values', jsonb_build_array(alias_value),
        'source', 'account_metadata_alias_migration'
    ),
    FALSE,
    1000,
    TRUE,
    'account_metadata_alias_migration',
    'metadata:' || account_id || ':' || substr(md5(lower(alias_value)), 1, 16),
    0
FROM distinct_aliases
WHERE NOT EXISTS (
    SELECT 1
    FROM account_rules existing
    WHERE existing.user_id = distinct_aliases.user_id
      AND existing.account_id IS NOT DISTINCT FROM distinct_aliases.account_id
      AND existing.source = 'account_metadata_alias_migration'
      AND existing.source_key = 'metadata:' || distinct_aliases.account_id || ':' || substr(md5(lower(distinct_aliases.alias_value)), 1, 16)
);

UPDATE accounts
SET metadata = metadata - 'aliases',
    updated_at = now(),
    version = version + 1
WHERE metadata ? 'aliases';

DO $$
DECLARE
    legacy_alias_column TEXT;
    legacy_row_count BIGINT;
    invalid_row_count BIGINT;
BEGIN
    IF to_regclass('account_aliases_legacy') IS NULL THEN
        RETURN;
    END IF;

    EXECUTE 'SELECT COUNT(*)::BIGINT FROM account_aliases_legacy'
        INTO legacy_row_count;
    IF legacy_row_count = 0 THEN
        RETURN;
    END IF;

    SELECT column_name
    INTO legacy_alias_column
    FROM information_schema.columns
    WHERE table_schema = ANY (current_schemas(false))
      AND table_name = 'account_aliases_legacy'
      AND column_name IN ('alias', 'alias_value', 'value')
    ORDER BY CASE column_name
        WHEN 'alias' THEN 1
        WHEN 'alias_value' THEN 2
        ELSE 3
    END
    LIMIT 1;

    IF NOT EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = ANY (current_schemas(false))
          AND table_name = 'account_aliases_legacy'
          AND column_name = 'user_id'
    )
    OR NOT EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = ANY (current_schemas(false))
          AND table_name = 'account_aliases_legacy'
          AND column_name = 'account_id'
    )
    OR legacy_alias_column IS NULL THEN
        RAISE EXCEPTION
            'account_aliases_legacy is non-empty but does not expose user_id, account_id, and alias/value columns';
    END IF;

    EXECUTE format($SQL$
        SELECT COUNT(*)::BIGINT
        FROM account_aliases_legacy legacy
        LEFT JOIN accounts account
            ON account.id = legacy.account_id
           AND account.user_id = legacy.user_id
        WHERE trim(COALESCE(legacy.%1$I::TEXT, '')) <> ''
          AND account.id IS NULL
    $SQL$, legacy_alias_column)
    INTO invalid_row_count;

    IF invalid_row_count > 0 THEN
        RAISE EXCEPTION
            'account_aliases_legacy contains aliases whose user/account relationship cannot be verified';
    END IF;

    EXECUTE format($SQL$
        WITH legacy_aliases AS (
            SELECT DISTINCT ON (legacy.user_id, legacy.account_id, lower(trim(legacy.%1$I::TEXT)))
                legacy.user_id,
                legacy.account_id,
                trim(legacy.%1$I::TEXT) AS alias_value
            FROM account_aliases_legacy legacy
            JOIN accounts account
              ON account.id = legacy.account_id
             AND account.user_id = legacy.user_id
            WHERE trim(COALESCE(legacy.%1$I::TEXT, '')) <> ''
            ORDER BY legacy.user_id, legacy.account_id, lower(trim(legacy.%1$I::TEXT)), trim(legacy.%1$I::TEXT)
        )
        INSERT INTO account_rules (
            user_id,
            account_id,
            name,
            account_role_scope,
            transaction_type_scope,
            field_scope,
            rule_expression,
            regex_enabled,
            priority,
            enabled,
            source,
            source_key,
            match_count
        )
        SELECT
            user_id,
            account_id,
            'recovered account rule: ' || alias_value,
            'any',
            'all',
            '["counterparty", "payment_method", "description", "parser"]'::jsonb,
            jsonb_build_object(
                'operator', 'contains_any',
                'values', jsonb_build_array(alias_value),
                'source', 'account_alias_legacy_table_migration'
            ),
            FALSE,
            1000,
            TRUE,
            'account_alias_legacy_table_migration',
            'legacy-table:' || account_id || ':' || substr(md5(lower(alias_value)), 1, 16),
            0
        FROM legacy_aliases
        WHERE NOT EXISTS (
            SELECT 1
            FROM account_rules existing
            WHERE existing.user_id = legacy_aliases.user_id
              AND existing.account_id IS NOT DISTINCT FROM legacy_aliases.account_id
              AND existing.source = 'account_alias_legacy_table_migration'
              AND existing.source_key = 'legacy-table:' || legacy_aliases.account_id || ':' || substr(md5(lower(legacy_aliases.alias_value)), 1, 16)
        );
    $SQL$, legacy_alias_column);
END $$;

DROP INDEX IF EXISTS idx_account_rules_alias_source_unique;
CREATE UNIQUE INDEX IF NOT EXISTS idx_account_rules_recovery_source_unique
    ON account_rules (user_id, account_id, source, source_key)
    WHERE source_key IS NOT NULL;

DROP TABLE IF EXISTS account_aliases_legacy;
