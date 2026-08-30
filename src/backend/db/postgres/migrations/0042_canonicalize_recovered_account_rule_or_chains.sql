UPDATE account_rules
SET rule_expression = jsonb_set(
        rule_expression,
        '{expression}',
        to_jsonb(
            regexp_replace(
                substring(
                    rule_expression->>'expression'
                    FROM 2
                    FOR char_length(rule_expression->>'expression') - 2
                ),
                '\}\)\|\(OR=\{',
                ',',
                'g'
            )
        ),
        false
    ),
    updated_at = now(),
    version = version + 1
WHERE source = 'sqlite_account_recovery'
  AND jsonb_typeof(rule_expression) = 'object'
  AND COALESCE(rule_expression->>'regex_enabled', 'false') = 'false'
  AND rule_expression->>'expression' LIKE '(OR={%})|(OR={%})'
  AND position('+' IN rule_expression->>'expression') = 0
  AND position('/' IN rule_expression->>'expression') = 0
  AND position('AND={' IN rule_expression->>'expression') = 0
  AND position('NOT={' IN rule_expression->>'expression') = 0
  AND position('REGEX={' IN rule_expression->>'expression') = 0;
