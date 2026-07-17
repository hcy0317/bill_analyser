import { describe, expect, jest, test } from '@jest/globals';

const { computed, ref } = jest.requireActual('vue') as typeof import('@/../node_modules/vue');

import {
    getImportConfigDisplayDescription,
    getMatchedImportConfigMessage,
    looksLikeStructuredBillStatementFile,
    normalizeImportConfigMatchResult,
    resolveActiveImportSource,
    resolveImportConfigFileFormat,
    resolveUnmatchedImportFiles,
} from '@/views/desktop/transactions/import/import-dialog/importConfigHelpers.ts';
import { useImportFlowProgress } from '@/views/desktop/transactions/import/import-dialog/useImportFlowProgress.ts';
import type { ImportTransactionDialogStep } from '@/views/desktop/transactions/import/import-dialog/types.ts';

function file(name: string): File {
    return { name } as File;
}

function progressHarness() {
    const currentStep = ref<ImportTransactionDialogStep | 'unexpected'>('uploadFile');
    const submitting = ref(false);
    const importProcess = ref(0);
    const unmatchedFilesQueue = ref<Array<{ originalName: string }>>([]);
    const currentUnmatchedIndex = ref(0);
    const matchedImportConfig = ref<{ id: number; name: string } | null>(null);
    const previewTotalCount = ref(0);
    const importedCount = ref<number | null>(null);
    const fileName = ref('statement.csv');
    const supportedImportFileExtensions = ref('.csv / .xlsx');
    const translate = (key: string, params?: Record<string, unknown>) => (
        params ? `${key}:${JSON.stringify(params)}` : `tt:${key}`
    );
    const formatNumber = (value: number, fractionDigits?: number) => `number:${value}:${fractionDigits}`;
    const formatCount = (count: number) => `count:${count}`;
    const progress = useImportFlowProgress({
        currentStep: currentStep as never,
        submitting,
        importProcess,
        unmatchedFilesQueue: unmatchedFilesQueue as never,
        currentUnmatchedIndex,
        matchedImportConfig: matchedImportConfig as never,
        previewTotalCount,
        importedCount,
        fileName: computed(() => fileName.value),
        supportedImportFileExtensions: computed(() => supportedImportFileExtensions.value),
        translate,
        formatNumber,
        formatCount,
    });

    return {
        currentStep,
        submitting,
        importProcess,
        unmatchedFilesQueue,
        currentUnmatchedIndex,
        matchedImportConfig,
        previewTotalCount,
        importedCount,
        fileName,
        supportedImportFileExtensions,
        progress,
    };
}

describe('import config helpers', () => {
    test('normalizes valid template fields and supplies stable collection defaults', () => {
        expect(normalizeImportConfigMatchResult({
            id: '7' as never,
            name: '  Card CSV  ',
            fileFormat: 'csv',
            description: 'description',
            descriptionSummary: 'summary',
            fieldMappings: { includeHeader: true, columnMapping: { 0: 1 } },
            sampleHeaders: ['日期', '金额'],
            dateFormat: 'YYYY-MM-DD',
            delimiter: ',',
            encoding: 'utf-8',
            skipRows: 2,
            hasHeader: true,
            customRules: [{ field: 'memo' }] as never,
            isDefault: true,
            defaultRecommendation: 'recommended' as never,
            matchScore: 0.95,
            matchReason: 'header_match',
        })).toMatchObject({
            id: 7,
            name: 'Card CSV',
            fieldMappings: { includeHeader: true, columnMapping: { 0: 1 } },
            sampleHeaders: ['日期', '金额'],
            skipRows: 2,
            hasHeader: true,
            isDefault: true,
            matchScore: 0.95,
        });

        expect(normalizeImportConfigMatchResult({ id: 8, name: 'Minimal' })).toMatchObject({
            id: 8,
            name: 'Minimal',
            fieldMappings: {},
            sampleHeaders: [],
        });
    });

    test('rejects invalid identifiers and blank template names', () => {
        expect(normalizeImportConfigMatchResult({ id: Number.NaN, name: 'name' })).toBeNull();
        expect(normalizeImportConfigMatchResult({ id: Number.POSITIVE_INFINITY, name: 'name' })).toBeNull();
        expect(normalizeImportConfigMatchResult({ id: 1, name: '   ' })).toBeNull();
        expect(normalizeImportConfigMatchResult({ id: 1, name: 42 as never })).toBeNull();
    });

    test('recognizes structured provider filenames and resolves spreadsheet formats', () => {
        for (const name of ['微信支付账单.csv', 'wechat-bill.CSV', 'WXPAY.txt', '支付宝交易明细.xlsx', 'ALIPAY.xls']) {
            expect(looksLikeStructuredBillStatementFile(file(name))).toBe(true);
        }
        expect(looksLikeStructuredBillStatementFile(file('bank-statement.csv'))).toBe(false);
        expect(looksLikeStructuredBillStatementFile()).toBe(false);

        expect(resolveImportConfigFileFormat(file('bill.CSV'))).toBe('csv');
        expect(resolveImportConfigFileFormat(file('bill.txt'))).toBe('csv');
        expect(resolveImportConfigFileFormat(file('bill.XLSX'))).toBe('excel');
        expect(resolveImportConfigFileFormat(file('bill.xls'))).toBe('excel');
        expect(resolveImportConfigFileFormat(file('bill.json'))).toBe('csv');
        expect(resolveImportConfigFileFormat()).toBe('csv');
    });

    test('uses the active unmatched file for template naming and format detection', () => {
        expect(resolveActiveImportSource(file('batch.csv'), { originalName: 'current.XLSX' })).toEqual({
            fileName: 'current.XLSX',
            fileFormat: 'excel',
        });
        expect(resolveActiveImportSource(file('batch.txt'))).toEqual({
            fileName: 'batch.txt',
            fileFormat: 'csv',
        });
        expect(resolveActiveImportSource()).toEqual({
            fileName: 'import',
            fileFormat: 'csv',
        });
    });

    test('maps stage-one unmatched files and reports unsupported legacy batches', () => {
        const generic = { original_name: 'generic.csv', temp_path: 'generic.tmp' };
        expect(resolveUnmatchedImportFiles([generic])).toEqual({
            errorMessage: '',
            queue: [{ originalName: 'generic.csv', tempPath: 'generic.tmp', reason: undefined }]
        });

        const legacyReason = 'This presentation text may change without changing the contract';
        expect(resolveUnmatchedImportFiles([
            generic,
            {
                original_name: 'old-a.xls', temp_path: 'a.tmp', reason: legacyReason,
                error_code: 'unsupported_legacy_xls'
            },
            {
                original_name: 'old-b.xls', temp_path: 'b.tmp', reason: legacyReason,
                errorCode: 'unsupported_legacy_xls'
            }
        ])).toMatchObject({
            errorMessage: '不支持旧版二进制 XLS：old-a.xls、old-b.xls。请先转换为 XLSX 或 CSV 后重新导入。'
        });
    });

    test('formats match messages and display descriptions by documented priority', () => {
        const config = normalizeImportConfigMatchResult({ id: 1, name: 'Template' })!;
        expect(getMatchedImportConfigMessage(config)).toBe('已自动套用模板：Template');
        expect(getMatchedImportConfigMessage({ ...config, matchReason: 'default_template_fallback' }))
            .toBe('已自动回退到默认模板：Template');

        expect(getImportConfigDisplayDescription({
            description: 'full', descriptionSummary: 'summary', sampleHeaders: ['a', 'b'],
        })).toBe('full');
        expect(getImportConfigDisplayDescription({ descriptionSummary: 'summary', sampleHeaders: ['a', 'b'] }))
            .toBe('summary');
        expect(getImportConfigDisplayDescription({ sampleHeaders: ['a', 'b'] })).toBe('a / b');
        expect(getImportConfigDisplayDescription({ sampleHeaders: [] })).toBe('');
        expect(getImportConfigDisplayDescription(null)).toBe('');
        expect(getImportConfigDisplayDescription(undefined)).toBe('');
    });
});

describe('import flow progress', () => {
    test('maps every dialog step to ordered active and completed progress items', () => {
        const harness = progressHarness();
        const cases = [
            ['uploadFile', false, 'selectSource', 0, 20],
            ['uploadFile', true, 'parseStageRows', 1, 40],
            ['defineColumn', false, 'parseStageRows', 1, 40],
            ['executeCustomScript', false, 'parseStageRows', 1, 40],
            ['checkData', false, 'reviewPreview', 2, 60],
            ['checkData', true, 'confirmImport', 3, 80],
            ['finalResult', false, 'result', 4, 100],
            ['unexpected', false, 'selectSource', 0, 20],
        ] as const;

        for (const [step, submitting, key, index, value] of cases) {
            harness.currentStep.value = step;
            harness.submitting.value = submitting;
            expect(harness.progress.currentImportFlowProgressKey.value).toBe(key);
            expect(harness.progress.currentFlowProgressIndex.value).toBe(index);
            expect(harness.progress.currentFlowProgressValue.value).toBe(value);
            expect(harness.progress.currentFlowProgressItem.value.key).toBe(key);
            expect(harness.progress.importFlowProgressItems.value).toHaveLength(5);
            expect(harness.progress.importFlowProgressItems.value.filter(item => item.active)).toHaveLength(1);
            expect(harness.progress.importFlowProgressItems.value.filter(item => item.complete)).toHaveLength(index);
        }

        expect(harness.progress.importFlowProgressItems.value.map(item => item.title)).toEqual([
            'tt:Select File',
            'tt:Parser / tt:Define Columns',
            'tt:Check Data',
            'tt:Confirm tt:Import',
            'tt:Import Result',
        ]);
    });

    test('derives source and parser details from files, queue position and matched templates', () => {
        const harness = progressHarness();
        expect(harness.progress.currentFlowProgressDetail.value).toBe('statement.csv');
        harness.fileName.value = '';
        expect(harness.progress.currentFlowProgressDetail.value).toBe('.csv / .xlsx');

        harness.currentStep.value = 'defineColumn';
        expect(harness.progress.currentFlowProgressDetail.value).toBe('tt:Parser');
        harness.matchedImportConfig.value = { id: 1, name: 'Matched Template' };
        expect(harness.progress.currentFlowProgressDetail.value).toBe('Matched Template');

        harness.unmatchedFilesQueue.value = [{ originalName: 'one.csv' }, { originalName: 'two.csv' }];
        harness.currentUnmatchedIndex.value = 1;
        expect(harness.progress.currentFlowProgressDetail.value).toBe('two.csv (2/2)');
        harness.currentUnmatchedIndex.value = 9;
        expect(harness.progress.currentFlowProgressDetail.value).toBe(' (10/2)');
    });

    test('formats preview, confirmation and result details with zero fallbacks', () => {
        const harness = progressHarness();
        harness.currentStep.value = 'checkData';
        expect(harness.progress.currentFlowProgressDetail.value).toBe('tt:Import Preview');
        harness.previewTotalCount.value = 12;
        expect(harness.progress.currentFlowProgressDetail.value)
            .toBe('format.misc.previewCount:{"count":"count:12"}');

        harness.submitting.value = true;
        expect(harness.progress.currentFlowProgressDetail.value).toBe('tt:Confirm');
        harness.importProcess.value = 37.456;
        expect(harness.progress.currentFlowProgressDetail.value)
            .toBe('format.misc.importingTransactions:{"process":"number:37.456:2"}');

        harness.currentStep.value = 'finalResult';
        expect(harness.progress.currentFlowProgressDetail.value)
            .toBe('format.misc.importTransactionResult:{"count":"count:0"}');
        harness.importedCount.value = 23;
        expect(harness.progress.currentFlowProgressDetail.value)
            .toBe('format.misc.importTransactionResult:{"count":"count:23"}');
    });
});
