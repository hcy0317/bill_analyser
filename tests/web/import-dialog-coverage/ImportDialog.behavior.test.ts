import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockTemplateRefs = new Map<string, any>();
const mockTemplateEvents: Array<{ name: string; handler: (...args: any[]) => any }> = [];
const mockFetchImportStage = jest.fn<(...args: any[]) => Promise<any>>();
const mockOpenImportFileDialog = jest.fn<(...args: any[]) => Promise<void>>();
const mockGetCurrentToken = jest.fn<() => string>();
const mockLogMilestone = jest.fn();
const mockLogger = {
    debug: jest.fn(),
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn()
};
const mockServices = {
    getImportConfigs: jest.fn<(...args: any[]) => Promise<any>>(),
    saveImportConfig: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteImportConfig: jest.fn<(...args: any[]) => Promise<any>>(),
    parseGenericIntoSession: jest.fn<(...args: any[]) => Promise<any>>(),
    previewImportFileFromTemp: jest.fn<(...args: any[]) => Promise<any>>(),
    matchImportConfig: jest.fn<(...args: any[]) => Promise<any>>(),
    suggestImportConfig: jest.fn<(...args: any[]) => Promise<any>>()
};

const mockAccountStore = {
    allVisiblePlainAccounts: [{ id: 'wallet' }, { id: 'bank' }],
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>(),
    updateAccountListInvalidState: jest.fn()
};
const mockCategoryStore = {
    allTransactionCategories: { 4: [{ id: 'transfer' }] },
    allTransactionCategoriesMap: {
        food: { id: 'food', type: 3, parentId: 'expense' },
        transfer: { id: 'transfer', type: 4, parentId: '' }
    },
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>()
};
const mockTagStore = { loadAllTags: jest.fn<(...args: any[]) => Promise<void>>() };
const mockTransactionStore = { updateTransactionListInvalidState: jest.fn() };
const mockOverviewStore = { updateTransactionOverviewInvalidState: jest.fn() };
const mockStatisticsStore = { updateTransactionStatisticsInvalidState: jest.fn() };
const mockSettingsStore = { appSettings: { timeZone: 'Asia/Shanghai' } };
const mockUserStore = { currentUserCashTransferCategoryId: 'cash-transfer' };
const mockSourceSelection = {
    allFileSubTypes: [] as any[],
    exportFileGuideDocumentLanguageName: '',
    exportFileGuideDocumentUrl: '',
    fileType: 'csv',
    isImportDataFromTextbox: false,
    showHandlingMethodSelector: false,
    supportedImportFileExtensions: '.csv'
};

const mockBuildTransaction = jest.fn((item: any, index: number) => ({
    _previewId: Number(item.id ?? item.preview_id ?? index + 1),
    categoryId: item.categoryId ?? 'food',
    matching: item.matching ?? {},
    selected: item.selected ?? true,
    type: item.type ?? 3,
    valid: item.valid ?? true,
    toCreateRequest: jest.fn(() => ({ amountCents: item.amountCents ?? -1234 }))
}));
const mockBuildPreviewUpdate = jest.fn((transaction: any, options: any) => ({
    id: transaction._previewId,
    selected: transaction.selected,
    amount_cents: -1234,
    category_path: options.categoryPath,
    clear_transfer_decision: options.clearTransferDecision
}));
const mockBuildHistoryOperation = jest.fn((previewId: number | null, reconciliation: any) => {
    if (previewId === null || !reconciliation.reconciliationDestructiveAckRequired) return null;
    return {
        preview_id: previewId,
        planned_operation: reconciliation.reconciliationPlannedOperation,
        history_bill_id: reconciliation.reconciliationHistoryBillId,
        history_bill_version: reconciliation.reconciliationHistoryBillVersion,
        operation_id: reconciliation.reconciliationOperationId,
        acknowledgement_token: reconciliation.reconciliationAcknowledgementToken
    };
});
const mockBuildHistoryAck = jest.fn((input: any) => input.operations.length > 0 ? ({
    preview_ids: input.selectedPreviewIds,
    operations: input.operations,
    selection_scope: input.selectionScope
}) : null);

const mockCoordinator = {
    abort: jest.fn(),
    begin: jest.fn<(key: string) => any>((key: string) => ({ key, controller: new AbortController() })),
    finish: jest.fn(),
    isCurrent: jest.fn(() => true)
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as Record<string, unknown>;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const value = (actual['ref'] as (value: unknown) => any)(null);
            mockTemplateRefs.set(name, value);
            return value;
        }
    };
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => `#${value}` }),
        formatNumberToLocalizedNumerals: (value: number) => String(value)
    })
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoryStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionStore }));
jest.mock('@/stores/overview.ts', () => ({ useOverviewStore: () => mockOverviewStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockStatisticsStore }));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => mockGetCurrentToken() }));
jest.mock('@/lib/importFileDialog.ts', () => ({ openImportFileDialog: (...args: any[]) => mockOpenImportFileDialog(...args) }));
jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

jest.mock('@/views/desktop/transactions/import/importDialogApi.ts', () => ({
    extractApiErrorMessage: (error: any, fallback: string) => error?.error || error?.message || fallback,
    fetchImportStage: (...args: any[]) => mockFetchImportStage(...args),
    isAbortError: (error: any) => error?.name === 'AbortError'
}));
jest.mock('@/views/desktop/transactions/import/importPreviewTransaction.ts', () => ({
    buildImportTransactionFromPreviewRecord: (...args: any[]) => mockBuildTransaction(args[0], args[1])
}));
jest.mock('@/views/desktop/transactions/import/importPreviewUpdates.ts', () => ({
    buildImportPreviewUpdateFromTransaction: (...args: any[]) => mockBuildPreviewUpdate(args[0], args[1]),
    getPreviewIdFromImportTransaction: (transaction: any) => Number.isFinite(transaction?._previewId)
        ? transaction._previewId
        : null,
    getPreviewUpdateId: (update: any) => Number.isFinite(update?.id) ? update.id : null
}));
jest.mock('@/views/desktop/transactions/import/importPreview.ts', () => ({
    resolveImportPreviewCategoryPath: (categoryId: string, categories: Record<string, any>) => categories[categoryId] || null
}));
jest.mock('@/views/desktop/transactions/import/checkDataMatching.ts', () => ({
    buildImportPreviewHistoryRewriteAcknowledgement: (...args: any[]) => mockBuildHistoryAck(args[0]),
    buildImportPreviewHistoryRewriteOperationAcknowledgement: (...args: any[]) => mockBuildHistoryOperation(args[0], args[1])
}));
jest.mock('@/views/desktop/transactions/import/decisionPreviewReplacement.ts', () => ({
    applyServerPagedReclassification: (serverPaged: boolean, reload: () => void) => {
        if (!serverPaged) return false;
        reload();
        return true;
    },
    applyNonServerPagedReplacement: (current: any[], removedIds: number[], replacements: any[], getId: (item: any) => number | null) => [
        ...current.filter(item => !removedIds.includes(getId(item) ?? -1)),
        ...replacements
    ]
}));
jest.mock('@/views/desktop/transactions/import/import-dialog/importConfigHelpers.ts', () => ({
    getImportConfigDisplayDescription: (config: any) => config?.descriptionSummary || config?.description || '',
    getMatchedImportConfigMessage: (config: any) => `matched:${config.name || 'template'}`,
    normalizeImportConfigMatchResult: (config: any) => config?.id ? config : null,
    resolveActiveImportSource: (file: File | undefined, queuedFile: { originalName?: string } | undefined) => {
        const fileName = queuedFile?.originalName || file?.name || 'import';
        return { fileName, fileFormat: /\.xlsx?$/i.test(fileName) ? 'excel' : 'csv' };
    },
    resolveImportConfigFileFormat: (file: File | undefined) => /\.xlsx?$/i.test(file?.name || '') ? 'excel' : 'csv',
    resolveUnmatchedImportFiles: (files: Array<{
        original_name: string,
        temp_path: string,
        reason?: string,
        error_code?: string,
        errorCode?: string
    }>) => {
        const unsupported = files.filter(file =>
            (file.error_code || file.errorCode) === 'unsupported_legacy_xls'
        );
        return {
            errorMessage: unsupported.length > 0
                ? `不支持旧版二进制 XLS：${unsupported.map(file => file.original_name).join('、')}。请先转换为 XLSX 或 CSV 后重新导入。`
                : '',
            queue: files.map(file => ({
                originalName: file.original_name,
                tempPath: file.temp_path,
                reason: file.reason
            }))
        };
    }
}));
jest.mock('@/views/desktop/transactions/import/import-dialog/previewPageQuery.ts', () => ({
    appendPreviewPageFilters: (search: URLSearchParams, filters: Record<string, unknown> | undefined) => {
        if (filters?.['signal']) search.set('signal', String(filters['signal']));
    },
    buildCanonicalPreviewPageRequestKey: (...args: any[]) => JSON.stringify(args),
    normalizePreviewPageSortBy: (value: unknown) => typeof value === 'string' ? value.trim() : '',
    normalizePreviewPageSortDirection: (value: unknown) => value === 'desc' ? 'desc' : 'asc'
}));
jest.mock('@/views/desktop/transactions/import/import-dialog/previewPageRequestCoordinator.ts', () => ({
    PreviewPageRequestCoordinator: class {
        abort = mockCoordinator.abort;
        begin = mockCoordinator.begin;
        finish = mockCoordinator.finish;
        isCurrent = mockCoordinator.isCurrent;
    }
}));
jest.mock('@/views/desktop/transactions/import/import-dialog/importFlowProfiler.ts', () => ({
    createImportFlowMilestoneLogger: () => mockLogMilestone
}));
jest.mock('@/views/desktop/transactions/import/import-dialog/useImportFlowProgress.ts', () => {
    const { computed } = jest.requireActual('vue') as any;
    return {
        useImportFlowProgress: () => ({
            currentFlowProgressDetail: computed(() => ''),
            currentFlowProgressIndex: computed(() => 0),
            currentFlowProgressItem: computed(() => null),
            currentFlowProgressValue: computed(() => 0),
            importFlowProgressItems: computed(() => [])
        })
    };
});
jest.mock('@/views/desktop/transactions/import/import-dialog/useImportCheckDataFilterMenu.ts', () => ({
    useImportCheckDataFilterMenu: () => ({ isActiveCheckDataFilterGroup: () => false })
}));
jest.mock('@/views/desktop/transactions/import/import-dialog/useImportSourceSelection.ts', () => {
    const { computed } = jest.requireActual('vue') as any;
    return {
        useImportSourceSelection: () => ({
            allFileSubTypes: computed(() => mockSourceSelection.allFileSubTypes),
            exportFileGuideDocumentLanguageName: computed(() => mockSourceSelection.exportFileGuideDocumentLanguageName),
            exportFileGuideDocumentUrl: computed(() => mockSourceSelection.exportFileGuideDocumentUrl),
            fileType: computed(() => mockSourceSelection.fileType),
            isImportDataFromTextbox: computed(() => mockSourceSelection.isImportDataFromTextbox),
            showHandlingMethodSelector: computed(() => mockSourceSelection.showHandlingMethodSelector),
            supportedImportFileExtensions: computed(() => mockSourceSelection.supportedImportFileExtensions)
        })
    };
});

for (const componentPath of [
    '@/components/desktop/ConfirmDialog.vue',
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/transactions/import/tabs/ImportTransactionDefineColumnTab.vue',
    '@/views/desktop/transactions/import/tabs/ImportTransactionExecuteCustomScriptTab.vue',
    '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue',
    '@/views/desktop/transactions/import/import-dialog/ImportFlowProgress.vue',
    '@/views/desktop/transactions/import/import-dialog/ImportCheckDataFilterButton.vue'
]) {
    jest.mock(componentPath, () => {
        const { defineComponent, h } = jest.requireActual('vue') as any;
        return {
            __esModule: true,
            default: defineComponent({
                name: 'ImportDialogCoverageStub',
                inheritAttrs: false,
                setup: (_props: unknown, { attrs, slots }: any) => {
                    for (const [name, handler] of Object.entries(attrs)) {
                        if (name.startsWith('on') && typeof handler === 'function') {
                            mockTemplateEvents.push({ name, handler: handler as (...args: any[]) => any });
                        }
                    }
                    return () => h('div', attrs, Object.values(slots).flatMap(slot => (
                        typeof slot === 'function' ? (slot as () => unknown[])() : []
                    )));
                }
            })
        };
    });
}

const ImportDialog = require('@/views/desktop/transactions/import/ImportDialog.vue').default as any;

function createResponse({ ok = true, result = {}, text = 'failed' }: {
    ok?: boolean;
    result?: any;
    text?: string;
} = {}): any {
    return {
        ok,
        json: jest.fn(async () => result),
        text: jest.fn(async () => text)
    };
}

function createConfig(overrides: Record<string, unknown> = {}): any {
    return {
        id: 7,
        name: 'CSV template',
        fileFormat: 'csv',
        description: 'description',
        descriptionSummary: 'summary',
        fieldMappings: { columnMapping: { amount: 1 }, includeHeader: true },
        dateFormat: '',
        encoding: 'utf-8',
        delimiter: ',',
        skipRows: 0,
        hasHeader: true,
        customRules: {},
        sampleHeaders: ['date', 'amount'],
        isDefault: false,
        defaultRecommendation: true,
        ...overrides
    };
}

function createBindings(): any {
    mockTemplateRefs.clear();
    const exposed: Record<string, unknown> = {};
    const bindings = ImportDialog.setup({ persistent: false }, {
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value)
    });
    expect(exposed).toHaveProperty('open');
    return bindings;
}

function setTemplateRef(name: string, value: unknown): void {
    const target = mockTemplateRefs.get(name);
    if (!target) throw new Error(`missing template ref: ${name}`);
    target.value = value;
}

function createSnackbar(): any {
    return { showError: jest.fn(), showMessage: jest.fn() };
}

async function flushAsync(): Promise<void> {
    await Promise.resolve();
    await new Promise(resolve => setImmediate(resolve));
}

async function renderDialog(mutator: (bindings: any) => void): Promise<string> {
    const { createSSRApp, defineComponent, h } = require('vue') as any;
    const { renderToString } = require('vue/server-renderer') as any;
    let bindings: any;
    const RuntimeImportDialog = {
        ...ImportDialog,
        setup(props: any, context: any) {
            bindings = ImportDialog.setup(props, context);
            mutator(bindings);
            return bindings;
        }
    };
    const app = createSSRApp(RuntimeImportDialog, { persistent: true });
    const UiStub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [name, handler] of Object.entries(attrs)) {
                if (name.startsWith('on') && typeof handler === 'function') {
                    mockTemplateEvents.push({ name, handler: handler as (...args: any[]) => any });
                }
            }
            return () => h(
                'div',
                attrs,
                Object.values(slots).flatMap(slot => typeof slot === 'function' ? (slot as () => unknown[])() : [])
            );
        }
    });
    for (const name of [
        'v-dialog', 'v-card', 'v-btn', 'v-icon', 'v-menu', 'v-list', 'v-list-item', 'v-divider',
        'v-window', 'v-window-item', 'v-row', 'v-col', 'v-select', 'v-text-field', 'v-textarea',
        'v-progress-circular', 'v-card-title', 'v-card-text', 'v-checkbox', 'v-card-actions'
    ]) app.component(name, UiStub);
    app.config.warnHandler = () => undefined;
    const html = await renderToString(app);
    expect(bindings).toBeDefined();
    return html;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateEvents.length = 0;
    mockGetCurrentToken.mockReturnValue('token-1');
    mockAccountStore.loadAllAccounts.mockResolvedValue();
    mockCategoryStore.loadAllCategories.mockResolvedValue();
    mockTagStore.loadAllTags.mockResolvedValue();
    mockServices.getImportConfigs.mockResolvedValue({ data: { result: [] } });
    mockServices.saveImportConfig.mockResolvedValue({ data: { success: true, result: { id: 11 } } });
    mockServices.deleteImportConfig.mockResolvedValue({ data: { success: true } });
    mockServices.parseGenericIntoSession.mockResolvedValue({ data: { success: true, result: { parsed_count: 1 } } });
    mockServices.previewImportFileFromTemp.mockResolvedValue({
        data: { result: { sampleData: [['date', 'amount'], ['2026-07-10', '-12.34']], delimiter: ',' } }
    });
    mockServices.matchImportConfig.mockResolvedValue({ data: { success: false } });
    mockServices.suggestImportConfig.mockResolvedValue({ data: { result: {} } });
    mockCoordinator.begin.mockImplementation((key: string) => ({ key, controller: new AbortController() }));
    mockCoordinator.isCurrent.mockReturnValue(true);
    Object.assign(mockSourceSelection, {
        allFileSubTypes: [],
        exportFileGuideDocumentLanguageName: '',
        exportFileGuideDocumentUrl: '',
        fileType: 'csv',
        isImportDataFromTextbox: false,
        showHandlingMethodSelector: false,
        supportedImportFileExtensions: '.csv'
    });
    (globalThis as any).fetch = jest.fn();
});

describe('ImportDialog production-loaded behavior coverage', () => {
    test('loads the production SFC setup and exposes the lifecycle actions', () => {
        const bindings = createBindings();

        expect(bindings).toEqual(expect.objectContaining({
            open: expect.any(Function),
            parseData: expect.any(Function),
            executeStage2Dedup: expect.any(Function),
            submit: expect.any(Function),
            close: expect.any(Function)
        }));
        expect(bindings.fileName.value).toBe('');
        expect(bindings.getDisplayCount(12)).toBe('#12');
        expect(bindings.isActiveCheckDataFilterGroup('All')).toBe(false);
        expect(createResponse({ result: { success: true } }).ok).toBe(true);
        expect(createConfig()).toMatchObject({ id: 7, delimiter: ',' });
        setTemplateRef('snackbar', createSnackbar());
        expect(mockTemplateRefs.get('snackbar').value).toEqual(expect.objectContaining({
            showError: expect.any(Function)
        }));
    });

    test('renders every dialog step and auxiliary template branch through the production render function', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        Object.assign(mockSourceSelection, {
            allFileSubTypes: [{ displayName: 'CSV', type: 'csv' }],
            exportFileGuideDocumentLanguageName: 'English',
            exportFileGuideDocumentUrl: 'https://example.test/guide',
            fileType: 'dsv',
            showHandlingMethodSelector: true
        });
        const uploadHtml = await renderDialog(bindings => {
            bindings.loading.value = false;
            bindings.importFiles.value = [new File(['a'], 'a.csv')];
        });
        expect(uploadHtml).toContain('desktop.import.dialog');
        expect(uploadHtml).toContain('How to import this file?');

        mockSourceSelection.isImportDataFromTextbox = true;
        mockSourceSelection.fileType = 'json';
        const textboxHtml = await renderDialog(bindings => {
            bindings.loading.value = false;
            bindings.importData.value = 'row';
        });
        expect(textboxHtml).toContain('Data to import');
        expect(textboxHtml).toContain('How to export this file?');

        const config = createConfig();
        const defineHtml = await renderDialog(bindings => {
            bindings.loading.value = false;
            bindings.currentStep.value = 'defineColumn';
            bindings.parsedFileData.value = [['amount']];
            bindings.importFiles.value = [new File(['a'], 'a.csv')];
            bindings.importTransactionDefineColumnTab.value = {
                menus: [{ prependIcon: 'menu', title: 'Mapping menu', disabled: false, onClick: jest.fn() }]
            };
            bindings.showSaveImportConfigDialog.value = true;
            bindings.saveImportConfigName.value = 'Template';
            bindings.saveImportConfigRecommended.value = true;
            bindings.showManageImportConfigDialog.value = true;
            bindings.importConfigList.value = [config];
            bindings.matchedImportConfig.value = config;
            bindings.showEditImportConfigDialog.value = true;
            bindings.editImportConfigName.value = 'Edited';
            bindings.editImportConfigRecommended.value = true;
        });
        expect(defineHtml).toContain('Mapping menu');
        expect(defineHtml).toContain('Saved Templates');

        const scriptHtml = await renderDialog(bindings => {
            bindings.loading.value = false;
            bindings.currentStep.value = 'executeCustomScript';
            bindings.importFiles.value = [new File(['a'], 'a.csv')];
            bindings.importTransactionExecuteCustomScriptTab.value = {
                menus: [{ prependIcon: 'script', title: 'Script menu', disabled: false, onClick: jest.fn() }]
            };
        });
        expect(scriptHtml).toContain('Script menu');

        const checkHtml = await renderDialog(bindings => {
            bindings.loading.value = false;
            bindings.currentStep.value = 'checkData';
            bindings.importTransactionCheckDataTab.value = {
                canImport: true,
                filterMenus: [{ title: 'Signals', summary: 'Learning' }],
                isEditing: false,
                toolMenus: [
                    { title: 'Review', subTitle: 'Selected', prependIcon: 'tool', appendIcon: 'next', disabled: false },
                    { title: 'After divider', divider: true, disabled: true }
                ]
            };
        });
        expect(checkHtml).toContain('Review');
        expect(checkHtml).toContain('Import');

        const resultHtml = await renderDialog(bindings => {
            bindings.loading.value = false;
            bindings.currentStep.value = 'finalResult';
            bindings.importedCount.value = 2;
        });
        expect(resultHtml).toContain('Data Import Completed');
        expect(resultHtml).toContain('Close');

        for (const event of mockTemplateEvents) {
            try {
                let result: any;
                if (event.name.startsWith('onUpdate:')) {
                    result = event.handler(false);
                } else if (event.name === 'onReclassified') {
                    result = event.handler([]);
                } else if (event.name === 'onRequestPage') {
                    result = event.handler(1, 10, {});
                } else {
                    result = event.handler({ target: { files: [] } });
                }
                if (result && typeof result.then === 'function') {
                    await result.catch(() => undefined);
                }
            } catch {
                // The handler itself is the coverage target; stateful actions are verified in dedicated tests.
            }
        }
        await flushAsync();
        warnSpy.mockRestore();
    });

    test('opens, resets, closes, and reports essential-store failures', async () => {
        const bindings = createBindings();
        const defineTab = { reset: jest.fn() };
        const scriptTab = { reset: jest.fn() };
        const checkTab = { reset: jest.fn() };
        setTemplateRef('importTransactionDefineColumnTab', defineTab);
        setTemplateRef('importTransactionExecuteCustomScriptTab', scriptTab);
        setTemplateRef('importTransactionCheckDataTab', checkTab);

        bindings.serverSessionId.value = 'old-session';
        (globalThis.fetch as any).mockResolvedValue(createResponse());
        const opened = bindings.open();
        await flushAsync();
        expect(bindings.showState.value).toBe(true);
        expect(bindings.loading.value).toBe(false);
        expect(mockAccountStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(defineTab.reset).toHaveBeenCalled();
        expect(scriptTab.reset).toHaveBeenCalled();
        expect(checkTab.reset).toHaveBeenCalled();
        expect(mockCoordinator.abort).toHaveBeenCalled();
        bindings.close(true);
        await expect(opened).resolves.toBeUndefined();
        expect(bindings.showState.value).toBe(false);

        const failureBindings = createBindings();
        const loadError = Object.assign(new Error('load failed'), { processed: false });
        mockAccountStore.loadAllAccounts.mockRejectedValueOnce(loadError);
        const failedOpen = failureBindings.open();
        await expect(failedOpen).rejects.toThrow('load failed');
        expect(failureBindings.showState.value).toBe(false);
        expect(mockLogger.error).toHaveBeenCalledWith(
            'failed to load essential data for importing transaction',
            loadError
        );
    });

    test('selects import files through both input paths and computes display names', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        setTemplateRef('snackbar', snackbar);

        bindings.submitting.value = true;
        await bindings.showOpenFileDialog();
        expect(mockOpenImportFileDialog).not.toHaveBeenCalled();
        bindings.submitting.value = false;
        await bindings.showOpenFileDialog();
        expect(mockOpenImportFileDialog).toHaveBeenCalledWith(expect.objectContaining({
            accept: '.csv',
            onFilesSelected: expect.any(Function)
        }));

        bindings.setImportFile(undefined as unknown as Event);
        bindings.setImportFile({ target: null } as unknown as Event);
        bindings.setImportFile({ target: { files: [] } } as unknown as Event);
        const first = new File(['one'], 'first.csv');
        const second = new File(['two'], 'second.csv');
        const input = { files: [first], value: 'selected' };
        bindings.setImportFile({ target: input } as unknown as Event);
        expect(input.value).toBe('');
        expect(bindings.fileName.value).toBe('first.csv');

        bindings.setSelectedImportFiles([first, second]);
        expect(bindings.fileName.value).toBe('2 files selected');
        expect(bindings.processDSVMethod.value).toBe(0);
        expect(mockLogMilestone).toHaveBeenCalledWith('files_selected_at', {
            file_count: 2,
            total_size_bytes: 6
        });
        expect(bindings.activeImportSource.value.fileFormat).toBe('csv');
    });

    test('loads, applies, edits, saves, and removes mapping templates across success and failure paths', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        const mapping = {
            columnMapping: { amount: 1 },
            transactionTypeMapping: { Expense: 3 },
            includeHeader: true,
            timeFormat: 'YYYY-MM-DD',
            timezoneFormat: '',
            amountDecimalSeparator: '.',
            amountDigitGroupingSymbol: ','
        };
        const defineTab = {
            applyFieldMappings: jest.fn(),
            generateResult: jest.fn<(...args: any[]) => any>(() => mapping),
            reset: jest.fn()
        };
        const confirmDialog = { open: jest.fn(async () => true) };
        setTemplateRef('snackbar', snackbar);
        setTemplateRef('confirmDialog', confirmDialog);
        setTemplateRef('importTransactionDefineColumnTab', defineTab);
        bindings.importFiles.value = [new File(['a'], 'input.csv')];
        bindings.parsedFileData.value = [['date', 'amount'], ['2026-07-10', '-12.34']];

        const config = createConfig();
        mockServices.getImportConfigs.mockResolvedValueOnce({ data: { result: [{}, config] } });
        await bindings.openManageImportConfigDialog();
        expect(bindings.importConfigList.value).toEqual([config]);
        expect(bindings.showManageImportConfigDialog.value).toBe(true);
        bindings.applyImportConfig(config);
        expect(defineTab.applyFieldMappings).toHaveBeenCalledWith(config.fieldMappings);
        expect(bindings.parsedFileDelimiter.value).toBe(',');

        bindings.openEditImportConfigDialog(config);
        expect(bindings.editImportConfigName.value).toBe('CSV template');
        expect(bindings.editImportConfigRecommended.value).toBe(true);
        bindings.editImportConfigName.value = ' Edited ';
        bindings.editImportConfigDescription.value = ' Changed ';
        bindings.matchedImportConfig.value = config;
        mockServices.getImportConfigs.mockResolvedValue({ data: { result: [config] } });
        await bindings.saveEditedImportConfig();
        expect(mockServices.saveImportConfig).toHaveBeenCalledWith(expect.objectContaining({
            id: 7,
            name: 'Edited',
            description: 'Changed',
            hasHeader: true
        }));
        expect(bindings.matchedImportConfig.value.name).toBe('Edited');
        expect(snackbar.showMessage).toHaveBeenCalledWith('模板已更新');

        bindings.editingImportConfig.value = null;
        await bindings.saveEditedImportConfig();
        bindings.editingImportConfig.value = config;
        bindings.editImportConfigName.value = '   ';
        await bindings.saveEditedImportConfig();
        bindings.editImportConfigName.value = 'Broken';
        mockServices.saveImportConfig.mockResolvedValueOnce({ data: { success: false } });
        await bindings.saveEditedImportConfig();
        mockServices.saveImportConfig.mockRejectedValueOnce(new Error('save unavailable'));
        await bindings.saveEditedImportConfig();
        expect(snackbar.showError).toHaveBeenCalledWith('Unable to update saved template');

        bindings.matchedImportConfig.value = config;
        bindings.removeImportConfig(config);
        await flushAsync();
        expect(mockServices.deleteImportConfig).toHaveBeenCalledWith({ id: 7 });
        expect(bindings.matchedImportConfig.value).toBeNull();
        mockServices.deleteImportConfig.mockResolvedValueOnce({ data: { success: false } });
        bindings.removeImportConfig(config);
        await flushAsync();
        mockServices.deleteImportConfig.mockRejectedValueOnce(new Error('delete unavailable'));
        bindings.removeImportConfig(config);
        await flushAsync();
        expect(snackbar.showError).toHaveBeenCalledWith('Unable to delete saved template');

        mockServices.getImportConfigs.mockRejectedValueOnce(new Error('list unavailable'));
        await bindings.openManageImportConfigDialog();
        expect(snackbar.showError).toHaveBeenCalledWith('Unable to load saved templates');

        mockServices.getImportConfigs.mockResolvedValueOnce({ data: { result: [] } });
        bindings.openSaveImportConfigDialog();
        await flushAsync();
        expect(bindings.showSaveImportConfigDialog.value).toBe(true);
        expect(bindings.saveImportConfigRecommended.value).toBe(true);
        bindings.saveImportConfigName.value = ' New template ';
        bindings.saveImportConfigDescription.value = ' Saved description ';
        bindings.parsedFileEncoding.value = 'gb18030';
        await bindings.saveCurrentImportConfig();
        expect(mockServices.saveImportConfig).toHaveBeenLastCalledWith(expect.objectContaining({
            name: 'New template',
            fileFormat: 'csv',
            sampleHeaders: ['date', 'amount'],
            dateFormat: 'YYYY-MM-DD',
            encoding: 'gb18030',
            skipRows: 0,
            customRules: {}
        }));
        expect(bindings.matchedImportConfig.value).toMatchObject({ id: 11, name: 'New template' });

        defineTab.generateResult.mockReturnValueOnce(null);
        await bindings.saveCurrentImportConfig();
        bindings.parsedFileData.value = [];
        await bindings.saveCurrentImportConfig();
        bindings.parsedFileData.value = [['amount']];
        mockServices.saveImportConfig.mockRejectedValueOnce(new Error('save unavailable'));
        await bindings.saveCurrentImportConfig();
        expect(snackbar.showError).toHaveBeenCalledWith('Unable to save data mapping file');

        bindings.parsedFileData.value = undefined;
        bindings.openSaveImportConfigDialog();
        mockServices.getImportConfigs.mockRejectedValueOnce(new Error('list unavailable'));
        bindings.parsedFileData.value = [['amount']];
        bindings.openSaveImportConfigDialog();
        await flushAsync();
        expect(snackbar.showError).toHaveBeenCalledWith('Unable to load saved templates');
    });

    test('prepares unmatched files and advances column mapping files into deduplication', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        setTemplateRef('snackbar', snackbar);

        await bindings.executeColumnMappingImport();
        expect(snackbar.showError).toHaveBeenCalledWith('Column mapping tab not ready');

        const mapping = {
            columnMapping: { amount: 1 },
            transactionTypeMapping: { Expense: 3 },
            includeHeader: true,
            timeFormat: 'YYYY-MM-DD',
            timezoneFormat: 'offset',
            amountDecimalSeparator: '.',
            amountDigitGroupingSymbol: ','
        };
        const defineTab = {
            applyFieldMappings: jest.fn(),
            generateResult: jest.fn<(...args: any[]) => any>(),
            reset: jest.fn()
        };
        setTemplateRef('importTransactionDefineColumnTab', defineTab);
        await bindings.executeColumnMappingImport();
        defineTab.generateResult.mockReturnValue(mapping);
        await bindings.executeColumnMappingImport();
        expect(snackbar.showError).toHaveBeenCalledWith('No unmatched file to process');

        bindings.serverSessionId.value = 'session-map';
        bindings.unmatchedFilesQueue.value = [
            { originalName: 'first.csv', tempPath: 'first.tmp' },
            { originalName: 'second.csv', tempPath: 'second.tmp' }
        ];
        bindings.parsedFileEncoding.value = 'gb18030';
        bindings.parsedFileDelimiter.value = ';';
        mockServices.matchImportConfig.mockResolvedValueOnce({
            data: { success: true, result: createConfig({ name: 'Matched' }) }
        });
        await bindings.executeColumnMappingImport();
        expect(mockServices.parseGenericIntoSession).toHaveBeenCalledWith(expect.objectContaining({
            sessionId: 'session-map',
            tempPath: 'first.tmp',
            columnMapping: { amount: 1 },
            transactionTypeMapping: { Expense: 3 },
            hasHeaderLine: true,
            fileEncoding: 'gb18030',
            delimiter: ';'
        }));
        expect(bindings.currentUnmatchedIndex.value).toBe(1);
        expect(bindings.currentStep.value).toBe('defineColumn');
        expect(defineTab.applyFieldMappings).toHaveBeenCalled();
        expect(mockServices.previewImportFileFromTemp).toHaveBeenLastCalledWith({
            sessionId: 'session-map',
            tempPath: 'second.tmp',
            fileEncoding: 'auto',
            delimiter: undefined
        });

        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: true, data: { after_dedup: 2, dedup_stats: { exact: 1 } } }
        }));
        await bindings.executeColumnMappingImport();
        expect(mockFetchImportStage).toHaveBeenLastCalledWith(
            '/api/bills/import/v2/dedup',
            expect.objectContaining({ method: 'POST' }),
            '阶段2去重',
            expect.any(Number)
        );
        expect(bindings.currentStep.value).toBe('checkData');
        expect(bindings.previewTotalCount.value).toBe(2);
        expect(bindings.serverPagedPreviewMode.value).toBe(true);
        expect(bindings.importProcess.value).toBe(100);

        mockServices.parseGenericIntoSession.mockResolvedValueOnce({ data: { success: false, error: 'bad mapping' } });
        await expect(bindings.executeColumnMappingImport()).rejects.toThrow('bad mapping');
    });

    test('covers preview matching, automatic suggestions, empty files, and matching service failures', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        const defineTab = { applyFieldMappings: jest.fn(), reset: jest.fn() };
        setTemplateRef('snackbar', snackbar);
        setTemplateRef('importTransactionDefineColumnTab', defineTab);
        bindings.serverSessionId.value = 'session-preview-mapping';
        bindings.unmatchedFilesQueue.value = [{ originalName: 'empty.xlsx', tempPath: 'empty.tmp' }];

        mockServices.previewImportFileFromTemp.mockResolvedValueOnce({ data: { result: { sampleData: [] } } });
        await bindings.prepareColumnMappingForUnmatchedFile(bindings.unmatchedFilesQueue.value[0]);
        expect(snackbar.showError).toHaveBeenCalledWith('文件 empty.xlsx 没有可导入的数据');

        mockServices.previewImportFileFromTemp.mockResolvedValueOnce({ data: { result: { sampleData: [[]] } } });
        await bindings.prepareColumnMappingForUnmatchedFile(bindings.unmatchedFilesQueue.value[0]);
        expect(bindings.parsedFileData.value).toEqual([[]]);

        const matched = createConfig({ name: 'Auto matched', delimiter: ';', encoding: 'gb18030' });
        mockServices.previewImportFileFromTemp.mockResolvedValueOnce({
            data: { result: { sampleData: [['date', 'amount'], ['2026-07-10', '-12.34']], delimiter: ',', encoding: 'gb18030' } }
        });
        mockServices.matchImportConfig.mockResolvedValueOnce({ data: { success: true, result: matched } });
        await bindings.prepareColumnMappingForUnmatchedFile(bindings.unmatchedFilesQueue.value[0]);
        expect(bindings.matchedImportConfig.value).toEqual(matched);
        expect(mockServices.matchImportConfig).toHaveBeenLastCalledWith({
            fileFormat: 'excel',
            headers: ['date', 'amount']
        });
        expect(bindings.parsedFileDelimiter.value).toBe(';');
        expect(bindings.parsedFileEncoding.value).toBe('gb18030');
        expect(snackbar.showMessage).toHaveBeenCalledWith('matched:Auto matched');

        mockServices.previewImportFileFromTemp.mockResolvedValueOnce({
            data: { result: { sampleData: [['date', 'amount'], ['2026-07-10', '-12.34']], delimiter: ',' } }
        });
        mockServices.matchImportConfig.mockRejectedValueOnce(new Error('match unavailable'));
        mockServices.suggestImportConfig.mockResolvedValueOnce({
            data: { result: { columnMapping: { amount: 1 }, includeHeader: true } }
        });
        await bindings.prepareColumnMappingForUnmatchedFile(bindings.unmatchedFilesQueue.value[0]);
        expect(snackbar.showMessage).toHaveBeenCalledWith('已自动建议列映射');

        mockServices.previewImportFileFromTemp.mockResolvedValueOnce({
            data: { result: { sampleData: [['date'], ['2026-07-10']] } }
        });
        mockServices.matchImportConfig.mockResolvedValueOnce({ data: { success: false } });
        mockServices.suggestImportConfig.mockRejectedValueOnce(new Error('suggest unavailable'));
        await bindings.prepareColumnMappingForUnmatchedFile(bindings.unmatchedFilesQueue.value[0]);
        expect(mockLogger.warn).toHaveBeenCalledWith('failed to suggest import config', expect.any(Error));
    });

    test('executes parser then dedup in order and routes unmatched files to manual mapping', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        const defineTab = { applyFieldMappings: jest.fn(), reset: jest.fn(), generateResult: jest.fn() };
        setTemplateRef('snackbar', snackbar);
        setTemplateRef('importTransactionDefineColumnTab', defineTab);

        await bindings.parseData();
        expect(snackbar.showError).toHaveBeenCalledWith('Please select at least one file');

        bindings.importFiles.value = [new File(['amount\n-12.34'], 'matched.csv')];
        mockFetchImportStage
            .mockResolvedValueOnce(createResponse({
                result: { success: true, data: { session_id: 'session-direct', parsed_count: 1, unmatched_files: [] } }
            }))
            .mockResolvedValueOnce(createResponse({
                result: { success: true, data: { preview_count: 1, dedup_stats: {} } }
            }));
        await bindings.parseData();
        expect(mockFetchImportStage.mock.calls.slice(-2).map(call => call[0])).toEqual([
            '/api/bills/import/v2/parse',
            '/api/bills/import/v2/dedup'
        ]);
        expect(bindings.serverSessionId.value).toBe('session-direct');
        expect(bindings.currentStep.value).toBe('checkData');
        const parseOptions = mockFetchImportStage.mock.calls.at(-2)?.[1];
        expect(parseOptions.headers).toEqual({ Authorization: 'Bearer token-1' });
        expect(parseOptions.body.get('parser_type')).toBe('auto');

        const unmatchedBindings = createBindings();
        setTemplateRef('snackbar', snackbar);
        setTemplateRef('importTransactionDefineColumnTab', defineTab);
        unmatchedBindings.importFiles.value = [new File(['amount\n-12.34'], 'generic.csv')];
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: {
                success: true,
                data: {
                    session_id: 'session-generic',
                    parsed_count: 0,
                    unmatched_files: [{ original_name: 'generic.csv', temp_path: 'generic.tmp' }]
                }
            }
        }));
        mockServices.matchImportConfig.mockResolvedValueOnce({ data: { success: false } });
        await unmatchedBindings.parseData();
        expect(unmatchedBindings.unmatchedFilesQueue.value).toEqual([
            { originalName: 'generic.csv', tempPath: 'generic.tmp', reason: undefined }
        ]);
        expect(mockServices.previewImportFileFromTemp).toHaveBeenLastCalledWith({
            sessionId: 'session-generic',
            tempPath: 'generic.tmp',
            fileEncoding: 'auto',
            delimiter: undefined
        });
        expect(unmatchedBindings.currentStep.value).toBe('defineColumn');

        unmatchedBindings.currentStep.value = 'defineColumn';
        defineTab.generateResult.mockReturnValueOnce(null);
        await unmatchedBindings.parseData();
        expect(unmatchedBindings.submitting.value).toBe(false);
        defineTab.generateResult.mockImplementationOnce(() => { throw new Error('mapping crashed'); });
        await unmatchedBindings.parseData();
        expect(snackbar.showError).toHaveBeenCalledWith('Unable to parse import file');
    });

    test('fails the whole batch clearly when an unmatched file is an unsupported legacy spreadsheet', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        setTemplateRef('snackbar', snackbar);
        bindings.importFiles.value = [new File(['legacy'], 'legacy.xls')];
        (globalThis.fetch as any).mockResolvedValue(createResponse());
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: {
                success: true,
                data: {
                    session_id: 'session-unsupported-xls',
                    parsed_count: 3,
                    unmatched_files: [{
                        original_name: 'legacy.xls',
                        temp_path: 'legacy.tmp',
                        reason: 'Presentation text is not the API discriminator',
                        error_code: 'unsupported_legacy_xls'
                    }]
                }
            }
        }));

        await bindings.parseData();

        expect(snackbar.showError).toHaveBeenCalledWith(
            '不支持旧版二进制 XLS：legacy.xls。请先转换为 XLSX 或 CSV 后重新导入。'
        );
        expect(bindings.currentStep.value).toBe('uploadFile');
        expect(bindings.unmatchedFilesQueue.value).toEqual([]);
        expect(bindings.serverSessionId.value).toBe('');
        expect(globalThis.fetch).toHaveBeenCalledWith(
            '/api/bills/import/v2/session/session-unsupported-xls',
            expect.objectContaining({ method: 'DELETE' })
        );
        expect(mockServices.previewImportFileFromTemp).not.toHaveBeenCalled();
    });

    test('reports parser and dedup HTTP/business failures and cleans created sessions', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        setTemplateRef('snackbar', snackbar);
        bindings.importFiles.value = [new File(['a'], 'failure.csv')];
        (globalThis.fetch as any).mockResolvedValue(createResponse());

        mockFetchImportStage.mockResolvedValueOnce(createResponse({ ok: false, text: 'parse rejected' }));
        await bindings.parseData();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.stringContaining('阶段1失败: parse rejected'));

        mockFetchImportStage.mockResolvedValueOnce(createResponse({ result: { success: false, error: 'no session' } }));
        await bindings.parseData();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.stringContaining('no session'));

        mockFetchImportStage
            .mockResolvedValueOnce(createResponse({
                result: { success: true, data: { session_id: 'session-dedup-fail', unmatched_files: [] } }
            }))
            .mockResolvedValueOnce(createResponse({ ok: false, text: 'dedup rejected' }));
        await bindings.parseData();
        await flushAsync();
        expect(globalThis.fetch).toHaveBeenCalledWith(
            '/api/bills/import/v2/session/session-dedup-fail',
            expect.objectContaining({ method: 'DELETE' })
        );

        bindings.serverSessionId.value = 'session-business-fail';
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: false, error: 'dedup business failed', data: {} }
        }));
        await expect(bindings.executeStage2Dedup()).rejects.toThrow('dedup business failed');
    });

    test('loads server-paged preview data with sorting, filters, stale-response guards, and request errors', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        setTemplateRef('snackbar', snackbar);

        await bindings.fetchPreviewPage();
        expect(mockFetchImportStage).not.toHaveBeenCalled();
        bindings.serverSessionId.value = 'session-preview';
        const previewRecord = { id: 31, selected: true, amountCents: -1234 };
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: {
                success: true,
                data: { preview: [previewRecord], total: '4', metadata: { selected_count: 2 } }
            }
        }));
        await bindings.fetchPreviewPage(0, 0, {
            sortBy: ' amount ',
            sortDirection: 'desc',
            filters: { signal: 'learning' }
        });
        expect(mockFetchImportStage).toHaveBeenLastCalledWith(
            expect.stringContaining('/api/bills/import/v2/preview/session-preview?page=1&page_size=10&sort_by=amount&sort_direction=desc&signal=learning'),
            expect.objectContaining({
                method: 'GET',
                headers: {
                    'Content-Type': 'application/json',
                    Authorization: 'Bearer token-1'
                },
                signal: expect.any(Object)
            }),
            '预览分页加载'
        );
        expect(bindings.importTransactions.value).toEqual([expect.objectContaining({ _previewId: 31 })]);
        expect(bindings.previewTotalCount.value).toBe(4);
        expect(bindings.previewMetadata.value).toEqual({ selected_count: 2 });
        expect(mockLogMilestone).toHaveBeenCalledWith('first_operable_preview_at', expect.objectContaining({
            row_count: 1,
            total: 4
        }));

        mockCoordinator.begin.mockReturnValueOnce(null);
        await bindings.fetchPreviewPage(2, 10);

        mockFetchImportStage.mockResolvedValueOnce(createResponse({ result: { success: true, data: { preview: [] } } }));
        mockCoordinator.isCurrent.mockReturnValueOnce(false);
        await bindings.fetchPreviewPage(2, 10);

        mockFetchImportStage.mockResolvedValueOnce(createResponse({ result: { success: true, data: { preview: [] } } }));
        mockCoordinator.isCurrent.mockReturnValueOnce(true).mockReturnValueOnce(false);
        await bindings.fetchPreviewPage(2, 10);

        mockFetchImportStage.mockResolvedValueOnce(createResponse({ ok: false, text: 'preview rejected' }));
        await expect(bindings.fetchPreviewPage(2, 10)).rejects.toThrow('preview rejected');
        mockFetchImportStage.mockResolvedValueOnce(createResponse({ result: { success: false, error: 'preview business failed' } }));
        await expect(bindings.fetchPreviewPage(2, 10)).rejects.toThrow('preview business failed');
        mockFetchImportStage.mockRejectedValueOnce({ name: 'AbortError' });
        await expect(bindings.fetchPreviewPage(2, 10)).resolves.toBeUndefined();
        mockFetchImportStage.mockRejectedValueOnce(new Error('preview unavailable'));
        await expect(bindings.fetchPreviewPage(2, 10)).rejects.toThrow('preview unavailable');
        expect(mockCoordinator.finish).toHaveBeenCalled();

        bindings.pendingInitialCheckDataPageRequest.value = {
            page: 1,
            pageSize: 10,
            sortBy: '',
            sortDirection: 'asc'
        };
        bindings.previewPageSortBy.value = '';
        bindings.previewPageSortDirection.value = 'asc';
        mockFetchImportStage.mockResolvedValueOnce(createResponse({ result: { success: true, data: { preview: [] } } }));
        await bindings.onCheckDataPageRequested();
        expect(bindings.pendingInitialCheckDataPageRequest.value).toBeNull();
        mockFetchImportStage.mockRejectedValueOnce({ name: 'AbortError' });
        await bindings.onCheckDataPageRequested(3, 20, { sortBy: 'date', sortDirection: 'desc' });
        mockFetchImportStage.mockRejectedValueOnce(new Error('page unavailable'));
        await bindings.onCheckDataPageRequested(3, 20, { filters: { signal: 'llm' } });
        expect(snackbar.showError).toHaveBeenCalledWith(expect.stringContaining('page unavailable'));
        bindings.abortPendingPreviewPageRequest();
        expect(mockCoordinator.abort).toHaveBeenCalled();
    });

    test('reclassifies visible previews by replacement and refreshes server-paged previews', async () => {
        const bindings = createBindings();
        const checkTab = {
            getCurrentPreviewPage: jest.fn(() => 2),
            getCurrentPreviewPageSize: jest.fn(() => 25),
            getCurrentServerPagedRequestOptions: jest.fn(() => ({ sortBy: 'date', sortDirection: 'desc' }))
        };
        setTemplateRef('importTransactionCheckDataTab', checkTab);

        bindings.onReclassified([]);
        bindings.importTransactions.value = [
            mockBuildTransaction({ id: 1 }, 0),
            mockBuildTransaction({ id: 2 }, 1)
        ];
        bindings.onReclassified([{ id: 3 }], [1]);
        expect(bindings.importTransactions.value.map((item: any) => item._previewId)).toEqual([2, 3]);
        bindings.onReclassified([{ id: 4 }]);
        expect(bindings.importTransactions.value.map((item: any) => item._previewId)).toEqual([4]);

        bindings.serverSessionId.value = 'session-reclassify';
        bindings.serverPagedPreviewMode.value = true;
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: true, data: { preview: [{ id: 5 }], total: 1 } }
        }));
        bindings.onReclassified([{ id: 99 }]);
        await flushAsync();
        expect(checkTab.getCurrentPreviewPage).toHaveBeenCalled();
        expect(bindings.importTransactions.value).toEqual([expect.objectContaining({ _previewId: 5 })]);
    });

    test('builds history rewrite acknowledgements for visible and server-paged selections', async () => {
        const bindings = createBindings();
        const historyTransaction = mockBuildTransaction({
            id: 51,
            selected: true,
            matching: {
                reconciliation: {
                    planned_operation: 'update_history',
                    history_bill_id: 7,
                    history_bill_version: 3,
                    history_summary: 'rewrite',
                    operation_id: 'operation-51',
                    acknowledgement_token: 'ack-51',
                    destructive_ack_required: true
                }
            }
        }, 0);
        const plainTransaction = mockBuildTransaction({ id: 52, selected: true }, 1);
        const unselectedTransaction = mockBuildTransaction({ id: 53, selected: false }, 2);

        expect(bindings.getPreviewIdFromTransaction({ _previewId: Number.NaN })).toBeNull();
        expect(bindings.getHistoryRewriteOperationFromTransaction(historyTransaction)).toMatchObject({
            preview_id: 51,
            history_bill_id: 7
        });
        const visibleAck = await bindings.buildHistoryRewriteConfirmAcknowledgement({
            selectedTransactions: [historyTransaction, plainTransaction, unselectedTransaction],
            selectedPreviewUpdates: [],
            selectedCount: 2
        });
        expect(visibleAck).toMatchObject({
            preview_ids: [51, 52],
            selection_scope: { mode: 'visible-preview', preserve_unpatched_selection: false }
        });
        expect(bindings.buildHistoryRewriteConfirmDetails(visibleAck)).toEqual(['update_history #7 v3']);
        expect(bindings.buildHistoryRewriteConfirmDetails(null)).toEqual([]);

        expect(await bindings.fetchSelectedPreviewTransactionsForConfirm(1)).toEqual([]);
        bindings.serverSessionId.value = 'session-history';
        mockFetchImportStage.mockResolvedValueOnce(createResponse({ ok: false, text: 'selected rejected' }));
        await expect(bindings.fetchSelectedPreviewTransactionsForConfirm(0)).rejects.toThrow('selected rejected');
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: false, error: 'selected business failed' }
        }));
        await expect(bindings.fetchSelectedPreviewTransactionsForConfirm(1)).rejects.toThrow('selected business failed');
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: true, data: { preview: [historyTransaction] } }
        }));
        expect(await bindings.fetchSelectedPreviewTransactionsForConfirm(1)).toHaveLength(1);

        bindings.serverPagedPreviewMode.value = true;
        bindings.importTransactions.value = [historyTransaction];
        setTemplateRef('importTransactionCheckDataTab', {
            getSelectedHistoryRewriteOperations: () => [
                { preview_id: 52, planned_operation: 'delete_history', history_bill_id: 8, history_bill_version: 2 }
            ],
            getSelectedVisibleHistoryRewriteOperationCount: () => 1
        });
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: true, data: { preview: [historyTransaction, plainTransaction] } }
        }));
        const serverAck = await bindings.buildHistoryRewriteConfirmAcknowledgement({
            selectedTransactions: [],
            selectedPreviewUpdates: [
                { id: 52, selected: false },
                { id: 54, selected: true },
                { id: 'bad', selected: true },
                { id: 55, selected: 'yes' }
            ],
            selectedCount: 2
        });
        expect(serverAck).toMatchObject({
            preview_ids: expect.arrayContaining([51, 54]),
            selection_scope: {
                mode: 'server-paged-selected-preview',
                preserve_unpatched_selection: true,
                selected_visible_history_rewrite_count: 1
            }
        });
        expect(serverAck.preview_ids).not.toContain(52);
    });

    test('submits legacy selected transactions with cents intact and covers confirmation failures', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        const confirmDialog = { open: jest.fn<(...args: any[]) => Promise<boolean>>() };
        const checkTab: any = { isEditing: true };
        setTemplateRef('snackbar', snackbar);
        setTemplateRef('confirmDialog', confirmDialog);
        setTemplateRef('importTransactionCheckDataTab', checkTab);

        await bindings.submit();
        expect(confirmDialog.open).not.toHaveBeenCalled();
        checkTab.isEditing = false;
        await bindings.submit();
        expect(snackbar.showError).toHaveBeenCalledWith('No data to import');

        bindings.importTransactions.value = [mockBuildTransaction({ id: 1, valid: false, selected: true }, 0)];
        await bindings.submit();
        expect(snackbar.showError).toHaveBeenCalledWith('Cannot import invalid transactions');

        const selected = mockBuildTransaction({ id: 2, valid: true, selected: true, amountCents: -1234 }, 0);
        const ignored = mockBuildTransaction({ id: 3, valid: true, selected: false }, 1);
        bindings.importTransactions.value = [selected, ignored];
        confirmDialog.open.mockResolvedValueOnce(false);
        await bindings.submit();
        await flushAsync();
        expect(globalThis.fetch).not.toHaveBeenCalled();

        confirmDialog.open.mockResolvedValueOnce(true);
        (globalThis.fetch as any).mockResolvedValueOnce(createResponse({
            result: { success: true, result: { items: [{ id: 1 }] } }
        }));
        await bindings.submit();
        await flushAsync();
        expect(globalThis.fetch).toHaveBeenCalledWith('/api/bills/batch', expect.objectContaining({
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                Authorization: 'Bearer token-1'
            }
        }));
        const legacyPayload = JSON.parse((globalThis.fetch as any).mock.calls.at(-1)?.[1].body);
        expect(legacyPayload.transactions[0]).toMatchObject({ amountCents: -1234 });
        expect(legacyPayload.transactions[0].clientSessionId).toBe(legacyPayload.clientSessionId);
        expect(bindings.currentStep.value).toBe('finalResult');
        expect(bindings.importedCount.value).toBe(1);
        expect(mockAccountStore.updateAccountListInvalidState).toHaveBeenCalledWith(true);

        bindings.currentStep.value = 'checkData';
        confirmDialog.open.mockResolvedValueOnce(true);
        (globalThis.fetch as any).mockResolvedValueOnce(createResponse({ ok: false, text: 'legacy rejected' }));
        await bindings.submit();
        await flushAsync();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.stringContaining('legacy rejected'));
        confirmDialog.open.mockResolvedValueOnce(true);
        (globalThis.fetch as any).mockResolvedValueOnce(createResponse({
            result: { success: false, error: 'legacy business failed' }
        }));
        await bindings.submit();
        await flushAsync();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.stringContaining('legacy business failed'));
        expect(bindings.submitting.value).toBe(false);
    });

    test('confirms server-paged preview updates, history acknowledgement, cleanup, and failure reporting', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        const confirmDialog = { open: jest.fn<(...args: any[]) => Promise<boolean>>(async () => true) };
        const checkTab = {
            isEditing: false,
            getSelectedPreviewUpdates: jest.fn(() => [{ id: 61, selected: true }]),
            getSelectedPreviewCount: jest.fn(() => 1),
            getSelectedHistoryRewriteOperations: jest.fn(() => [{
                preview_id: 61,
                planned_operation: 'update_history',
                history_bill_id: 9,
                history_bill_version: 4,
                operation_id: 'operation-61',
                acknowledgement_token: 'ack-61'
            }]),
            getSelectedVisibleHistoryRewriteOperationCount: jest.fn(() => 1)
        };
        setTemplateRef('snackbar', snackbar);
        setTemplateRef('confirmDialog', confirmDialog);
        setTemplateRef('importTransactionCheckDataTab', checkTab);
        bindings.serverSessionId.value = 'session-confirm';
        bindings.serverPagedPreviewMode.value = true;
        (globalThis.fetch as any).mockResolvedValue(createResponse());

        mockFetchImportStage
            .mockResolvedValueOnce(createResponse({
                result: {
                    success: true,
                    data: {
                        preview: [{
                            id: 61,
                            selected: true,
                            matching: {
                                reconciliation: {
                                    planned_operation: 'update_history',
                                    history_bill_id: 9,
                                    history_bill_version: 4,
                                    operation_id: 'operation-61',
                                    acknowledgement_token: 'ack-61',
                                    destructive_ack_required: true
                                }
                            }
                        }]
                    }
                }
            }))
            .mockResolvedValueOnce(createResponse({
                result: { success: true, data: { imported_count: 1 } }
            }));
        await bindings.submit();
        await flushAsync();
        expect(confirmDialog.open).toHaveBeenCalledWith(
            'format.misc.confirmImportTransactions',
            expect.objectContaining({ warning: 'History Rewrite', color: 'warning' })
        );
        const confirmCall = mockFetchImportStage.mock.calls.find(call => call[0] === '/api/bills/import/v2/confirm');
        expect(confirmCall).toBeDefined();
        const confirmPayload = JSON.parse(confirmCall?.[1].body);
        expect(confirmPayload).toMatchObject({
            session_id: 'session-confirm',
            preserve_unpatched_selection: true,
            preview_updates: [{ id: 61, selected: true }],
            history_rewrite_acknowledgement: {
                operations: [expect.objectContaining({ preview_id: 61, history_bill_id: 9 })]
            }
        });
        expect(bindings.currentStep.value).toBe('finalResult');
        expect(bindings.serverSessionId.value).toBe('');
        expect(globalThis.fetch).toHaveBeenCalledWith(
            '/api/bills/import/v2/session/session-confirm',
            expect.objectContaining({ method: 'DELETE' })
        );

        const failedBindings = createBindings();
        setTemplateRef('snackbar', snackbar);
        setTemplateRef('confirmDialog', confirmDialog);
        setTemplateRef('importTransactionCheckDataTab', checkTab);
        failedBindings.serverSessionId.value = 'session-history-fail';
        failedBindings.serverPagedPreviewMode.value = true;
        mockFetchImportStage.mockRejectedValueOnce(new Error('history unavailable'));
        await failedBindings.submit();
        expect(snackbar.showError).toHaveBeenCalledWith('history unavailable');

        mockFetchImportStage
            .mockResolvedValueOnce(createResponse({ result: { success: true, data: { preview: [] } } }))
            .mockResolvedValueOnce(createResponse({ ok: false, text: 'confirm rejected' }));
        await failedBindings.submit();
        await flushAsync();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.stringContaining('confirm rejected'));
        mockFetchImportStage
            .mockResolvedValueOnce(createResponse({ result: { success: true, data: { preview: [] } } }))
            .mockResolvedValueOnce(createResponse({ result: { success: false, error: 'confirm business failed' } }));
        await failedBindings.submit();
        await flushAsync();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.stringContaining('confirm business failed'));
    });

    test('confirms a visible v2 preview with category validation and cents-based preview updates', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        const confirmDialog = { open: jest.fn<(...args: any[]) => Promise<boolean>>(async () => true) };
        setTemplateRef('snackbar', snackbar);
        setTemplateRef('confirmDialog', confirmDialog);
        setTemplateRef('importTransactionCheckDataTab', {
            isEditing: false,
            getSelectedVisibleHistoryRewriteOperationCount: () => 0
        });
        const matchingCategory = mockBuildTransaction({ id: 71, selected: true, type: 3 }, 0);
        matchingCategory.categoryId = 'food';
        (matchingCategory as any)._shouldClearTransferDecision = true;
        const mismatchedCategory = mockBuildTransaction({ id: 72, selected: true, type: 3 }, 1);
        mismatchedCategory.categoryId = 'transfer';
        bindings.importTransactions.value = [matchingCategory, mismatchedCategory];
        bindings.serverSessionId.value = 'session-visible-confirm';
        bindings.serverPagedPreviewMode.value = false;
        mockGetCurrentToken.mockReturnValue('');
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: true, data: {} }
        }));
        (globalThis.fetch as any).mockResolvedValue(createResponse());

        await bindings.submit();
        await flushAsync();
        const confirmCall = mockFetchImportStage.mock.calls.find(call => call[0] === '/api/bills/import/v2/confirm');
        const payload = JSON.parse(confirmCall?.[1].body);
        expect(payload.preserve_unpatched_selection).toBe(false);
        expect(payload.preview_updates).toEqual([
            expect.objectContaining({ id: 71, amount_cents: -1234, category_path: mockCategoryStore.allTransactionCategoriesMap.food }),
            expect.objectContaining({ id: 72, amount_cents: -1234, category_path: null })
        ]);
        expect(payload.preview_updates[0].clear_transfer_decision).toBe(true);
        expect(confirmCall?.[1].headers).toEqual({ 'Content-Type': 'application/json' });
        expect(bindings.importedCount.value).toBe(2);
    });

    test('covers optional API fields and fallback values without changing the import contract', async () => {
        const bindings = createBindings();
        const snackbar = createSnackbar();
        const confirmDialog = { open: jest.fn<(...args: any[]) => Promise<boolean>>(async () => true) };
        const mapping = {
            columnMapping: { amount: 0 },
            includeHeader: false,
            timeFormat: '',
            timezoneFormat: '',
            amountDecimalSeparator: '.',
            amountDigitGroupingSymbol: ''
        };
        const defineTab = {
            applyFieldMappings: jest.fn(),
            generateResult: jest.fn<(...args: any[]) => any>(() => mapping),
            reset: jest.fn()
        };
        setTemplateRef('snackbar', snackbar);
        setTemplateRef('confirmDialog', confirmDialog);
        setTemplateRef('importTransactionDefineColumnTab', defineTab);
        expect(bindings.importFile.value).toBeUndefined();

        mockServices.getImportConfigs.mockResolvedValueOnce({ data: { result: {} } });
        await bindings.loadImportConfigList();
        expect(bindings.importConfigList.value).toEqual([]);
        bindings.parsedFileDelimiter.value = '|';
        bindings.applyImportConfig(createConfig({ delimiter: '' }));
        expect(bindings.parsedFileDelimiter.value).toBe('|');

        const minimalConfig = createConfig({
            id: 88,
            name: '',
            fileFormat: '',
            encoding: '',
            delimiter: undefined,
            hasHeader: undefined,
            customRules: undefined,
            sampleHeaders: undefined,
            description: '',
            descriptionSummary: '',
            defaultRecommendation: false
        });
        bindings.openEditImportConfigDialog(minimalConfig);
        expect(bindings.editImportConfigName.value).toBe('');
        bindings.editImportConfigName.value = 'Minimal';
        bindings.editingImportConfig.value = minimalConfig;
        bindings.matchedImportConfig.value = createConfig({ id: 99 });
        bindings.importFiles.value = [new File(['a'], 'fallback.csv')];
        mockServices.saveImportConfig.mockResolvedValueOnce({ data: { success: true } });
        mockServices.getImportConfigs.mockResolvedValueOnce({ data: { result: [] } });
        await bindings.saveEditedImportConfig();
        expect(mockServices.saveImportConfig).toHaveBeenLastCalledWith(expect.objectContaining({
            fileFormat: 'csv',
            encoding: 'utf-8',
            hasHeader: true,
            customRules: {},
            sampleHeaders: []
        }));
        expect(bindings.matchedImportConfig.value.id).toBe(99);

        bindings.removeImportConfig(minimalConfig);
        await flushAsync();
        expect(bindings.matchedImportConfig.value.id).toBe(99);

        bindings.parsedFileData.value = [['amount']];
        bindings.importFiles.value = [];
        bindings.matchedImportConfig.value = null;
        mockServices.getImportConfigs.mockResolvedValueOnce({ data: { result: [createConfig({ isDefault: true })] } });
        bindings.openSaveImportConfigDialog();
        await flushAsync();
        expect(bindings.saveImportConfigName.value).toBe('import 模板');
        expect(bindings.saveImportConfigRecommended.value).toBe(false);

        bindings.parsedFileData.value = [undefined] as any;
        bindings.saveImportConfigName.value = 'Fallback';
        mockServices.saveImportConfig.mockResolvedValueOnce({ data: { success: true, result: {} } });
        mockServices.getImportConfigs.mockResolvedValueOnce({ data: { result: [] } });
        await bindings.saveCurrentImportConfig();
        expect(bindings.matchedImportConfig.value).toMatchObject({ id: 0, sampleHeaders: [] });

        bindings.serverSessionId.value = 'session-preview-fallback';
        bindings.unmatchedFilesQueue.value = [{ originalName: 'missing.csv', tempPath: 'missing.tmp' }];
        mockServices.previewImportFileFromTemp.mockResolvedValueOnce({ data: { result: {} } });
        await bindings.prepareColumnMappingForUnmatchedFile(bindings.unmatchedFilesQueue.value[0]);
        mockServices.previewImportFileFromTemp.mockResolvedValueOnce({ data: { result: { sampleData: [undefined] } } });
        await bindings.prepareColumnMappingForUnmatchedFile(bindings.unmatchedFilesQueue.value[0]);
        mockServices.previewImportFileFromTemp.mockResolvedValueOnce({
            data: { result: { sampleData: [['amount'], ['-12.34']], delimiter: ',' } }
        });
        mockServices.matchImportConfig.mockResolvedValueOnce({
            data: { success: true, result: createConfig({ delimiter: undefined }) }
        });
        await bindings.prepareColumnMappingForUnmatchedFile(bindings.unmatchedFilesQueue.value[0]);

        mockGetCurrentToken.mockReturnValue('');
        bindings.serverSessionId.value = 'session-fallback';
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: true, data: { preview: {}, total: undefined, metadata: undefined } }
        }));
        await bindings.fetchPreviewPage(1, 10);
        expect(bindings.importTransactions.value).toEqual([]);
        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: true, data: { after_dedup: 0, preview_count: 0, preview: [], dedup_stats: undefined } }
        }));
        await bindings.executeStage2Dedup();

        mockFetchImportStage.mockResolvedValueOnce(createResponse({
            result: { success: true, data: { preview: {} } }
        }));
        expect(await bindings.fetchSelectedPreviewTransactionsForConfirm(1)).toEqual([]);
    });

    test('cleans sessions with and without tokens, tolerates cleanup errors, and rejects cancellation', async () => {
        const bindings = createBindings();
        await bindings.cleanupServerSession();
        expect(globalThis.fetch).not.toHaveBeenCalled();

        bindings.serverSessionId.value = 'session-clean';
        mockGetCurrentToken.mockReturnValueOnce('');
        (globalThis.fetch as any).mockResolvedValueOnce(createResponse());
        await bindings.cleanupServerSession();
        expect(globalThis.fetch).toHaveBeenLastCalledWith(
            '/api/bills/import/v2/session/session-clean',
            { method: 'DELETE', headers: {} }
        );
        expect(bindings.previewMetadata.value).toBeNull();

        bindings.serverSessionId.value = 'session-clean-error';
        (globalThis.fetch as any).mockRejectedValueOnce(new Error('cleanup unavailable'));
        await bindings.cleanupServerSession();
        expect(mockLogger.warn).toHaveBeenCalledWith('[三阶段导入] 清理会话失败:', expect.any(Error));

        const cancelBindings = createBindings();
        mockAccountStore.loadAllAccounts.mockResolvedValue();
        const pending = cancelBindings.open();
        await flushAsync();
        cancelBindings.serverSessionId.value = 'session-cancel';
        (globalThis.fetch as any).mockResolvedValue(createResponse());
        cancelBindings.close(false);
        await expect(pending).rejects.toBeUndefined();
        await flushAsync();
        expect(cancelBindings.showState.value).toBe(false);
    });
});
