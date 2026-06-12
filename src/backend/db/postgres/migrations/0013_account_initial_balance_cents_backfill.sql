-- Backfill legacy yuan-valued JSON metadata into explicit integer cents fields.
-- New code reads and writes *_cents fields only; this migration preserves
-- existing restored accounts, transfer/investment bills, and pending previews
-- without keeping yuan semantics in API DTOs.

UPDATE accounts
SET metadata = jsonb_set(
    COALESCE(metadata, '{}'::jsonb),
    '{initial_balance_cents}',
    to_jsonb(ROUND((metadata->>'initial_balance')::numeric * 100)::bigint),
    true
)
WHERE metadata ? 'initial_balance'
  AND NOT (metadata ? 'initial_balance_cents')
  AND jsonb_typeof(metadata->'initial_balance') IN ('number', 'string')
  AND (metadata->>'initial_balance') ~ '^[+-]?[0-9]+(\.[0-9]+)?$';

UPDATE bills
SET standard_payload = jsonb_set(
    COALESCE(standard_payload, '{}'::jsonb),
    '{destination_amount_cents}',
    to_jsonb(ROUND((COALESCE(
        NULLIF(standard_payload->>'destination_amount', ''),
        NULLIF(standard_payload->>'destinationAmount', '')
    ))::numeric * 100)::bigint),
    true
)
WHERE NOT (standard_payload ? 'destination_amount_cents')
  AND NOT (standard_payload ? 'destinationAmountCents')
  AND COALESCE(
        NULLIF(standard_payload->>'destination_amount', ''),
        NULLIF(standard_payload->>'destinationAmount', '')
      ) ~ '^[+-]?[0-9]+(\.[0-9]+)?$';

UPDATE import_preview_rows
SET preview_payload = jsonb_set(
    COALESCE(preview_payload, '{}'::jsonb),
    '{preview_destination_amount_cents}',
    to_jsonb((COALESCE(
        NULLIF(preview_payload->>'destination_amount_cents', ''),
        NULLIF(preview_payload->>'destinationAmountCents', '')
    ))::bigint),
    true
)
WHERE NOT (preview_payload ? 'preview_destination_amount_cents')
  AND COALESCE(
        NULLIF(preview_payload->>'destination_amount_cents', ''),
        NULLIF(preview_payload->>'destinationAmountCents', '')
      ) ~ '^[+-]?[0-9]+$';

UPDATE import_preview_rows
SET preview_payload = jsonb_set(
    COALESCE(preview_payload, '{}'::jsonb),
    '{preview_destination_amount_cents}',
    to_jsonb(ROUND((COALESCE(
        NULLIF(preview_payload->>'preview_destination_amount', ''),
        NULLIF(preview_payload->>'destination_amount', ''),
        NULLIF(preview_payload->>'destinationAmount', '')
    ))::numeric * 100)::bigint),
    true
)
WHERE NOT (preview_payload ? 'preview_destination_amount_cents')
  AND COALESCE(
        NULLIF(preview_payload->>'preview_destination_amount', ''),
        NULLIF(preview_payload->>'destination_amount', ''),
        NULLIF(preview_payload->>'destinationAmount', '')
      ) ~ '^[+-]?[0-9]+(\.[0-9]+)?$';
