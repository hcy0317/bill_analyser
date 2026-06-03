use super::types::DbWriteInvariant;

pub(super) const IMPORT_DB_WRITE_INVARIANTS: [DbWriteInvariant; 8] = [
    DbWriteInvariant {
        key: "postgres_authority",
        description: "Business writes must target PostgreSQL authority.",
    },
    DbWriteInvariant {
        key: "referential_integrity",
        description: "PostgreSQL schema must enforce relational integrity for current-user data.",
    },
    DbWriteInvariant {
        key: "single_writer",
        description: "A business domain may have only one active DB writer during takeover.",
    },
    DbWriteInvariant {
        key: "transactional_staging",
        description: "Import session, preview, and confirmation mutations must be transactional.",
    },
    DbWriteInvariant {
        key: "rollback_on_error",
        description: "Failed import writes must roll back partial staging and bill mutations.",
    },
    DbWriteInvariant {
        key: "positive_user_scope",
        description: "All import writes must bind a positive authenticated user id.",
    },
    DbWriteInvariant {
        key: "amount_units",
        description: "Amount boundaries must state yuan or cents explicitly.",
    },
    DbWriteInvariant {
        key: "time_normalization",
        description: "Date/time writes must normalize local bill time explicitly.",
    },
];

pub(super) const CRUD_DB_WRITE_INVARIANTS: [DbWriteInvariant; 6] = [
    DbWriteInvariant {
        key: "postgres_authority",
        description: "Business writes must target PostgreSQL authority.",
    },
    DbWriteInvariant {
        key: "referential_integrity",
        description: "PostgreSQL schema must enforce relational integrity for current-user data.",
    },
    DbWriteInvariant {
        key: "rollback_on_error",
        description: "Failed runtime writes must roll back partial bill or budget mutations.",
    },
    DbWriteInvariant {
        key: "positive_user_scope",
        description: "All runtime writes must bind a positive authenticated user id.",
    },
    DbWriteInvariant {
        key: "amount_units",
        description: "Amount boundaries must state yuan or cents explicitly.",
    },
    DbWriteInvariant {
        key: "time_normalization",
        description: "Date/time writes must normalize local bill time explicitly.",
    },
];
