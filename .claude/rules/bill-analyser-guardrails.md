# Bill Analyser Guardrails

- Shared repository guidance is canonical in `AGENTS.md`; keep this Claude rule layer concise.
- Read the repository entry files before making changes: CLAUDE.md, AGENTS.md, and .github/copilot-instructions.md.
- Preserve the Rust REST runtime, PostgreSQL authority, and required Weaviate index boundary.
- Keep REST as the primary runtime API chain.
- Handle money units explicitly between yuan and cents.
- Do not use destructive process-kill shortcuts for local services.
