import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockGetLLMMemoryEvents = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockRejectMatchingCandidate = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockGetMatchingSessionCandidates = jest.fn<(...args: Array<unknown>) => Promise<any>>();

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => String(value) }),
        formatUnixTimeToLongDateTime: (value: unknown) => String(value ?? ''),
        formatAmountToLocalizedNumeralsWithCurrency: (value: unknown, currency: string) => `${currency}:${value}`,
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
        allAccounts: [],
        loadAllAccounts: jest.fn()
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: {},
        allTransactionCategoriesMap: {},
        loadAllCategories: jest.fn()
    })
}));
jest.mock('@/stores/transactionTag.ts', () => ({
    useTransactionTagsStore: () => ({ allTransactionTags: [], allTransactionTagsMap: {} })
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getLLMMemoryEvents: mockGetLLMMemoryEvents,
        rejectMatchingCandidate: mockRejectMatchingCandidate,
        getMatchingSessionCandidates: mockGetMatchingSessionCandidates,
        getImportPreviewRowVersionConflict: () => null
    }
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isTransactionFromAIImageRecognitionEnabled: () => false
}));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => 'draft-token' }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    showLoading: jest.fn(),
    hideLoading: jest.fn(),
    useI18nUIComponents: () => ({
        showAlert: jest.fn(),
        showConfirm: jest.fn(),
        showToast: jest.fn(),
        routeBackOnError: jest.fn()
    })
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
        default: { name: 'ServerDraftRehydrateStub' }
    }));
}

import { ImportTransaction } from '@/models/imported_transaction.ts';
import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';

function createTransaction(id: number): ImportTransaction {
    const transaction = ImportTransaction.of({
        type: 3,
        categoryId: '8',
        originalCategoryName: 'Original category',
        time: 1_788_480_000,
        utcOffset: 480,
        sourceAccountId: '11',
        originalSourceAccountName: 'Original source',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: '',
        originalDestinationAccountName: '',
        originalDestinationAccountCurrency: 'CNY',
        sourceAmountCents: -1_000,
        destinationAmountCents: 1_000,
        tagIds: ['31'],
        originalTagNames: ['Original tag'],
        counterparty: 'Original merchant',
        paymentMethod: 'Original payment',
        comment: 'Original comment',
        recurringTemplateId: '51',
        recurringTemplateName: 'Original recurring',
        recurringCandidateCount: 1,
        recurringMatchScore: 0.51,
        recurringMatchReasons: 'original recurring reason',
        recurringMatchedDate: '2026-07-01',
        selected: false,
        matching: {
            parser: { id: 'parser-old', tags: ['old'] },
            annotation: {}
        }
    } as never, id);
    (transaction as ImportTransaction & { _previewId: number })._previewId = id;
    (transaction as ImportTransaction & { _rowVersion: number })._rowVersion = 5;
    return transaction;
}

function createBindings(transaction: ImportTransaction): any {
    return (ImportTransactionCheckDataTab as any).setup({
        importTransactions: [transaction],
        sessionId: 'server-draft-session',
        serverPaged: true,
        totalImportTransactionCount: 2,
        previewMetadata: null
    }, {
        emit: jest.fn(),
        expose: jest.fn()
    });
}

function setFreshAuthoritativeServerResponse(transaction: ImportTransaction): Record<string, unknown> {
    const matching = {
        parser: { id: 'parser-new', tags: ['new'] },
        platform_duplicate: { status: 'pending', duplicate_count: 2 },
        transfer: { review_status: 'accepted', lifecycle_status: 'accepted', signal_state: 'resolved' },
        reconciliation: { operation_status: 'pending', group_key: 'history-group' },
        learning: {
            review_status: 'pending',
            lifecycle_status: 'pending',
            signal_state: 'pending',
            rule_id: 77,
            score: 0.91
        },
        llm: {
            review_status: 'rejected',
            lifecycle_status: 'rejected',
            signal_state: 'resolved',
            confidence: 0.88
        },
        annotation: { needs_review: true }
    };

    transaction.type = 1;
    transaction.categoryId = '99';
    transaction.sourceAmountCents = -9_900;
    transaction.destinationAmountCents = 9_900;
    transaction.sourceAccountId = '91';
    transaction.destinationAccountId = '92';
    transaction.tagIds = ['93'];
    transaction.originalTagNames = ['Server tag'];
    transaction.counterparty = 'Server merchant';
    transaction.paymentMethod = 'Server payment';
    transaction.comment = 'Server comment';
    transaction.recurringTemplateId = '95';
    transaction.recurringTemplateName = 'Server recurring';
    transaction.recurringCandidateCount = 9;
    transaction.recurringMatchScore = 0.95;
    transaction.recurringMatchReasons = 'server recurring reason';
    transaction.recurringMatchedDate = '2026-07-09';
    transaction.selected = true;
    transaction.originalCategoryName = 'Server category evidence';
    transaction.parserId = 'parser-new';
    transaction.parserTags = ['new'];
    transaction.matching = matching as never;
    return matching;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
    mockRejectMatchingCandidate.mockResolvedValue({
        data: {
            result: {
                sessionId: 'server-draft-session',
                previewItem: null
            }
        }
    });
    mockGetMatchingSessionCandidates.mockResolvedValue({
        data: { result: { candidates: [] } }
    });
});

describe('desktop server-paged draft rehydration', () => {
    test('an unedited cached row cannot replace a fresh authoritative signal response', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(41);
            const bindings = createBindings(transaction);
            bindings.cacheCurrentPageDrafts();

            const matching = setFreshAuthoritativeServerResponse(transaction);
            bindings.rehydrateCurrentPageDrafts();

            expect(transaction).toMatchObject({
                type: 1,
                categoryId: '99',
                sourceAmountCents: -9_900,
                destinationAmountCents: 9_900,
                sourceAccountId: '91',
                destinationAccountId: '92',
                tagIds: ['93'],
                originalTagNames: ['Server tag'],
                counterparty: 'Server merchant',
                paymentMethod: 'Server payment',
                comment: 'Server comment',
                recurringTemplateId: '95',
                recurringTemplateName: 'Server recurring',
                recurringCandidateCount: 9,
                recurringMatchScore: 0.95,
                recurringMatchReasons: 'server recurring reason',
                recurringMatchedDate: '2026-07-09',
                selected: true,
                originalCategoryName: 'Server category evidence',
                parserId: 'parser-new',
                parserTags: ['new']
            });
            expect(transaction.matching).toEqual(matching);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('only local editable deltas and selection delta survive a fresh signal response', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(42);
            const bindings = createBindings(transaction);

            transaction.type = 4;
            transaction.categoryId = '18';
            transaction.sourceAmountCents = -1_800;
            transaction.destinationAmountCents = 1_800;
            transaction.sourceAccountId = '21';
            transaction.destinationAccountId = '22';
            transaction.tagIds = ['41', '42'];
            transaction.originalTagNames = ['Draft tag 1', 'Draft tag 2'];
            transaction.counterparty = 'Draft merchant';
            transaction.paymentMethod = 'Draft payment';
            transaction.comment = 'Draft comment';
            transaction.recurringTemplateId = '61';
            transaction.recurringTemplateName = 'Draft recurring';
            transaction.recurringCandidateCount = 6;
            transaction.recurringMatchScore = 0.61;
            transaction.recurringMatchReasons = 'draft recurring reason';
            transaction.recurringMatchedDate = '2026-07-06';
            transaction.selected = true;
            transaction.isManuallyAnnotated = true;
            bindings.cacheCurrentPageDrafts();

            const matching = setFreshAuthoritativeServerResponse(transaction);
            transaction.selected = false;
            const serverEvidence = transaction.originalCategoryName;
            bindings.rehydrateCurrentPageDrafts();

            expect(transaction).toMatchObject({
                type: 4,
                categoryId: '18',
                sourceAmountCents: -1_800,
                destinationAmountCents: 1_800,
                sourceAccountId: '21',
                destinationAccountId: '22',
                tagIds: ['41', '42'],
                originalTagNames: ['Server tag'],
                counterparty: 'Draft merchant',
                paymentMethod: 'Draft payment',
                comment: 'Draft comment',
                recurringTemplateId: '61',
                recurringTemplateName: 'Draft recurring',
                recurringCandidateCount: 6,
                recurringMatchScore: 0.61,
                recurringMatchReasons: 'draft recurring reason',
                recurringMatchedDate: '2026-07-06',
                selected: true,
                isManuallyAnnotated: true,
                originalCategoryName: serverEvidence,
                parserId: 'parser-new',
                parserTags: ['new']
            });
            expect(transaction.matching).toEqual(matching);
            expect(transaction.matching?.learning).toMatchObject({
                review_status: 'pending',
                lifecycle_status: 'pending',
                signal_state: 'pending',
                score: 0.91
            });
            expect(transaction.matching?.llm).toMatchObject({
                review_status: 'rejected',
                lifecycle_status: 'rejected',
                signal_state: 'resolved'
            });
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('a fresh authoritative learning baseline allows reject to reach the candidate endpoint', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(43);
            const bindings = createBindings(transaction);
            bindings.cacheCurrentPageDrafts();

            setFreshAuthoritativeServerResponse(transaction);
            bindings.rehydrateCurrentPageDrafts();
            await bindings.reviewLearningSuggestion(transaction, 'reject');

            expect(mockRejectMatchingCandidate).toHaveBeenCalledTimes(1);
            expect(mockRejectMatchingCandidate).toHaveBeenCalledWith({
                candidateId: 'preview:43:learning',
                payload: expect.objectContaining({
                    responseMode: 'preview-item'
                })
            });
        } finally {
            warnSpy.mockRestore();
        }
    });
});
