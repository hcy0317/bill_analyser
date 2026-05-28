ALTER TABLE transaction_templates
    ALTER COLUMN source_amount_minor_units TYPE BIGINT
        USING ROUND(source_amount_minor_units)::BIGINT,
    ALTER COLUMN destination_amount_minor_units TYPE BIGINT
        USING ROUND(destination_amount_minor_units)::BIGINT;

COMMENT ON COLUMN transaction_templates.source_amount_minor_units IS 'frontend-compatible template amount value in exact minor units; migrated from legacy bill_templates.amount without implicit yuan/cent conversion';
COMMENT ON COLUMN transaction_templates.destination_amount_minor_units IS 'frontend-compatible destination amount value in exact minor units; migrated from legacy template destination_amount without implicit yuan/cent conversion';
