import { describe, expect, jest, test } from '@jest/globals';

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => String(value) }),
        formatUnixTimeToLongDateTime: (value: unknown) => String(value || ''),
        formatAmountToLocalizedNumeralsWithCurrency: (value: unknown) => String(value || 0),
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
    useTransactionTagsStore: () => ({
        allTransactionTags: [],
        allTransactionTagsMap: {}
    })
}));
jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: {} }));
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
        default: { name: 'SignalAdapterSfcStub' }
    }));
}

import { ImportTransaction } from '@/models/imported_transaction.ts';
import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';
import { previewStateSnapshot } from '../../../../helpers/importPreviewState.ts';

function createSignalTransaction(learning: Record<string, unknown> = {}, llm?: Record<string, unknown>): ImportTransaction {
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
            learning,
            ...(llm ? { llm } : {})
        }
    } as never, 0);
    const signals: Array<'parser' | 'learning' | 'llm'> = [];
    const statuses: Partial<Record<'learning' | 'llm', 'pending' | 'accepted' | 'rejected' | 'skipped'>> = {};
    const addFamily = (family: 'learning' | 'llm', section: Record<string, unknown> | undefined): void => {
        if (!section || section['review_status'] === null || section['suppressed'] === true) {
            return;
        }
        const status = String(
            section['review_status']
            || section['status']
            || section['lifecycle_status']
            || section['signal_state']
            || '',
        ).trim().toLowerCase();
        const hasEvidence = Object.entries(section).some(([key, value]) => (
            !['review_status', 'status', 'lifecycle_status', 'signal_state', 'suppressed'].includes(key)
            && value !== ''
            && value !== null
            && value !== undefined
            && value !== 0
            && value !== false
        ));
        if (!hasEvidence && !['accepted', 'rejected', 'skipped'].includes(status)) {
            return;
        }
        signals.push(family);
        statuses[family] = ['accepted', 'rejected', 'skipped'].includes(status)
            ? status as 'accepted' | 'rejected' | 'skipped'
            : 'pending';
    };
    addFamily('learning', learning);
    addFamily('llm', llm);
    if (signals.length === 0) {
        signals.push('parser');
    }
    transaction.previewState = previewStateSnapshot(signals, statuses);
    return transaction;
}

function createBindings(transaction: ImportTransaction): any {
    const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
    try {
        return (ImportTransactionCheckDataTab as any).setup({
            importTransactions: [transaction],
            sessionId: 'signal-adapter-session',
            serverPaged: false
        }, {
            emit: jest.fn(),
            expose: jest.fn()
        });
    } finally {
        warnSpy.mockRestore();
    }
}

describe('desktop import signal adapter', () => {
    test('maps the browser E2E learning payload to a learning chip and excludes parser', () => {
        const transaction = createSignalTransaction({
            review_status: 'pending',
            lifecycle_status: 'pending',
            signal_state: 'pending',
            score: 0.91,
            rule_id: 42,
            confidence: 0.88,
            margin: 0.21,
            accepted_count: 3,
            rejected_count: 1,
            auto_applied_count: 2,
            suppressed: false,
            auto_apply: false,
            reason: '',
            summary: '',
            mode: ''
        });
        const bindings = createBindings(transaction);

        const view = bindings.getImportPreviewSignalViewModel(transaction);

        expect(view.learning).toMatchObject({ status: 'pending' });
        expect(view.learning.actions).toEqual(expect.arrayContaining([
            expect.objectContaining({ decision: 'accept' }),
            expect.objectContaining({ decision: 'reject' })
        ]));
        expect(view.parser).toBeNull();
    });

    test('keeps status-only pending learning hidden and preserves terminal states', () => {
        const pendingOnly = createSignalTransaction({
            review_status: 'pending',
            lifecycle_status: 'pending',
            signal_state: 'pending'
        });
        const accepted = createSignalTransaction({
            review_status: 'accepted'
        });
        const authoritativeNone = createSignalTransaction({
            review_status: null,
            score: 0.91
        });
        const pendingBindings = createBindings(pendingOnly);
        const acceptedBindings = createBindings(accepted);
        const authoritativeNoneBindings = createBindings(authoritativeNone);

        expect(pendingBindings.getImportPreviewSignalViewModel(pendingOnly)).toMatchObject({
            learning: null,
            parser: expect.any(Object)
        });
        expect(acceptedBindings.getImportPreviewSignalViewModel(accepted).learning).toMatchObject({
            status: 'accepted'
        });
        expect(authoritativeNoneBindings.getImportPreviewSignalViewModel(authoritativeNone)).toMatchObject({
            learning: null,
            parser: expect.any(Object)
        });
    });

    test('maps LLM alias and category-id evidence and invalidates caches for business evidence', () => {
        const transaction = createSignalTransaction({}, {
            review_status: '',
            status: '',
            lifecycle_status: 'pending',
            signal_state: 'pending',
            suggested_category_id: 7,
            suppressed: false
        });
        const bindings = createBindings(transaction);

        const firstView = bindings.getImportPreviewSignalViewModel(transaction);
        expect(firstView.llm).toMatchObject({ status: 'pending' });
        expect(firstView.parser).toBeNull();

        transaction.matching!.llm!.suggested_category_id = 0;
        transaction.previewState = previewStateSnapshot(['parser']);
        const secondView = bindings.getImportPreviewSignalViewModel(transaction);
        expect(secondView.llm).toBeNull();
        expect(secondView.parser).toEqual(expect.any(Object));
        expect(secondView).not.toBe(firstView);

        const terminal = createSignalTransaction({}, {
            lifecycle_status: 'accepted'
        });
        const terminalBindings = createBindings(terminal);
        expect(terminalBindings.getImportPreviewSignalViewModel(terminal).llm).toMatchObject({
            status: 'accepted'
        });

    });

    test('invalidates cached learning and LLM views when status authority changes', () => {
        const learningTransaction = createSignalTransaction({
            review_status: undefined,
            score: 0.91
        });
        const learningBindings = createBindings(learningTransaction);
        expect(learningBindings.getImportPreviewSignalViewModel(learningTransaction).learning).toMatchObject({
            status: 'pending'
        });

        (learningTransaction.matching!.learning as any).review_status = null;
        learningTransaction.previewState = previewStateSnapshot(['parser']);
        expect(learningBindings.getImportPreviewSignalViewModel(learningTransaction)).toMatchObject({
            learning: null,
            parser: expect.any(Object)
        });

        const llmTransaction = createSignalTransaction({}, {
            review_status: undefined,
            suggested_category_id: 7
        });
        const llmBindings = createBindings(llmTransaction);
        expect(llmBindings.getImportPreviewSignalViewModel(llmTransaction).llm).toMatchObject({
            status: 'pending'
        });

        (llmTransaction.matching!.llm as any).review_status = null;
        llmTransaction.previewState = previewStateSnapshot(['parser']);
        expect(llmBindings.getImportPreviewSignalViewModel(llmTransaction)).toMatchObject({
            llm: null,
            parser: expect.any(Object)
        });
    });

    test('uses the complete six-signal metadata as the v2 row-authority protocol marker', () => {
        const transaction = createSignalTransaction();
        (transaction as ImportTransaction & { _previewId: number })._previewId = 97;
        const bindings = createBindings(transaction);
        const currentV2Metadata = {
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

        expect(bindings.hasCurrentV2PreviewSignalProtocol(null)).toBe(false);
        expect(bindings.hasCurrentV2PreviewSignalProtocol({
            counts: { signals: { parser: 1 } }
        })).toBe(false);
        expect(bindings.hasCurrentV2PreviewSignalProtocol(currentV2Metadata)).toBe(true);

        bindings.llmSessionSignalMemory.value = new Map([[97, {
            reviewStatus: 'accepted',
            suppressed: false,
            suggestedCategoryId: 7,
            suggestedMainCategory: '餐饮',
            suggestedSubCategory: '咖啡',
            suggestedSourceAccount: '',
            suggestedDestinationAccount: '',
            confidence: 0.91,
            reason: 'legacy-memory'
        }]]);
        bindings.applyLLMSignalMemoryToTransactions([transaction]);
        expect(bindings.getImportPreviewSignalViewModel(transaction)).toMatchObject({
            llm: null,
            parser: expect.any(Object)
        });

        bindings.recordCurrentV2ServerLLMAuthority([transaction, createSignalTransaction()], currentV2Metadata);
        expect(bindings.getImportPreviewSignalViewModel(transaction)).toMatchObject({
            llm: null,
            parser: expect.any(Object)
        });
    });
});
