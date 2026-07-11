import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const { nextTick, reactive } = jest.requireActual<{
    nextTick: () => Promise<void>;
    reactive: <T extends object>(value: T) => T;
}>('vue');

const mockGetLLMMemoryEvents = jest.fn<(...args: Array<unknown>) => Promise<any>>();

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => String(value) }),
        formatUnixTimeToLongDateTime: (value: unknown) => String(value ?? ''),
        formatAmountToLocalizedNumeralsWithCurrency: (value: unknown) => String(value ?? 0),
        getCategorizedAccountsWithDisplayBalance: () => []
    })
}));
jest.mock('@/stores/setting.ts', () => ({
    useSettingsStore: () => ({ appSettings: { showAccountBalance: false, timeZone: 'Asia/Shanghai' } })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({
        currentUserFirstDayOfWeek: 1,
        currentUserFiscalYearStart: 1,
        currentUserDefaultCurrency: 'CNY',
        currentUserCoordinateDisplayType: 0
    })
}));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => ({
        allPlainAccounts: [],
        allVisiblePlainAccounts: [],
        allAccountsMap: {},
        allAccounts: []
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: {},
        allTransactionCategoriesMap: {}
    })
}));
jest.mock('@/stores/transactionTag.ts', () => ({
    useTransactionTagsStore: () => ({ allTransactionTags: [], allTransactionTagsMap: {} })
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: { getLLMMemoryEvents: mockGetLLMMemoryEvents }
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isTransactionFromAIImageRecognitionEnabled: () => false
}));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => '' }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));
jest.mock('@/views/desktop/transactions/import/check-data-tab/useImportCheckDataMenus.ts', () => ({
    useImportCheckDataMenus: () => ({ filterMenus: [], toolMenus: [] })
}));
jest.mock('@/views/desktop/transactions/import/check-data-tab/useImportCheckDataBatchActions.ts', () => ({
    useImportCheckDataBatchActions: () => ({
        clearSelectedRecurringMatches: jest.fn(),
        convertTransactionType: jest.fn(),
        showBatchAddDialog: jest.fn(),
        showBatchCreateInvalidItemDialog: jest.fn(),
        showBatchReplaceDialog: jest.fn(),
        showReplaceAllTypesDialog: jest.fn(),
        showReplaceInvalidItemDialog: jest.fn()
    })
}));

for (const componentPath of [
    '@/components/desktop/PaginationButtons.vue',
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/transactions/import/tabs/ImportPreviewSignalCell.vue',
    '@/views/desktop/transactions/import/dialogs/BatchReplaceDialog.vue',
    '@/views/desktop/transactions/import/dialogs/BatchReplaceAllTypesDialog.vue',
    '@/views/desktop/transactions/import/dialogs/BatchCreateDialog.vue',
    '@/views/desktop/transactions/import/dialogs/ImportLearningSuggestionDialog.vue',
    '@/views/desktop/categories/list/dialogs/EditDialog.vue',
    '@/views/desktop/accounts/list/dialogs/EditDialog.vue'
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: { name: 'LlmMemoryAuthorityStub' }
    }));
}

import { ImportTransaction } from '@/models/imported_transaction.ts';
import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';

function createTransaction(id: number, llm?: Record<string, unknown>): ImportTransaction {
    const transaction = ImportTransaction.of({
        type: 3,
        categoryId: '1',
        originalCategoryName: '餐饮',
        time: 1_788_480_000,
        utcOffset: 480,
        sourceAccountId: 'wallet',
        originalSourceAccountName: '钱包',
        originalSourceAccountCurrency: 'CNY',
        tagIds: [],
        originalTagNames: [],
        sourceAmountCents: 1_234,
        comment: '午餐',
        counterparty: '测试商户',
        paymentMethod: '微信支付',
        matching: {
            parser: { id: 'wechat', tags: ['parser:wechat'] },
            ...(llm ? { llm } : {})
        }
    } as never, id);
    (transaction as ImportTransaction & { _previewId: number })._previewId = id;
    return transaction;
}

function createBindings(
    transaction: ImportTransaction,
    serverPaged: boolean = false,
    previewMetadata: Record<string, unknown> | null = null
): any {
    const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
    try {
        return (ImportTransactionCheckDataTab as any).setup({
            importTransactions: [transaction],
            sessionId: 'llm-memory-authority-session',
            serverPaged,
            totalImportTransactionCount: 1,
            previewMetadata
        }, {
            emit: jest.fn(),
            expose: jest.fn()
        });
    } finally {
        warnSpy.mockRestore();
    }
}

function createReactiveBindings(transaction: ImportTransaction): {
    bindings: any;
    props: {
        importTransactions: ImportTransaction[];
        sessionId: string;
        serverPaged: boolean;
        totalImportTransactionCount: number;
        previewMetadata: Record<string, unknown> | null;
    };
} {
    const props = reactive({
        importTransactions: [transaction],
        sessionId: 'llm-memory-authority-session',
        serverPaged: false,
        totalImportTransactionCount: 1,
        previewMetadata: null as Record<string, unknown> | null
    });
    const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
    let bindings: any;
    try {
        bindings = (ImportTransactionCheckDataTab as any).setup(props, {
            emit: jest.fn(),
            expose: jest.fn()
        });
    } finally {
        warnSpy.mockRestore();
    }
    return { bindings, props };
}

async function flushMemoryRequest(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
}

function memoryEvent(id: number, previewId: number, decision: string): Record<string, unknown> {
    return {
        id,
        preview_id: previewId,
        event_type: 'preview_review',
        decision,
        llm_response_raw: JSON.stringify({
            suggested_category_id: 7,
            suggested_main_category: '餐饮',
            suggested_sub_category: '咖啡',
            confidence: 0.91,
            reason: `memory-${decision}`
        })
    };
}

beforeEach(() => {
    jest.clearAllMocks();
});

describe('desktop LLM memory authority', () => {
    test('a first-load v2 preview row with no llm rejects older terminal memory', async () => {
        mockGetLLMMemoryEvents.mockResolvedValue({
            data: { result: { events: [
                memoryEvent(15, 40, 'accept'),
                memoryEvent(14, 40, 'reject')
            ] } }
        });
        const transaction = createTransaction(40);
        const bindings = createBindings(transaction, false, {
            counts: {
                signals: {
                    parser: 1,
                    platform_duplicate: 0,
                    transfer: 0,
                    history: 0,
                    learning: 0,
                    llm: 0
                }
            }
        });

        await bindings.refreshLLMSessionSignalMemory(true);
        bindings.applyLLMSignalMemoryToTransactions([transaction]);

        expect(bindings.getImportPreviewSignalViewModel(transaction)).toMatchObject({
            llm: null,
            parser: expect.any(Object)
        });
    });

    test('metadata arriving after memory reclassifies the first page as v2 authority', async () => {
        mockGetLLMMemoryEvents.mockResolvedValue({
            data: { result: { events: [memoryEvent(16, 47, 'accept')] } }
        });
        const transaction = createTransaction(47);
        const { bindings, props } = createReactiveBindings(transaction);
        await bindings.refreshLLMSessionSignalMemory(true);
        expect(bindings.getImportPreviewSignalViewModel(transaction).llm).toMatchObject({ status: 'accepted' });

        props.previewMetadata = {
            counts: {
                signals: {
                    parser: 1,
                    platform_duplicate: 0,
                    transfer: 0,
                    history: 0,
                    learning: 0,
                    llm: 0
                }
            }
        };
        await nextTick();

        expect(bindings.getImportPreviewSignalViewModel(transaction)).toMatchObject({
            llm: null,
            parser: expect.any(Object)
        });
    });

    test('a newest clear tombstone keeps a fresh server row without llm clear', async () => {
        mockGetLLMMemoryEvents.mockResolvedValue({
            data: { result: { events: [
                memoryEvent(13, 41, 'clear'),
                memoryEvent(12, 41, 'reject'),
                memoryEvent(11, 41, 'accept')
            ] } }
        });
        const transaction = createTransaction(41);
        const bindings = createBindings(transaction, false, {
            counts: { signals: { parser: 1 } }
        });

        await flushMemoryRequest();
        bindings.applyLLMSignalMemoryToTransactions([transaction]);

        expect(bindings.getImportPreviewSignalViewModel(transaction)).toMatchObject({
            llm: null,
            parser: expect.any(Object)
        });
    });

    test('explicit server llm status is authoritative over memory feedback', async () => {
        mockGetLLMMemoryEvents.mockResolvedValue({
            data: { result: { events: [memoryEvent(22, 42, 'reject')] } }
        });
        const transaction = createTransaction(42, {
            review_status: 'accepted',
            lifecycle_status: 'accepted',
            signal_state: 'resolved',
            suggested_category_id: 8,
            reason: 'server-accepted'
        });
        const bindings = createBindings(transaction);

        await flushMemoryRequest();
        bindings.applyLLMSignalMemoryToTransactions([transaction]);

        expect(transaction.matching?.llm).toMatchObject({
            review_status: 'accepted',
            reason: 'server-accepted',
            suggested_category_id: 8
        });
        expect(bindings.getImportPreviewSignalViewModel(transaction).llm).toMatchObject({
            status: 'accepted'
        });
    });

    test('legacy llm evidence without an authoritative status can use memory feedback', async () => {
        mockGetLLMMemoryEvents.mockResolvedValue({
            data: { result: { events: [memoryEvent(31, 43, 'accept')] } }
        });
        const transaction = createTransaction(43, {
            suggested_category_id: 7,
            reason: 'legacy-evidence'
        });
        const bindings = createBindings(transaction, false, {
            counts: { signals: { parser: 1 } }
        });

        await flushMemoryRequest();
        bindings.applyLLMSignalMemoryToTransactions([transaction]);

        expect(transaction.matching?.llm?.review_status).toBe('accepted');
        expect(bindings.getImportPreviewSignalViewModel(transaction).llm).toMatchObject({
            status: 'accepted'
        });
    });

    test('a material-edit tombstone blocks cached memory from resurrecting llm', async () => {
        mockGetLLMMemoryEvents.mockResolvedValue({
            data: { result: { events: [memoryEvent(41, 44, 'accept')] } }
        });
        const transaction = createTransaction(44);
        const bindings = createBindings(transaction);
        await flushMemoryRequest();
        expect(bindings.getImportPreviewSignalViewModel(transaction).llm).toMatchObject({ status: 'accepted' });

        bindings.clearLLMRecommendationState(transaction);
        bindings.applyLLMSignalMemoryToTransactions([transaction]);

        expect(bindings.getImportPreviewSignalViewModel(transaction)).toMatchObject({
            llm: null,
            parser: expect.any(Object)
        });
    });

    test('rehydrating a fresh server row without llm cannot restore an older terminal memory event', async () => {
        mockGetLLMMemoryEvents.mockResolvedValue({
            data: { result: { events: [memoryEvent(51, 45, 'reject')] } }
        });
        const transaction = createTransaction(45);
        const bindings = createBindings(transaction, true);
        await flushMemoryRequest();
        bindings.cacheCurrentPageDrafts();

        transaction.matching = {
            ...transaction.matching,
            parser: { id: 'wechat', tags: ['parser:wechat'] },
            llm: undefined
        } as never;
        bindings.rehydrateCurrentPageDrafts();
        bindings.applyLLMSignalMemoryToTransactions([transaction]);

        expect(bindings.getImportPreviewSignalViewModel(transaction)).toMatchObject({
            llm: null,
            parser: expect.any(Object)
        });
    });

    test('a sparse authoritative matching payload does not crash transfer baseline capture', () => {
        mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
        const transaction = createTransaction(46);
        const bindings = createBindings(transaction);
        transaction.matching = {
            parser: { id: 'wechat', tags: ['parser:wechat'] }
        } as never;

        expect(bindings.buildTransferDecisionBaseline(transaction)).toMatchObject({
            reviewStatus: '',
            reviewedType: '',
            suppressed: false
        });
    });
});
