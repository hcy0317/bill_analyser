import { spawnSync } from 'child_process';
import { readFileSync } from 'fs';
import path from 'path';

import { expect, type Page } from '@playwright/test';
import { mdiFilterOutline } from '@mdi/js';

import { E2EApiClient } from './apiClient';
import type { E2EEnvironment } from './env';
import { desktopRoute } from './routes';

export const IMPORT_SIGNAL_FAMILIES = [
    'parser',
    'platform_duplicate',
    'transfer',
    'history',
    'learning',
    'llm'
] as const;

export const IMPORT_SIGNAL_ROW_COUNT = 12;

export const IMPORT_SIGNAL_LABELS = [
    'Parser',
    'Platform Duplicate',
    'Transfer Match',
    'History Rewrite',
    'Learning Suggestion',
    'LLM Suggestion'
] as const;

export const IMPORT_AUXILIARY_SIGNAL_LABELS = [
    'Recurring',
    'Reconciliation',
    'Identity Validation'
] as const;

export const IMPORT_SIGNAL_MARKERS = {
    parser: 'E2E-SIG-PARSER',
    platform_duplicate: 'E2E-SIG-PLATFORM',
    transfer: 'E2E-SIG-TRANSFER-PENDING',
    transfer_terminal: 'E2E-SIG-TRANSFER-TERMINAL',
    history: 'E2E-SIG-HISTORY',
    learning: 'E2E-SIG-LEARNING',
    llm: 'E2E-SIG-LLM-ACTION',
    llm_edit: 'E2E-SIG-LLM-EDIT'
} as const;

export const EXPECTED_SIGNAL_COUNTS = {
    parser: 6,
    platform_duplicate: 1,
    transfer: 1,
    history: 1,
    learning: 1,
    llm: 2
} as const;

interface Entity {
    readonly id: string | number;
    readonly name: string;
}

export interface ImportSignalFixtureContext {
    readonly account: Entity;
    readonly expenseCategory: Entity;
}

export interface ImportSignalPreviewPage {
    readonly preview?: Array<Record<string, unknown>>;
    readonly total?: number;
    readonly metadata?: {
        readonly counts?: {
            readonly signals?: Record<string, number>;
        };
    };
}

export interface ImportSignalDedupResponse {
    readonly session_id: string;
    readonly preview?: Array<Record<string, unknown>>;
    readonly after_dedup?: number;
}

export function importSignalFixturePath(): string {
    return path.resolve(__dirname, '../fixtures/import-signal-system.csv');
}

export async function stageImportSignalFixtureViaApi(
    client: E2EApiClient
): Promise<ImportSignalDedupResponse> {
    const parse = await client.postMultipart<{ session_id: string; parsed_count: number }>('bills/import/v2/parse', {
        parser_type: 'auto',
        files: {
            name: 'import-signal-system.csv',
            mimeType: 'text/csv',
            buffer: readFileSync(importSignalFixturePath())
        }
    });
    expect(parse.parsed_count).toBe(IMPORT_SIGNAL_ROW_COUNT);
    const dedup = await client.post<ImportSignalDedupResponse>('bills/import/v2/dedup', {
        session_id: parse.session_id,
        include_preview: false
    });
    expect(dedup.after_dedup).toBe(IMPORT_SIGNAL_ROW_COUNT);
    return dedup;
}

export async function createImportSignalFixtureContext(
    client: E2EApiClient,
    env: E2EEnvironment
): Promise<ImportSignalFixtureContext> {
    const suffix = `${env.runId}-signals`;
    const accountName = `E2E Signal CNY ${suffix}`;
    const expenseName = `E2E Signal Expense ${suffix}`;
    const account = entityFromResponse(await client.post<Record<string, unknown>>('accounts', {
        name: accountName,
        category: 1,
        type: 1,
        icon: '1',
        color: '#4c6ef5',
        currency: 'CNY',
        balanceCents: 0,
        balanceTime: 0,
        comment: suffix,
        clientSessionId: suffix
    }), accountName);
    const expenseCategory = entityFromResponse(await client.post<Record<string, unknown>>('categories', {
        name: expenseName,
        type: 3,
        parentId: '0',
        icon: '1',
        color: '#e03131',
        comment: suffix,
        displayOrder: 0,
        ruleExpression: '',
        clientSessionId: suffix
    }), expenseName);
    return { account, expenseCategory };
}

export function patchImportSignalPreview(
    sessionId: string,
    context: ImportSignalFixtureContext
): void {
    const accountId = numericEntityId(context.account);
    const expenseCategoryId = numericEntityId(context.expenseCategory);
    const feedbackByMarker: Record<string, Record<string, unknown>> = {
        [IMPORT_SIGNAL_MARKERS.parser]: {
            parser: { parser_id: 'alipay', parser_tags: ['e2e:parser'] },
            recurring: { review_status: 'pending', candidate_count: 1, reason: 'auxiliary only' },
            reconciliation: { review_status: 'pending', note: 'auxiliary only' },
            identity_validation: { review_status: 'pending', issues: [] }
        },
        [IMPORT_SIGNAL_MARKERS.platform_duplicate]: {},
        [IMPORT_SIGNAL_MARKERS.transfer]: {
            transfer: {
                review_status: 'pending',
                candidate_id: 'e2e-transfer-pending',
                candidate: 'E2E transfer candidate',
                reason: 'deterministic pending transfer',
                score: 0.99
            }
        },
        [IMPORT_SIGNAL_MARKERS.transfer_terminal]: {
            transfer: {
                review_status: 'accepted',
                candidate_id: 'e2e-transfer-terminal',
                candidate: 'E2E terminal transfer'
            }
        },
        [IMPORT_SIGNAL_MARKERS.history]: {
            reconciliation: {
                review_status: 'pending',
                planned_operation: 'update_history',
                destructive_ack_required: false,
                reason: 'deterministic history rewrite'
            }
        },
        [IMPORT_SIGNAL_MARKERS.learning]: {
            learning: {
                review_status: 'pending',
                lifecycle_status: 'pending',
                signal_state: 'pending',
                source: 'model',
                model_version: 'e2e-model-v1',
                recommendation_key: `e2e:${sessionId}:learning`,
                score: 0.91,
                summary: 'deterministic learning suggestion'
            }
        },
        [IMPORT_SIGNAL_MARKERS.llm]: {
            llm: {
                review_status: 'pending',
                confidence: 0.88,
                reason: 'deterministic LLM action suggestion',
                suggested_type: '支出',
                suggested_category_id: expenseCategoryId,
                suggested_main_category: context.expenseCategory.name,
                suggested_source_account: context.account.name
            }
        },
        [IMPORT_SIGNAL_MARKERS.llm_edit]: {
            llm: {
                review_status: 'pending',
                confidence: 0.87,
                reason: 'deterministic LLM edit suggestion',
                suggested_type: '支出',
                suggested_category_id: expenseCategoryId,
                suggested_main_category: context.expenseCategory.name,
                suggested_source_account: context.account.name
            },
            recurring: { review_status: 'pending', candidate_count: 1, reason: 'dependent signal' }
        }
    };
    const feedbackCase = Object.entries(feedbackByMarker)
        .map(([marker, feedback]) => `WHEN ${sqlLiteral(marker)} THEN ${sqlLiteral(JSON.stringify(feedback))}::jsonb`)
        .join('\n');
    const selectedFalseMarkers = [
        IMPORT_SIGNAL_MARKERS.transfer,
        IMPORT_SIGNAL_MARKERS.transfer_terminal,
        IMPORT_SIGNAL_MARKERS.history
    ].map(sqlLiteral).join(', ');
    const sql = `
WITH target_session AS (
    SELECT id
    FROM import_sessions
    WHERE session_key = ${sqlLiteral(sessionId)}
)
UPDATE import_preview_rows AS preview
SET selected = CASE WHEN preview.merchant IN (${selectedFalseMarkers}) THEN false ELSE true END,
    account_id = ${accountId},
    transfer_target_account_id = NULL,
    category_id = ${expenseCategoryId},
    transaction_type = '支出',
    direction = 'expense',
    preview_payload = preview.preview_payload || jsonb_build_object(
        'preview_type', '支出',
        'preview_source_account_id', ${accountId},
        'preview_destination_account_id', NULL,
        'category_id', ${expenseCategoryId},
        'categoryId', ${expenseCategoryId},
        'preview_main_category', ${sqlLiteral(context.expenseCategory.name)},
        'preview_sub_category', '',
        'preview_selected', CASE WHEN preview.merchant IN (${selectedFalseMarkers}) THEN false ELSE true END,
        'dedup_type', CASE WHEN preview.merchant = ${sqlLiteral(IMPORT_SIGNAL_MARKERS.platform_duplicate)} THEN 'platform_bank' ELSE 'remaining' END,
        'preview_matching_feedback', CASE preview.merchant
            ${feedbackCase}
            ELSE '{}'::jsonb
        END
    ),
    updated_at = now()
FROM target_session
WHERE preview.session_id = target_session.id
  AND preview.merchant LIKE 'E2E-SIG-%';
`;
    runFixtureSql(sql);
}

export async function openImportSignalDialog(page: Page, env: E2EEnvironment): Promise<void> {
    await page.goto(desktopRoute('/transaction/list?pageType=0&dateType=7', env), {
        waitUntil: 'domcontentloaded'
    });
    await page.getByTestId('desktop.transactions.action.import').click();
    await expect(page.getByTestId('desktop.import.dialog')).toBeVisible();
    await page.getByTestId('desktop.import.file-input').setInputFiles(importSignalFixturePath());
}

export async function refreshPreviewThroughFilter(
    page: Page,
    label: string
): Promise<Record<string, unknown>> {
    const menu = await openSignalFilterMenu(page);
    const responsePromise = page.waitForResponse(response => (
        response.request().method() === 'GET'
        && response.url().includes('/api/bills/import/v2/preview/')
        && response.url().includes(`signal=${encodeURIComponent(signalFamilyForLabel(label))}`)
    ));
    await menu.getByText(label, { exact: true }).click();
    const response = await responsePromise;
    expect(response.ok(), `${label} preview request`).toBe(true);
    return response.json() as Promise<Record<string, unknown>>;
}

export async function openSignalFilterMenu(page: Page) {
    const dialog = page.getByTestId('desktop.import.dialog');
    const menu = page.locator('.import-check-data-filter-menu');
    if (!await menu.isVisible()) {
        const filterButton = dialog.locator(`button:has(path[d="${mdiFilterOutline}"])`).first();
        await filterButton.click();
        await expect(menu).toBeVisible();
    }
    const signalsGroup = menu.getByText('Signals', { exact: true });
    const firstSignalLabel = menu.getByText(IMPORT_SIGNAL_LABELS[0], { exact: true });
    if (!await firstSignalLabel.isVisible()) {
        await signalsGroup.click();
        await expect(firstSignalLabel).toBeVisible();
    }
    return menu;
}

export function previewRowsFromEnvelope(value: Record<string, unknown>): Array<Record<string, unknown>> {
    const data = value['data'];
    if (data && typeof data === 'object') {
        const preview = (data as Record<string, unknown>)['preview'];
        return Array.isArray(preview) ? preview as Array<Record<string, unknown>> : [];
    }
    const preview = value['preview'];
    return Array.isArray(preview) ? preview as Array<Record<string, unknown>> : [];
}

export function signalCountsFromEnvelope(value: Record<string, unknown>): Record<string, number> {
    const data = value['data'] && typeof value['data'] === 'object'
        ? value['data'] as Record<string, unknown>
        : value;
    const metadata = data['metadata'];
    const counts = metadata && typeof metadata === 'object'
        ? (metadata as Record<string, unknown>)['counts']
        : undefined;
    const signals = counts && typeof counts === 'object'
        ? (counts as Record<string, unknown>)['signals']
        : undefined;
    return signals && typeof signals === 'object'
        ? signals as Record<string, number>
        : {};
}

export function rowIncludesMarker(row: Record<string, unknown>, marker: string): boolean {
    return JSON.stringify(row).includes(marker);
}

function signalFamilyForLabel(label: string): string {
    const index = IMPORT_SIGNAL_LABELS.indexOf(label as typeof IMPORT_SIGNAL_LABELS[number]);
    if (index < 0) {
        throw new Error(`Unknown import signal label: ${label}`);
    }
    return IMPORT_SIGNAL_FAMILIES[index] as string;
}

function runFixtureSql(sql: string): void {
    const container = process.env['E2E_POSTGRES_CONTAINER']?.trim() || 'bill-analyser-postgres';
    const database = process.env['E2E_POSTGRES_DATABASE']?.trim() || 'bill_analyser_e2e';
    if (!/(e2e|test)/iu.test(database)) {
        throw new Error('E2E_POSTGRES_DATABASE must name a dedicated e2e/test database.');
    }
    const result = spawnSync('docker', [
        'exec', '-i', container,
        'psql', '-U', 'bill_analyser', '-d', database,
        '-v', 'ON_ERROR_STOP=1', '-q'
    ], {
        encoding: 'utf8',
        input: sql,
        windowsHide: true
    });
    if (result.status !== 0) {
        const diagnostic = result.stderr
            .split(/\r?\n/gu)
            .filter(line => !/password|postgres(?:ql)?:\/\//iu.test(line))
            .slice(-4)
            .join(' | ');
        throw new Error(
            `Unable to seed isolated import signal fixture (exit ${result.status ?? 'unknown'}): ${diagnostic}`
        );
    }
}

function entityFromResponse(response: Record<string, unknown>, fallbackName: string): Entity {
    const id = response['id'] ?? response['category_id'] ?? response['categoryId'] ?? response['account_id'] ?? response['accountId'];
    if (typeof id !== 'string' && typeof id !== 'number') {
        throw new Error(`Fixture entity response is missing id for ${fallbackName}.`);
    }
    return {
        id,
        name: typeof response['name'] === 'string' ? response['name'] : fallbackName
    };
}

function numericEntityId(entity: Entity): number {
    const value = typeof entity.id === 'number' ? entity.id : Number(entity.id);
    if (!Number.isFinite(value) || value <= 0) {
        throw new Error(`Fixture entity id is invalid for ${entity.name}.`);
    }
    return Math.trunc(value);
}

function sqlLiteral(value: string): string {
    return `'${value.replace(/'/gu, "''")}'`;
}
