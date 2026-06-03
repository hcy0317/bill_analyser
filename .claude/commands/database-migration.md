# Database Migration

When changing schema or persistence behavior in Bill Analyser:

1. Review the PostgreSQL migrations under `src/backend/db/postgres/migrations/`.
2. Keep user isolation, explicit PostgreSQL transactions, and current repository-layer access intact.
3. Do not add historical datastore migration or recovery compatibility.
4. Add validation or regression tests for the changed path.
5. Document any API contract or data-shape impact.
