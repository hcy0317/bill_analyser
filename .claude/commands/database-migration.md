# Database Migration

When changing schema or persistence behavior in Bill Analyser:

1. Review `src/core/db.py` and existing migration-style patterns in the repository.
2. Keep user isolation, WAL behavior, and async access intact.
3. Preserve backward compatibility where possible.
4. Add validation or regression tests for the changed path.
5. Document any API contract or data-shape impact.