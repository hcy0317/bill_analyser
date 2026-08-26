WITH stale_sessions AS (
    SELECT user_id, id, session_key
    FROM (
        SELECT
            user_id,
            id,
            session_key,
            ROW_NUMBER() OVER (
                PARTITION BY user_id
                ORDER BY updated_at DESC, id DESC
            ) AS retention_rank
        FROM import_sessions
        WHERE status <> 'confirmed'
    ) ranked
    WHERE retention_rank > 2
), deleted_annotations AS (
    DELETE FROM import_annotation_samples annotations
    USING stale_sessions stale
    WHERE annotations.user_id = stale.user_id
      AND annotations.session_id = stale.session_key
    RETURNING annotations.id
)
DELETE FROM import_sessions sessions
USING stale_sessions stale
WHERE sessions.user_id = stale.user_id
  AND sessions.id = stale.id
  AND sessions.status <> 'confirmed';
