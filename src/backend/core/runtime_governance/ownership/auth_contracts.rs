use super::super::types::{EndpointOwnership, ResponseEnvelopeFamily, RuntimeState};

pub(super) const ROUTES: &[EndpointOwnership] = &[
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/login",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies username/email password login, handles lockout/inactive/2FA-pending branches, writes sessions and auth logs, and returns current API tokens, profile, and application cloud settings.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/register",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime validates registration input/password policy, creates the user, default categories/rules/accounts, and auth log transactionally, and returns the current API registration result.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/logout",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime invalidates the bearer session by token hash, records logout auth logs for active sessions, and keeps current API idempotent success.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/email/verify",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime validates verify_email action tokens, marks email_verified, optionally issues a new session token, writes email_verified auth logs, and returns result.user.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/email/resend-verification",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime verifies email/password credentials, issues mock-success verification tokens, records verification_email_resend_requested metadata, and returns result=true.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/password/forgot",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime honors the forget-password feature flag, returns success for unknown emails, and records mock-success reset token metadata for existing users.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/password/reset",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime validates reset_password action tokens, password policy, and email/user match before updating the password hash and writing password_reset_completed auth logs.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/oauth2/authorize",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime owns the OAuth2 callback authorize disabled-safe/not-implemented response contract; the current workspace build has no live provider exchange implementation or fallback remainder for this route.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/tokens",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime revokes all other token sessions while preserving current bearer session semantics.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/tokens",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime lists active token sessions with current API success/result envelope.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/tokens/{token_id}",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime revokes one token session by id with user-scope validation.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tokens/api",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies the current password and issues API personal access tokens with session and auth-log writes.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tokens/mcp",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies the current password and issues MCP personal access tokens with session and auth-log writes.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tokens/refresh",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies refresh token JWT/session state and issues rotated access/refresh sessions with profile/cloud settings response.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/profile",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime returns the authenticated user's current API profile payload.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/profile",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime updates whitelisted user profile/display/investment-keyword fields, validates scoped account/category references, resets email verification on email changes, and returns result.user.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/profile/avatar",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime accepts validated PNG/JPEG/GIF/WebP multipart avatar uploads, stores a data URL, and returns the updated profile.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/profile/avatar",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime clears the avatar field and returns the updated profile.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/profile/email/resend-verification",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime rate-limits and records the mock-success verification email resend auth log, then returns result=true.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/profile/cloud-settings",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime lists application cloud settings and preserves the empty false response.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/profile/cloud-settings",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime validates supported application cloud setting keys/types and upserts full or partial updates.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/profile/cloud-settings",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime deletes all authenticated-user cloud settings and returns result=true.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/profile/external-auths",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime lists user-scoped external-auth bindings, preserves createdAt millisecond projection, and appends the configured OAuth2 unlinked placeholder.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/profile/external-auths/unlink",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime verifies the current password before deleting a user-scoped external-auth binding and writing external_auth_unlinked audit metadata.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/system/version",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime serves the unauthenticated system version metadata route with the current payload shape.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/data/statistics",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth user-data runtime returns authenticated user-scoped bill/account/category/tag/template counts with the current API success/result envelope.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://import-v2-envelope-oracle",
        domain: "bills-import",
        state: RuntimeState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust core pins import envelope families for Rust-owned import_db_runtime routes.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://postgres-import-writer-policy",
        domain: "database-facade",
        state: RuntimeState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust core pins PostgreSQL writer invariants for import_db_runtime.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://postgres-foundational-schema-policy",
        domain: "database-schema",
        state: RuntimeState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust DB crate owns PostgreSQL foundational schema initialization and user-scoped constraints.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://postgres-repository-policy",
        domain: "database-repositories",
        state: RuntimeState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust DB crate records the active PostgreSQL repository surfaces for import staging, bills, budgets, statistics, app settings, and taxonomy.",
    },
];
