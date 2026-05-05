# Rust Backend Migration Inventory

This S0 baseline is generated from a deterministic filesystem scan.
It does not change runtime behavior and does not mark any Python business file as dead.

## Summary

- Python backend files: 314
- Rust backend files: 32
- Migration domains: 22
- Files marked port: 251
- Files marked facade: 63
- Files marked deferred: 0
- Verified dead files: 0

## Domain Counts

| Domain | Port | Facade | Deferred |
| --- | ---: | ---: | ---: |
| accounts | 6 | 2 | 0 |
| ai-learning-llm | 24 | 6 | 0 |
| ai-ocr | 4 | 1 | 0 |
| api-contract-adapters | 0 | 4 | 0 |
| api-runtime-shell | 3 | 8 | 0 |
| auth-security | 19 | 2 | 0 |
| backup-operations | 5 | 1 | 0 |
| bills-import | 53 | 7 | 0 |
| budgets | 16 | 6 | 0 |
| classification-rules | 16 | 2 | 0 |
| database-facade | 3 | 2 | 0 |
| database-schema | 11 | 3 | 0 |
| import-contracts | 0 | 2 | 0 |
| import-parsers | 8 | 2 | 0 |
| matching-reconciliation | 22 | 5 | 0 |
| recurring-calendar | 4 | 0 | 0 |
| settings-bundle | 8 | 1 | 0 |
| shared-primitives | 5 | 3 | 0 |
| smart-dedup | 9 | 1 | 0 |
| statistics-reporting | 26 | 4 | 0 |
| sync-runtime | 1 | 0 | 0 |
| tags-templates | 8 | 1 | 0 |

## Python File Matrix

| Path | Migration domain | Role | Initial status | Lines |
| --- | --- | --- | --- | ---: |
| src/bill_analyser/__init__.py | shared-primitives | package-marker | facade | 11 |
| src/bill_analyser/api/__init__.py | api-runtime-shell | package-marker | facade | 3 |
| src/bill_analyser/api/adapters/__init__.py | api-contract-adapters | package-marker | facade | 10 |
| src/bill_analyser/api/adapters/account_adapter.py | api-contract-adapters | api-adapter | facade | 125 |
| src/bill_analyser/api/adapters/category_adapter.py | api-contract-adapters | api-adapter | facade | 85 |
| src/bill_analyser/api/adapters/transaction_adapter.py | api-contract-adapters | api-adapter | facade | 242 |
| src/bill_analyser/api/app.py | api-runtime-shell | api-shell | facade | 363 |
| src/bill_analyser/api/config/__init__.py | api-runtime-shell | package-marker | facade | 1 |
| src/bill_analyser/api/config/auth.py | api-runtime-shell | api-config | facade | 148 |
| src/bill_analyser/api/config/bills.py | api-runtime-shell | api-config | facade | 132 |
| src/bill_analyser/api/middleware/__init__.py | api-runtime-shell | package-marker | facade | 1 |
| src/bill_analyser/api/middleware/auth.py | api-runtime-shell | api-middleware | facade | 265 |
| src/bill_analyser/api/routes/__init__.py | api-runtime-shell | package-marker | facade | 3 |
| src/bill_analyser/api/routes/accounts/__init__.py | accounts | package-marker | facade | 34 |
| src/bill_analyser/api/routes/accounts/crud.py | accounts | api-route | port | 298 |
| src/bill_analyser/api/routes/accounts/support.py | accounts | api-route | port | 56 |
| src/bill_analyser/api/routes/accounts/transactions.py | accounts | api-route | port | 219 |
| src/bill_analyser/api/routes/auth/__init__.py | auth-security | package-marker | facade | 67 |
| src/bill_analyser/api/routes/auth/account_recovery.py | auth-security | api-route | port | 306 |
| src/bill_analyser/api/routes/auth/cloud_settings.py | auth-security | api-route | port | 135 |
| src/bill_analyser/api/routes/auth/external_auth.py | auth-security | api-route | port | 167 |
| src/bill_analyser/api/routes/auth/profile.py | auth-security | api-route | port | 355 |
| src/bill_analyser/api/routes/auth/registration.py | auth-security | api-route | port | 324 |
| src/bill_analyser/api/routes/auth/session.py | auth-security | api-route | port | 369 |
| src/bill_analyser/api/routes/auth/step_up.py | auth-security | api-route | port | 97 |
| src/bill_analyser/api/routes/auth/support.py | auth-security | api-route | port | 335 |
| src/bill_analyser/api/routes/auth/system.py | auth-security | api-route | port | 15 |
| src/bill_analyser/api/routes/auth/tokens.py | auth-security | api-route | port | 421 |
| src/bill_analyser/api/routes/auth/two_factor.py | auth-security | api-route | port | 279 |
| src/bill_analyser/api/routes/auth/two_factor_login.py | auth-security | api-route | port | 174 |
| src/bill_analyser/api/routes/auth/two_factor_support.py | auth-security | api-route | port | 106 |
| src/bill_analyser/api/routes/auth/user_data.py | auth-security | api-route | port | 361 |
| src/bill_analyser/api/routes/backup/__init__.py | backup-operations | package-marker | facade | 37 |
| src/bill_analyser/api/routes/backup/cleanup.py | backup-operations | api-route | port | 150 |
| src/bill_analyser/api/routes/backup/files.py | backup-operations | api-route | port | 320 |
| src/bill_analyser/api/routes/backup/jobs.py | backup-operations | api-route | port | 93 |
| src/bill_analyser/api/routes/backup/support.py | backup-operations | api-route | port | 364 |
| src/bill_analyser/api/routes/bills/__init__.py | bills-import | package-marker | facade | 76 |
| src/bill_analyser/api/routes/bills/category_actions.py | bills-import | api-route | port | 212 |
| src/bill_analyser/api/routes/bills/crud_create_update.py | bills-import | api-route | port | 309 |
| src/bill_analyser/api/routes/bills/crud_prepare.py | bills-import | api-route | port | 297 |
| src/bill_analyser/api/routes/bills/crud_query.py | bills-import | api-route | port | 339 |
| src/bill_analyser/api/routes/bills/import_config.py | bills-import | api-route | port | 339 |
| src/bill_analyser/api/routes/bills/import_detection.py | bills-import | api-route | port | 284 |
| src/bill_analyser/api/routes/bills/import_learning.py | bills-import | api-route | port | 410 |
| src/bill_analyser/api/routes/bills/import_legacy.py | bills-import | api-route | port | 285 |
| src/bill_analyser/api/routes/bills/import_mapping.py | bills-import | api-route | port | 322 |
| src/bill_analyser/api/routes/bills/import_review.py | bills-import | api-route | port | 394 |
| src/bill_analyser/api/routes/bills/import_rows.py | bills-import | api-route | port | 316 |
| src/bill_analyser/api/routes/bills/parse_import.py | bills-import | api-route | port | 198 |
| src/bill_analyser/api/routes/bills/reconciliation.py | bills-import | api-route | port | 253 |
| src/bill_analyser/api/routes/bills/support.py | bills-import | api-route | port | 322 |
| src/bill_analyser/api/routes/bills/v2_pipeline.py | bills-import | api-route | port | 416 |
| src/bill_analyser/api/routes/bills/v2_preview_actions.py | bills-import | api-route | port | 316 |
| src/bill_analyser/api/routes/bills/v2_sessions.py | bills-import | api-route | port | 194 |
| src/bill_analyser/api/routes/budgets/__init__.py | budgets | package-marker | facade | 40 |
| src/bill_analyser/api/routes/budgets/crud.py | budgets | api-route | port | 229 |
| src/bill_analyser/api/routes/budgets/execution.py | budgets | api-route | port | 193 |
| src/bill_analyser/api/routes/budgets/history.py | budgets | api-route | port | 113 |
| src/bill_analyser/api/routes/budgets/io.py | budgets | api-route | port | 89 |
| src/bill_analyser/api/routes/budgets/support.py | budgets | api-route | port | 305 |
| src/bill_analyser/api/routes/calendar.py | recurring-calendar | api-route | port | 167 |
| src/bill_analyser/api/routes/categories/__init__.py | classification-rules | package-marker | facade | 46 |
| src/bill_analyser/api/routes/categories/analytics.py | classification-rules | api-route | port | 51 |
| src/bill_analyser/api/routes/categories/collection.py | classification-rules | api-route | port | 302 |
| src/bill_analyser/api/routes/categories/io.py | classification-rules | api-route | port | 155 |
| src/bill_analyser/api/routes/categories/lists.py | classification-rules | api-route | port | 58 |
| src/bill_analyser/api/routes/categories/mutations.py | classification-rules | api-route | port | 419 |
| src/bill_analyser/api/routes/categories/rules.py | classification-rules | api-route | port | 84 |
| src/bill_analyser/api/routes/categories/support.py | classification-rules | api-route | port | 95 |
| src/bill_analyser/api/routes/category_rules.py | classification-rules | api-route | port | 277 |
| src/bill_analyser/api/routes/encryption.py | auth-security | api-route | port | 24 |
| src/bill_analyser/api/routes/insights.py | statistics-reporting | api-route | port | 178 |
| src/bill_analyser/api/routes/learning.py | ai-learning-llm | api-route | port | 256 |
| src/bill_analyser/api/routes/llm/__init__.py | ai-learning-llm | package-marker | facade | 40 |
| src/bill_analyser/api/routes/llm/analysis.py | ai-learning-llm | api-route | port | 137 |
| src/bill_analyser/api/routes/llm/candidates.py | ai-learning-llm | api-route | port | 97 |
| src/bill_analyser/api/routes/llm/configs.py | ai-learning-llm | api-route | port | 174 |
| src/bill_analyser/api/routes/llm/preview.py | ai-learning-llm | api-route | port | 178 |
| src/bill_analyser/api/routes/llm/support.py | ai-learning-llm | api-route | port | 225 |
| src/bill_analyser/api/routes/matching/__init__.py | matching-reconciliation | package-marker | facade | 37 |
| src/bill_analyser/api/routes/matching/actions.py | matching-reconciliation | api-route | port | 161 |
| src/bill_analyser/api/routes/matching/pairs.py | matching-reconciliation | api-route | port | 114 |
| src/bill_analyser/api/routes/matching/queries.py | matching-reconciliation | api-route | port | 152 |
| src/bill_analyser/api/routes/matching/support.py | matching-reconciliation | api-route | port | 366 |
| src/bill_analyser/api/routes/networth.py | statistics-reporting | api-route | port | 86 |
| src/bill_analyser/api/routes/receipt_ocr.py | ai-ocr | api-route | port | 185 |
| src/bill_analyser/api/routes/recurring.py | recurring-calendar | api-route | port | 210 |
| src/bill_analyser/api/routes/request_context_helpers.py | api-runtime-shell | api-route | port | 40 |
| src/bill_analyser/api/routes/rules.py | classification-rules | api-route | port | 105 |
| src/bill_analyser/api/routes/settings_bundle.py | settings-bundle | api-route | port | 279 |
| src/bill_analyser/api/routes/statistics/__init__.py | statistics-reporting | package-marker | facade | 43 |
| src/bill_analyser/api/routes/statistics/asset_trends.py | statistics-reporting | api-route | port | 323 |
| src/bill_analyser/api/routes/statistics/basic.py | statistics-reporting | api-route | port | 341 |
| src/bill_analyser/api/routes/statistics/category_analysis.py | statistics-reporting | api-route | port | 276 |
| src/bill_analyser/api/routes/statistics/exchange_rates.py | statistics-reporting | api-route | port | 393 |
| src/bill_analyser/api/routes/statistics/support.py | statistics-reporting | api-route | port | 140 |
| src/bill_analyser/api/routes/statistics/trend_analysis.py | statistics-reporting | api-route | port | 363 |
| src/bill_analyser/api/routes/tags.py | tags-templates | api-route | port | 237 |
| src/bill_analyser/api/routes/templates.py | tags-templates | api-route | port | 239 |
| src/bill_analyser/constants.py | shared-primitives | backend-module | port | 17 |
| src/bill_analyser/core/__init__.py | shared-primitives | package-marker | facade | 6 |
| src/bill_analyser/core/ai/__init__.py | ai-learning-llm | package-marker | facade | 1 |
| src/bill_analyser/core/ai/llm/__init__.py | ai-learning-llm | package-marker | facade | 1 |
| src/bill_analyser/core/ai/llm/learning_service/__init__.py | ai-learning-llm | package-marker | facade | 11 |
| src/bill_analyser/core/ai/llm/learning_service/common.py | ai-learning-llm | core-service | port | 5 |
| src/bill_analyser/core/ai/llm/learning_service/errors.py | ai-learning-llm | core-service | port | 10 |
| src/bill_analyser/core/ai/llm/learning_service/generation.py | ai-learning-llm | core-service | port | 106 |
| src/bill_analyser/core/ai/llm/learning_service/limits.py | ai-learning-llm | core-service | port | 10 |
| src/bill_analyser/core/ai/llm/learning_service/normalization.py | ai-learning-llm | core-service | port | 92 |
| src/bill_analyser/core/ai/llm/learning_service/preview_recommendations.py | ai-learning-llm | core-service | port | 349 |
| src/bill_analyser/core/ai/llm/learning_service/rate_limit.py | ai-learning-llm | core-service | port | 43 |
| src/bill_analyser/core/ai/llm/learning_service/rule_synthesis.py | ai-learning-llm | core-service | port | 547 |
| src/bill_analyser/core/ai/llm/learning_service/service.py | ai-learning-llm | core-service | port | 34 |
| src/bill_analyser/core/ai/llm/learning_service/session_analysis.py | ai-learning-llm | core-service | port | 324 |
| src/bill_analyser/core/ai/llm/prompts.py | ai-learning-llm | core-service | port | 253 |
| src/bill_analyser/core/ai/llm/provider.py | ai-learning-llm | core-service | port | 346 |
| src/bill_analyser/core/ai/ocr/__init__.py | ai-ocr | package-marker | facade | 1 |
| src/bill_analyser/core/ai/ocr/payment_screenshot_parser.py | ai-ocr | core-service | port | 245 |
| src/bill_analyser/core/ai/ocr/provider.py | ai-ocr | core-service | port | 181 |
| src/bill_analyser/core/ai/ocr/service.py | ai-ocr | core-service | port | 340 |
| src/bill_analyser/core/analyzer.py | statistics-reporting | core-service | port | 683 |
| src/bill_analyser/core/auth_rust_bridge.py | auth-security | core-service | port | 148 |
| src/bill_analyser/core/bill_date_utils.py | shared-primitives | core-service | port | 51 |
| src/bill_analyser/core/bills/__init__.py | bills-import | package-marker | facade | 5 |
| src/bill_analyser/core/bills/service_parts/__init__.py | bills-import | package-marker | facade | 5 |
| src/bill_analyser/core/bills/service_parts/account_matching.py | bills-import | core-service | port | 603 |
| src/bill_analyser/core/bills/service_parts/category_maintenance.py | bills-import | core-service | port | 301 |
| src/bill_analyser/core/bills/service_parts/common.py | bills-import | core-service | port | 44 |
| src/bill_analyser/core/bills/service_parts/facade.py | bills-import | core-service | port | 171 |
| src/bill_analyser/core/bills/service_parts/import_learning_rules.py | bills-import | core-service | port | 545 |
| src/bill_analyser/core/bills/service_parts/import_learning_signals.py | bills-import | core-service | port | 548 |
| src/bill_analyser/core/bills/service_parts/import_legacy.py | bills-import | core-service | port | 353 |
| src/bill_analyser/core/bills/service_parts/import_preview_paging.py | bills-import | core-service | port | 313 |
| src/bill_analyser/core/bills/service_parts/import_preview_projection.py | bills-import | core-service | port | 423 |
| src/bill_analyser/core/bills/service_parts/import_reclassify.py | bills-import | core-service | port | 230 |
| src/bill_analyser/core/bills/service_parts/import_v2_pipeline.py | bills-import | core-service | port | 576 |
| src/bill_analyser/core/bills/service_parts/investment_helpers.py | bills-import | core-service | port | 171 |
| src/bill_analyser/core/bills/service_parts/matching_actions.py | bills-import | core-service | port | 569 |
| src/bill_analyser/core/bills/service_parts/matching_preview_accept.py | bills-import | core-service | port | 302 |
| src/bill_analyser/core/bills/service_parts/matching_preview_reject_clear.py | bills-import | core-service | port | 291 |
| src/bill_analyser/core/bills/service_parts/matching_reads.py | bills-import | core-service | port | 501 |
| src/bill_analyser/core/bills/service_parts/preview_learning_decisions.py | bills-import | core-service | port | 399 |
| src/bill_analyser/core/bills/service_parts/preview_pairing_decisions.py | bills-import | core-service | port | 304 |
| src/bill_analyser/core/budgets/__init__.py | budgets | package-marker | facade | 5 |
| src/bill_analyser/core/budgets/execution_summary.py | budgets | core-service | port | 118 |
| src/bill_analyser/core/budgets/manager.py | budgets | core-service | port | 152 |
| src/bill_analyser/core/category_engine/__init__.py | classification-rules | package-marker | facade | 49 |
| src/bill_analyser/core/category_engine/compiled_rule.py | classification-rules | core-service | port | 28 |
| src/bill_analyser/core/category_engine/engine.py | classification-rules | core-service | port | 622 |
| src/bill_analyser/core/category_engine/expression.py | classification-rules | core-service | port | 112 |
| src/bill_analyser/core/category_engine/matcher.py | classification-rules | core-service | port | 642 |
| src/bill_analyser/core/database/__init__.py | database-facade | package-marker | facade | 2 |
| src/bill_analyser/core/database/accounts/__init__.py | accounts | package-marker | facade | 20 |
| src/bill_analyser/core/database/accounts/balances.py | accounts | database-access | port | 309 |
| src/bill_analyser/core/database/accounts/mutations.py | accounts | database-access | port | 251 |
| src/bill_analyser/core/database/accounts/reads.py | accounts | database-access | port | 284 |
| src/bill_analyser/core/database/audit_backup/__init__.py | backup-operations | package-implementation | port | 269 |
| src/bill_analyser/core/database/bills/__init__.py | bills-import | package-implementation | port | 692 |
| src/bill_analyser/core/database/budgets/__init__.py | budgets | package-marker | facade | 2 |
| src/bill_analyser/core/database/budgets/core/__init__.py | budgets | package-marker | facade | 22 |
| src/bill_analyser/core/database/budgets/core/hierarchy.py | budgets | database-access | port | 400 |
| src/bill_analyser/core/database/budgets/core/listing.py | budgets | database-access | port | 163 |
| src/bill_analyser/core/database/budgets/core/mutations.py | budgets | database-access | port | 100 |
| src/bill_analyser/core/database/budgets/core/shared.py | budgets | database-access | port | 369 |
| src/bill_analyser/core/database/budgets/execution/__init__.py | budgets | package-marker | facade | 22 |
| src/bill_analyser/core/database/budgets/execution/details.py | budgets | database-access | port | 310 |
| src/bill_analyser/core/database/budgets/execution/history.py | budgets | database-access | port | 222 |
| src/bill_analyser/core/database/budgets/execution/on_demand.py | budgets | database-access | port | 211 |
| src/bill_analyser/core/database/budgets/execution/snapshots.py | budgets | database-access | port | 134 |
| src/bill_analyser/core/database/budgets/forecast/__init__.py | budgets | package-implementation | port | 510 |
| src/bill_analyser/core/database/budgets/reporting/__init__.py | budgets | package-marker | facade | 13 |
| src/bill_analyser/core/database/categories/__init__.py | classification-rules | package-implementation | port | 410 |
| src/bill_analyser/core/database/category_rules/__init__.py | classification-rules | package-implementation | port | 551 |
| src/bill_analyser/core/database/encryption.py | auth-security | database-access | port | 105 |
| src/bill_analyser/core/database/imports/__init__.py | bills-import | package-marker | facade | 2 |
| src/bill_analyser/core/database/imports/configs/__init__.py | bills-import | package-implementation | port | 419 |
| src/bill_analyser/core/database/imports/learning/__init__.py | bills-import | package-marker | facade | 28 |
| src/bill_analyser/core/database/imports/learning/base.py | bills-import | database-access | port | 157 |
| src/bill_analyser/core/database/imports/learning/corpus.py | bills-import | database-access | port | 226 |
| src/bill_analyser/core/database/imports/learning/events.py | bills-import | database-access | port | 151 |
| src/bill_analyser/core/database/imports/learning/model.py | bills-import | database-access | port | 274 |
| src/bill_analyser/core/database/imports/learning/rules.py | bills-import | database-access | port | 461 |
| src/bill_analyser/core/database/imports/learning/suggestion_center.py | bills-import | database-access | port | 421 |
| src/bill_analyser/core/database/imports/preview/__init__.py | bills-import | package-marker | facade | 28 |
| src/bill_analyser/core/database/imports/preview/base.py | bills-import | database-access | port | 258 |
| src/bill_analyser/core/database/imports/preview/confirmation.py | bills-import | database-access | port | 135 |
| src/bill_analyser/core/database/imports/preview/decisions.py | bills-import | database-access | port | 435 |
| src/bill_analyser/core/database/imports/preview/inserts.py | bills-import | database-access | port | 162 |
| src/bill_analyser/core/database/imports/preview/reads.py | bills-import | database-access | port | 256 |
| src/bill_analyser/core/database/imports/preview/updates.py | bills-import | database-access | port | 207 |
| src/bill_analyser/core/database/imports/sessions/__init__.py | bills-import | package-implementation | port | 213 |
| src/bill_analyser/core/database/llm/__init__.py | ai-learning-llm | package-marker | facade | 2 |
| src/bill_analyser/core/database/llm/candidates/__init__.py | ai-learning-llm | package-marker | facade | 24 |
| src/bill_analyser/core/database/llm/candidates/crud.py | ai-learning-llm | database-access | port | 208 |
| src/bill_analyser/core/database/llm/candidates/feedback.py | ai-learning-llm | database-access | port | 154 |
| src/bill_analyser/core/database/llm/candidates/memory.py | ai-learning-llm | database-access | port | 140 |
| src/bill_analyser/core/database/llm/candidates/preview_apply.py | ai-learning-llm | database-access | port | 278 |
| src/bill_analyser/core/database/llm/candidates/preview_review.py | ai-learning-llm | database-access | port | 209 |
| src/bill_analyser/core/database/llm/config/__init__.py | ai-learning-llm | package-implementation | port | 288 |
| src/bill_analyser/core/database/matching/__init__.py | matching-reconciliation | package-marker | facade | 30 |
| src/bill_analyser/core/database/matching/base.py | matching-reconciliation | database-access | port | 205 |
| src/bill_analyser/core/database/matching/candidates.py | matching-reconciliation | database-access | port | 289 |
| src/bill_analyser/core/database/matching/feedback.py | matching-reconciliation | database-access | port | 174 |
| src/bill_analyser/core/database/matching/learning.py | matching-reconciliation | database-access | port | 192 |
| src/bill_analyser/core/database/matching/manual_pairs.py | matching-reconciliation | database-access | port | 372 |
| src/bill_analyser/core/database/matching/settings.py | matching-reconciliation | database-access | port | 141 |
| src/bill_analyser/core/database/matching/suppressions.py | matching-reconciliation | database-access | port | 551 |
| src/bill_analyser/core/database/reconciliation/__init__.py | matching-reconciliation | package-marker | facade | 25 |
| src/bill_analyser/core/database/reconciliation/actions.py | matching-reconciliation | database-access | port | 410 |
| src/bill_analyser/core/database/reconciliation/base.py | matching-reconciliation | database-access | port | 354 |
| src/bill_analyser/core/database/reconciliation/persistence.py | matching-reconciliation | database-access | port | 407 |
| src/bill_analyser/core/database/reconciliation/projection.py | matching-reconciliation | database-access | port | 451 |
| src/bill_analyser/core/database/recurring_suggestions/__init__.py | recurring-calendar | package-implementation | port | 275 |
| src/bill_analyser/core/database/runtime.py | database-facade | database-access | port | 235 |
| src/bill_analyser/core/database/schema/__init__.py | database-schema | package-marker | facade | 65 |
| src/bill_analyser/core/database/schema/core/__init__.py | database-schema | package-marker | facade | 39 |
| src/bill_analyser/core/database/schema/core/budgets_exchange.py | database-schema | database-schema | port | 137 |
| src/bill_analyser/core/database/schema/core/business.py | database-schema | database-schema | port | 310 |
| src/bill_analyser/core/database/schema/core/indexes.py | database-schema | database-schema | port | 120 |
| src/bill_analyser/core/database/schema/core/matching.py | database-schema | database-schema | port | 197 |
| src/bill_analyser/core/database/schema/core/migrations.py | database-schema | database-schema | port | 202 |
| src/bill_analyser/core/database/schema/templates_imports/__init__.py | database-schema | package-marker | facade | 39 |
| src/bill_analyser/core/database/schema/templates_imports/learning.py | database-schema | database-schema | port | 255 |
| src/bill_analyser/core/database/schema/templates_imports/migrations.py | database-schema | database-schema | port | 25 |
| src/bill_analyser/core/database/schema/templates_imports/staging.py | database-schema | database-schema | port | 181 |
| src/bill_analyser/core/database/schema/templates_imports/suggestions.py | database-schema | database-schema | port | 47 |
| src/bill_analyser/core/database/schema/templates_imports/templates.py | database-schema | database-schema | port | 95 |
| src/bill_analyser/core/database/schema/users_security.py | database-schema | database-schema | port | 345 |
| src/bill_analyser/core/database/settings_bundle/__init__.py | settings-bundle | package-marker | facade | 36 |
| src/bill_analyser/core/database/settings_bundle/accounts_categories_tags.py | settings-bundle | database-access | port | 386 |
| src/bill_analyser/core/database/settings_bundle/base.py | settings-bundle | database-access | port | 216 |
| src/bill_analyser/core/database/settings_bundle/exporters.py | settings-bundle | database-access | port | 199 |
| src/bill_analyser/core/database/settings_bundle/resolution.py | settings-bundle | database-access | port | 106 |
| src/bill_analyser/core/database/settings_bundle/rules_llm_ocr.py | settings-bundle | database-access | port | 259 |
| src/bill_analyser/core/database/settings_bundle/shared.py | settings-bundle | database-access | port | 118 |
| src/bill_analyser/core/database/settings_bundle/templates.py | settings-bundle | database-access | port | 315 |
| src/bill_analyser/core/database/shared.py | database-facade | database-access | port | 224 |
| src/bill_analyser/core/database/tags/__init__.py | tags-templates | package-implementation | port | 247 |
| src/bill_analyser/core/database/templates/__init__.py | tags-templates | package-marker | facade | 7 |
| src/bill_analyser/core/database/templates/crud.py | tags-templates | database-access | port | 287 |
| src/bill_analyser/core/database/templates/mixin.py | tags-templates | database-access | port | 75 |
| src/bill_analyser/core/database/templates/recurring.py | tags-templates | database-access | port | 247 |
| src/bill_analyser/core/database/templates/schedule.py | tags-templates | database-access | port | 145 |
| src/bill_analyser/core/database/templates/serialization.py | tags-templates | database-access | port | 80 |
| src/bill_analyser/core/database/time.py | database-facade | database-access | port | 15 |
| src/bill_analyser/core/database/users/__init__.py | auth-security | package-marker | facade | 2 |
| src/bill_analyser/core/database/users/auth/__init__.py | auth-security | package-implementation | port | 578 |
| src/bill_analyser/core/database/users/data/__init__.py | auth-security | package-implementation | port | 351 |
| src/bill_analyser/core/db.py | database-facade | core-service | facade | 86 |
| src/bill_analyser/core/default_category_seed.py | classification-rules | core-service | port | 692 |
| src/bill_analyser/core/exchange_rate_providers/__init__.py | statistics-reporting | package-marker | facade | 33 |
| src/bill_analyser/core/exchange_rate_providers/base.py | statistics-reporting | core-service | port | 77 |
| src/bill_analyser/core/exchange_rate_providers/china.py | statistics-reporting | core-service | port | 236 |
| src/bill_analyser/core/exchange_rate_providers/global_providers.py | statistics-reporting | core-service | port | 378 |
| src/bill_analyser/core/exchange_rate_providers/manager.py | statistics-reporting | core-service | port | 147 |
| src/bill_analyser/core/exchange_rate_providers/parsing.py | statistics-reporting | core-service | port | 90 |
| src/bill_analyser/core/import_learning/__init__.py | bills-import | package-marker | facade | 6 |
| src/bill_analyser/core/import_learning/features.py | bills-import | core-service | port | 210 |
| src/bill_analyser/core/import_learning/model.py | bills-import | core-service | port | 196 |
| src/bill_analyser/core/import_learning/policy.py | bills-import | core-service | port | 81 |
| src/bill_analyser/core/investment/__init__.py | matching-reconciliation | package-marker | facade | 1 |
| src/bill_analyser/core/investment/matching.py | matching-reconciliation | core-service | port | 557 |
| src/bill_analyser/core/investment/settings.py | matching-reconciliation | core-service | port | 220 |
| src/bill_analyser/core/matching/__init__.py | matching-reconciliation | package-marker | facade | 12 |
| src/bill_analyser/core/matching/candidate_ids.py | matching-reconciliation | core-service | port | 109 |
| src/bill_analyser/core/matching/models.py | matching-reconciliation | core-service | port | 137 |
| src/bill_analyser/core/matching/preview_matching.py | matching-reconciliation | core-service | port | 532 |
| src/bill_analyser/core/matching/session_candidates.py | matching-reconciliation | core-service | port | 260 |
| src/bill_analyser/core/matching/transfer_candidates.py | matching-reconciliation | core-service | port | 176 |
| src/bill_analyser/core/recurring_detection.py | recurring-calendar | core-service | port | 270 |
| src/bill_analyser/core/report.py | statistics-reporting | core-service | facade | 23 |
| src/bill_analyser/core/smart_dedup/__init__.py | smart-dedup | package-marker | facade | 52 |
| src/bill_analyser/core/smart_dedup/database.py | smart-dedup | core-service | port | 195 |
| src/bill_analyser/core/smart_dedup/engine.py | smart-dedup | core-service | port | 252 |
| src/bill_analyser/core/smart_dedup/exact.py | smart-dedup | core-service | port | 71 |
| src/bill_analyser/core/smart_dedup/grouping.py | smart-dedup | core-service | port | 346 |
| src/bill_analyser/core/smart_dedup/models.py | smart-dedup | core-service | port | 42 |
| src/bill_analyser/core/smart_dedup/normalization.py | smart-dedup | core-service | port | 422 |
| src/bill_analyser/core/smart_dedup/platform_bank.py | smart-dedup | core-service | port | 268 |
| src/bill_analyser/core/smart_dedup/reconciliation.py | smart-dedup | core-service | port | 298 |
| src/bill_analyser/core/smart_dedup/transfers.py | smart-dedup | core-service | port | 343 |
| src/bill_analyser/core/sync.py | sync-runtime | core-service | port | 402 |
| src/bill_analyser/import_contracts/__init__.py | import-contracts | package-marker | facade | 17 |
| src/bill_analyser/import_contracts/parser_tags.py | import-contracts | import-contract | facade | 142 |
| src/bill_analyser/parsers/__init__.py | import-parsers | package-marker | facade | 6 |
| src/bill_analyser/parsers/abc.py | import-parsers | import-parser | port | 336 |
| src/bill_analyser/parsers/alipay.py | import-parsers | import-parser | port | 153 |
| src/bill_analyser/parsers/base.py | import-parsers | import-parser | port | 484 |
| src/bill_analyser/parsers/ccb.py | import-parsers | import-parser | port | 254 |
| src/bill_analyser/parsers/cmbc.py | import-parsers | import-parser | port | 423 |
| src/bill_analyser/parsers/factory.py | import-parsers | import-parser | port | 268 |
| src/bill_analyser/parsers/icbc.py | import-parsers | import-parser | port | 512 |
| src/bill_analyser/parsers/parser_tags.py | import-parsers | import-parser | facade | 10 |
| src/bill_analyser/parsers/wechat.py | import-parsers | import-parser | port | 256 |
| src/bill_analyser/utils/__init__.py | shared-primitives | package-marker | facade | 5 |
| src/bill_analyser/utils/charting/__init__.py | statistics-reporting | package-marker | facade | 12 |
| src/bill_analyser/utils/charting/budgets.py | statistics-reporting | utility | port | 129 |
| src/bill_analyser/utils/charting/categories.py | statistics-reporting | utility | port | 97 |
| src/bill_analyser/utils/charting/comparison.py | statistics-reporting | utility | port | 96 |
| src/bill_analyser/utils/charting/constants.py | statistics-reporting | utility | port | 34 |
| src/bill_analyser/utils/charting/dashboard.py | statistics-reporting | utility | port | 171 |
| src/bill_analyser/utils/charting/generator.py | statistics-reporting | utility | port | 86 |
| src/bill_analyser/utils/charting/heatmap.py | statistics-reporting | utility | port | 102 |
| src/bill_analyser/utils/charting/matplotlib_setup.py | statistics-reporting | utility | port | 14 |
| src/bill_analyser/utils/charting/ranking.py | statistics-reporting | utility | port | 95 |
| src/bill_analyser/utils/charting/trends.py | statistics-reporting | utility | port | 110 |
| src/bill_analyser/utils/charts.py | statistics-reporting | utility | port | 12 |
| src/bill_analyser/utils/config.py | api-runtime-shell | utility | port | 585 |
| src/bill_analyser/utils/constants.py | shared-primitives | utility | port | 231 |
| src/bill_analyser/utils/currency.py | shared-primitives | utility | port | 160 |
| src/bill_analyser/utils/logger.py | api-runtime-shell | utility | port | 491 |
| src/bill_analyser/utils/report_export.py | statistics-reporting | utility | port | 225 |
| src/bill_analyser/utils/validator.py | shared-primitives | utility | port | 278 |

## Rust Backend Files

- crates/bill-analyser-core/src/adapters/account.rs
- crates/bill-analyser-core/src/adapters/api.rs
- crates/bill-analyser-core/src/adapters/category.rs
- crates/bill-analyser-core/src/adapters/mod.rs
- crates/bill-analyser-core/src/adapters/transaction.rs
- crates/bill-analyser-core/src/auth/mod.rs
- crates/bill-analyser-core/src/bin/bill_auth_bridge.rs
- crates/bill-analyser-core/src/error.rs
- crates/bill-analyser-core/src/lib.rs
- crates/bill-analyser-core/src/primitives/auth.rs
- crates/bill-analyser-core/src/primitives/currency.rs
- crates/bill-analyser-core/src/primitives/date_time.rs
- crates/bill-analyser-core/src/primitives/ids.rs
- crates/bill-analyser-core/src/primitives/mod.rs
- crates/bill-analyser-core/src/primitives/money.rs
- crates/bill-analyser-core/src/primitives/pagination.rs
- crates/bill-analyser-core/src/primitives/sorting.rs
- crates/bill-analyser-core/src/primitives/transaction_type.rs
- crates/bill-analyser-core/src/response.rs
- crates/bill-analyser-core/src/runtime.rs
- crates/bill-analyser-core/tests/auth_security_contracts.rs
- crates/bill-analyser-core/tests/response_contract.rs
- crates/bill-analyser-core/tests/runtime_contract.rs
- crates/bill-analyser-core/tests/shared_primitives.rs
- crates/bill-analyser-db/src/connection.rs
- crates/bill-analyser-db/src/error.rs
- crates/bill-analyser-db/src/lib.rs
- crates/bill-analyser-db/src/path.rs
- crates/bill-analyser-db/src/schema.rs
- crates/bill-analyser-db/src/transaction.rs
- crates/bill-analyser-db/src/user_scope.rs
- crates/bill-analyser-db/tests/sqlite_runtime.rs

## Verified Dead Baseline

No Python backend file is marked `verified_dead` in S0.
Later slices may use this inventory as the comparison surface, but any deletion requires direct evidence.
