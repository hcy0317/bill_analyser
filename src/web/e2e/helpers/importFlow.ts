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
        readonly incomeAmount: number;
        readonly expenseAmount: number;
    }>;
}

export interface AlipayImportResult {
    readonly sessionId: string;
    readonly parse: ImportStageParseData;
    readonly dedup: ImportStageDedupData;
    readonly preview: ImportPreviewPageData;
    readonly confirm: ImportStageConfirmData;
    readonly transactions: TransactionListResponse;
    readonly januaryAmounts: TransactionAmountsResponseItem;
}

export async function runAlipayImportFlow(client: E2EApiClient): Promise<AlipayImportResult> {
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

    const confirm = await client.post<ImportStageConfirmData>('bills/import/v2/confirm', {
        session_id: parse.session_id,
        preserve_unpatched_selection: true,
        preview_updates: []
    });
    expect(confirm.imported_count, 'confirmed imported count').toBeGreaterThan(0);
    expect(confirm.imported_count, 'confirmed imported count must not exceed dedup preview count').toBeLessThanOrEqual(dedup.after_dedup);
    expect(confirm.errors || [], 'confirm errors').toHaveLength(0);

    const transactions = await client.get<TransactionListResponse>(
        'bills/by-month?year=2026&month=1&type=0&categoryIds=&accountIds=&tagIds=&tagFilterType=0&amountFilter=&keyword='
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
        confirm,
        transactions,
        januaryAmounts: januaryAmounts as TransactionAmountsResponseItem
    };
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
        importedCount: result.confirm.imported_count,
        observedIncomeCents: cny?.incomeAmount,
        observedExpenseCents: cny?.expenseAmount
    });
}

export function expectAlipayStatisticsTotals(januaryAmounts: TransactionAmountsResponseItem): void {
    const cny = januaryAmounts.amounts?.find(item => item.currency === 'CNY');
    expect(cny, 'January statistics must include CNY amount bucket').toBeTruthy();
    expect(cny?.incomeAmount, 'January CNY income should match the Alipay fixture source total in cents')
        .toBe(ALIPAY_SAMPLE_SOURCE_TOTALS.sourceIncomeCents);
    expect(cny?.expenseAmount, 'January CNY expense should match the Alipay fixture source total in cents')
        .toBe(ALIPAY_SAMPLE_SOURCE_TOTALS.sourceExpenseCents);
}

function rowsContaining(rows: Array<Record<string, unknown>>, needle: string): Array<Record<string, unknown>> {
    return rows.filter(row => JSON.stringify(row).includes(needle));
}

function alipaySamplePath(): string {
    return path.resolve(__dirname, '../../../../tests/fixtures/import_samples/alipay_statement_sample.csv');
}
