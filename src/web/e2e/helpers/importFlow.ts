import { readFileSync } from 'fs';
import path from 'path';

import { expect } from '@playwright/test';

import { E2EApiClient } from './apiClient';
import type { E2EEnvironment } from './env';

export const ALIPAY_SAMPLE_SOURCE_TOTALS = {
    fileName: 'alipay_statement_sample.csv',
    sourceRecordCount: 6,
    sourceIncomeCents: 2522,
    sourceExpenseCents: 4000,
    startDate: '2026-01-15',
    endDate: '2026-01-18',
    merchant: '测试早餐店'
} as const;

export const JANUARY_2026_RANGE = {
    startTime: 1767225600,
    endTime: 1769903999
} as const;

interface ImportStageParseData {
    readonly session_id: string;
    readonly parsed_count: number;
    readonly unmatched_files?: unknown[];
    readonly errors?: string[];
}

interface ImportStageDedupData {
    readonly session_id: string;
    readonly total: number;
    readonly after_dedup: number;
}

interface ImportPreviewPageData {
    readonly preview?: Array<Record<string, unknown>>;
    readonly total?: number;
    readonly metadata?: Record<string, unknown>;
}

interface ImportPreviewSelectionData {
    readonly updated: number;
    readonly selectionAction: string;
    readonly metadata?: {
        readonly counts?: {
            readonly selected?: number;
            readonly selected_invalid?: number;
        };
    };
}

interface ImportPreviewUpdateData {
    readonly updated?: boolean;
    readonly previewItem?: Record<string, unknown>;
}

interface ImportPreviewReclassifyData {
    readonly updated: number;
    readonly total: number;
    readonly categorized: number;
    readonly account_matched: number;
    readonly preview?: Array<Record<string, unknown>>;
}

interface ImportStageConfirmData {
    readonly imported_count: number;
    readonly skipped_count: number;
    readonly errors?: string[];
}

interface TransactionListResponse {
    readonly items?: Array<Record<string, unknown>>;
    readonly totalCount?: number;
}

interface TransactionAmountsResponseItem {
    readonly amounts?: Array<{
        readonly currency: string;
        readonly incomeAmountCents: number;
        readonly expenseAmountCents: number;
    }>;
}

interface Entity {
    readonly id: string | number;
    readonly name: string;
}

interface ImportConfirmContext {
    readonly account: Entity;
    readonly expenseCategory: Entity;
    readonly incomeCategory: Entity;
}

export interface AlipayImportResult {
    readonly sessionId: string;
    readonly parse: ImportStageParseData;
    readonly dedup: ImportStageDedupData;
    readonly preview: ImportPreviewPageData;
    readonly update: ImportPreviewUpdateData;
    readonly reclassify: ImportPreviewReclassifyData;
    readonly deselect: ImportPreviewSelectionData;
    readonly selectAll: ImportPreviewSelectionData;
    readonly confirm: ImportStageConfirmData;
    readonly transactions: TransactionListResponse;
    readonly januaryAmounts: TransactionAmountsResponseItem;
}

export async function runAlipayImportFlow(client: E2EApiClient): Promise<AlipayImportResult> {
    const confirmContext = await createImportConfirmContext(client);
    const parse = await client.postMultipart<ImportStageParseData>('bills/import/v2/parse', {
        parser_type: 'auto',
        files: {
            name: ALIPAY_SAMPLE_SOURCE_TOTALS.fileName,
            mimeType: 'text/csv',
            buffer: readFileSync(alipaySamplePath())
        }
    });

    expect(parse.parsed_count, 'source Alipay sample record count').toBe(ALIPAY_SAMPLE_SOURCE_TOTALS.sourceRecordCount);
    expect(parse.unmatched_files || [], 'Alipay sample should match a dedicated parser').toHaveLength(0);

    const dedup = await client.post<ImportStageDedupData>('bills/import/v2/dedup', {
        session_id: parse.session_id,
        include_preview: false
    });
    expect(dedup.after_dedup, 'dedup preview count').toBeGreaterThan(0);
    expect(dedup.after_dedup, 'dedup preview count must not exceed source records').toBeLessThanOrEqual(ALIPAY_SAMPLE_SOURCE_TOTALS.sourceRecordCount);

    const preview = await client.get<ImportPreviewPageData>(`bills/import/v2/preview/${encodeURIComponent(parse.session_id)}?page=1&page_size=50`);
    expect(Number(preview.total || 0), 'server-paged preview total').toBe(dedup.after_dedup);
    expect(preview.preview || [], 'preview rows').not.toHaveLength(0);

    const previewRows = preview.preview || [];
    const firstPreviewRow = previewRows[0];
    expect(firstPreviewRow, 'first preview row for update smoke').toBeTruthy();
    if (!firstPreviewRow) {
        throw new Error('Import preview update smoke requires at least one preview row');
    }

    const update = await client.put<ImportPreviewUpdateData>(
        `bills/import/v2/preview/${encodeURIComponent(parse.session_id)}/update`,
        {
            ...buildPreviewUpdate(firstPreviewRow, confirmContext),
            responseMode: 'preview-item'
        }
    );
    expect(update.updated, 'single preview update result').toBe(true);
    expect(
        numericField(update.previewItem || {}, ['id']),
        'updated preview item id echoes the requested row'
    ).toBe(numericField(firstPreviewRow, ['id']));

    const previewUpdates = previewRows.map(row => buildPreviewUpdate(row, confirmContext));
    const reclassify = await client.post<ImportPreviewReclassifyData>(
        `bills/import/v2/reclassify/${encodeURIComponent(parse.session_id)}`,
        { preview_updates: previewUpdates }
    );
    expect(reclassify.updated, 'reclassify applies preview updates').toBeGreaterThan(0);
    expect(reclassify.total, 'reclassify keeps preview row count').toBe(dedup.after_dedup);
    expect(reclassify.categorized, 'reclassify returns categorized rows').toBeGreaterThan(0);
    expect(reclassify.account_matched, 'reclassify returns account-matched rows').toBeGreaterThan(0);

    const deselect = await client.put<ImportPreviewSelectionData>(
        `bills/import/v2/preview/${encodeURIComponent(parse.session_id)}/selection`,
        { selectionAction: 'select_none' }
    );
    expect(deselect.selectionAction, 'deselect selection action echo').toBe('select_none');
    expect(deselect.updated, 'deselect touches server-paged preview rows').toBeGreaterThan(0);

    const selectAll = await client.put<ImportPreviewSelectionData>(
        `bills/import/v2/preview/${encodeURIComponent(parse.session_id)}/selection`,
        { selectionAction: 'select_all' }
    );
    expect(selectAll.selectionAction, 'select all action echo').toBe('select_all');
    expect(selectAll.metadata?.counts?.selected || 0, 'server-paged selection count').toBeGreaterThan(0);

    const confirm = await client.post<ImportStageConfirmData>('bills/import/v2/confirm', {
        session_id: parse.session_id,
        preserve_unpatched_selection: true,
        preview_updates: previewUpdates
    });
    expect(confirm.imported_count, 'confirmed imported count').toBeGreaterThan(0);
    expect(confirm.imported_count, 'confirmed imported count must not exceed dedup preview count').toBeLessThanOrEqual(dedup.after_dedup);
    expect(confirm.errors || [], 'confirm errors').toHaveLength(0);

    const transactions = await client.get<TransactionListResponse>(
        'bills/by-month?year=2026&month=1&type=0&categoryIds=&accountIds=&tagIds=&tagFilterType=0&amountFilterCents=&keyword='
    );
    expect(transactions.items || [], 'January transaction list after import').not.toHaveLength(0);
    expect(rowsContaining(transactions.items || [], ALIPAY_SAMPLE_SOURCE_TOTALS.merchant), 'January list rows for fixture merchant').not.toHaveLength(0);

    const amounts = await client.get<Record<string, TransactionAmountsResponseItem>>(
        `statistics/amounts?use_transaction_timezone=false&query=thisMonth_${JANUARY_2026_RANGE.startTime}_${JANUARY_2026_RANGE.endTime}`
    );
    const januaryAmounts = amounts['thisMonth'];
    expect(januaryAmounts?.amounts || [], 'January statistics amounts').not.toHaveLength(0);
    expectAlipayStatisticsTotals(januaryAmounts as TransactionAmountsResponseItem);

    return {
        sessionId: parse.session_id,
        parse,
        dedup,
        preview,
        update,
        reclassify,
        deselect,
        selectAll,
        confirm,
        transactions,
        januaryAmounts: januaryAmounts as TransactionAmountsResponseItem
    };
}

async function createImportConfirmContext(client: E2EApiClient): Promise<ImportConfirmContext> {
    const suffix = `import-${Date.now()}`;
    const account = entityFromResponse(await client.post<Record<string, unknown>>('accounts', {
        name: `E2E Import CNY ${suffix}`,
        category: 1,
        type: 1,
        icon: '1',
        color: '#4c6ef5',
        currency: 'CNY',
        balanceCents: 0,
        balanceTime: 0,
        comment: `created by ${suffix}`,
        clientSessionId: suffix
    }), `E2E Import CNY ${suffix}`);
    const expenseCategory = entityFromResponse(await client.post<Record<string, unknown>>('categories', {
        name: `E2E Import Expense ${suffix}`,
        type: 3,
        parentId: '0',
        icon: '1',
        color: '#e03131',
        comment: `created by ${suffix}`,
        displayOrder: 0,
        ruleExpression: '',
        clientSessionId: suffix
    }), `E2E Import Expense ${suffix}`);
    const incomeCategory = entityFromResponse(await client.post<Record<string, unknown>>('categories', {
        name: `E2E Import Income ${suffix}`,
        type: 2,
        parentId: '0',
        icon: '1',
        color: '#2f9e44',
        comment: `created by ${suffix}`,
        displayOrder: 0,
        ruleExpression: '',
        clientSessionId: suffix
    }), `E2E Import Income ${suffix}`);

    return { account, expenseCategory, incomeCategory };
}

function buildPreviewUpdate(
    row: Record<string, unknown>,
    context: ImportConfirmContext
): Record<string, unknown> {
    const id = numericField(row, ['id']);
    if (id === null) {
        throw new Error(`Preview row is missing id: ${JSON.stringify(row)}`);
    }
    const previewType = stringField(row, ['preview_type', 'type']);
    const category = isIncomePreviewType(previewType) ? context.incomeCategory : context.expenseCategory;
    const categoryId = numericEntityId(category);
    const accountId = numericEntityId(context.account);

    return {
        id,
        preview_type: previewType,
        preview_amount_cents: numericField(row, ['preview_amount_cents', 'amountCents']) ?? 0,
        preview_destination_amount_cents: numericField(row, [
            'preview_destination_amount_cents',
            'destinationAmountCents'
        ]) ?? 0,
        preview_source_account_id: accountId,
        preview_destination_account_id: null,
        category_id: categoryId,
        preview_main_category: category.name,
        preview_sub_category: '',
        selected: true
    };
}

function isIncomePreviewType(previewType: string): boolean {
    return ['收入', 'income', '2'].includes(previewType.trim().toLowerCase());
}

function entityFromResponse(response: Record<string, unknown>, fallbackName: string): Entity {
    const id = response['id'] ?? response['category_id'] ?? response['categoryId'] ?? response['account_id'] ?? response['accountId'];
    if (typeof id !== 'string' && typeof id !== 'number') {
        throw new Error(`Entity response is missing id: ${JSON.stringify(response)}`);
    }

    return {
        id,
        name: typeof response['name'] === 'string' ? response['name'] : fallbackName
    };
}

function numericEntityId(entity: Entity): number {
    const value = typeof entity.id === 'number' ? entity.id : Number(entity.id);
    if (!Number.isFinite(value) || value <= 0) {
        throw new Error(`Entity id is not numeric: ${JSON.stringify(entity)}`);
    }
    return Math.trunc(value);
}

function numericField(row: Record<string, unknown>, keys: readonly string[]): number | null {
    for (const key of keys) {
        const value = row[key];
        if (typeof value === 'number' && Number.isFinite(value)) {
            return Math.trunc(value);
        }
        if (typeof value === 'string' && value.trim()) {
            const parsed = Number(value);
            if (Number.isFinite(parsed)) {
                return Math.trunc(parsed);
            }
        }
    }
    return null;
}

function stringField(row: Record<string, unknown>, keys: readonly string[]): string {
    for (const key of keys) {
        const value = row[key];
        if (typeof value === 'string') {
            return value;
        }
    }
    return '';
}

export function januaryTransactionListRoute(env: E2EEnvironment): string {
    return `${env.baseURL}/desktop.html#/transaction/list?pageType=0&dateType=255&minTime=${JANUARY_2026_RANGE.startTime}&maxTime=${JANUARY_2026_RANGE.endTime}&keyword=${encodeURIComponent(ALIPAY_SAMPLE_SOURCE_TOTALS.merchant)}`;
}

export function januaryStatisticsRoute(env: E2EEnvironment): string {
    return `${env.baseURL}/desktop.html#/statistics/transaction?chartDateType=255&startTime=${JANUARY_2026_RANGE.startTime}&endTime=${JANUARY_2026_RANGE.endTime}`;
}

export function summarizeObservedAmounts(result: AlipayImportResult): string {
    const cny = result.januaryAmounts.amounts?.find(item => item.currency === 'CNY') || result.januaryAmounts.amounts?.[0];
    return JSON.stringify({
        sourceIncomeCents: ALIPAY_SAMPLE_SOURCE_TOTALS.sourceIncomeCents,
        sourceExpenseCents: ALIPAY_SAMPLE_SOURCE_TOTALS.sourceExpenseCents,
        reclassifiedRows: result.reclassify.total,
        selectedRows: result.selectAll.metadata?.counts?.selected,
        importedCount: result.confirm.imported_count,
        observedIncomeCents: cny?.incomeAmountCents,
        observedExpenseCents: cny?.expenseAmountCents
    });
}

export function expectAlipayStatisticsTotals(januaryAmounts: TransactionAmountsResponseItem): void {
    const cny = januaryAmounts.amounts?.find(item => item.currency === 'CNY');
    expect(cny, 'January statistics must include CNY amount bucket').toBeTruthy();
    expect(cny?.incomeAmountCents, 'January CNY income should match the Alipay fixture source total in cents')
        .toBe(ALIPAY_SAMPLE_SOURCE_TOTALS.sourceIncomeCents);
    expect(cny?.expenseAmountCents, 'January CNY expense should match the Alipay fixture source total in cents')
        .toBe(ALIPAY_SAMPLE_SOURCE_TOTALS.sourceExpenseCents);
}

function rowsContaining(rows: Array<Record<string, unknown>>, needle: string): Array<Record<string, unknown>> {
    return rows.filter(row => JSON.stringify(row).includes(needle));
}

function alipaySamplePath(): string {
    return path.resolve(__dirname, '../../../../tests/fixtures/import_samples/alipay_statement_sample.csv');
}
