import { describe, expect, jest, test } from '@jest/globals';
import { previewStateSnapshot } from '../../../../helpers/importPreviewState.ts';

const mockGetImportPreviewPage = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAcceptMatchingCandidate = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockRejectMatchingCandidate = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockClearMatchingCandidate = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockGetMatchingSessionCandidates = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockGetLLMMemoryEvents = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockLlmPreviewRecommendAccept = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockLlmPreviewRecommendReject = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockReviewImportTransferDecision = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockConfirmImportPreview = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockUpdateImportPreviewItem = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockGetImportPreviewRowVersionConflict = jest.fn<(...args: Array<unknown>) => any>();
const mockLlmPreviewRecommend = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAnalyzeLLMTransactions = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockGetImportLearningSuggestions = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockPromoteImportLearning = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockShowConfirm = jest.fn();
const mockShowToast = jest.fn();
let mockCurrentToken = '';
const mockExpenseSubcategory = {
    id: '8',
    parentId: '7',
    name: 'Cafe',
    type: 3,
    icon: 'food',
    color: '#ffaa00',
    hidden: false,
    subCategories: []
};
const mockExpenseCategory = {
    id: '7',
    parentId: '0',
    name: 'Food',
    type: 3,
    icon: 'food',
    color: '#ffaa00',
    hidden: false,
    subCategories: [mockExpenseSubcategory]
};
const mockWalletAccount = {
    id: 'wallet',
    name: 'Wallet',
    currency: 'CNY',
    icon: 'wallet',
    color: '#0088ff',
    hidden: false
};

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
        allPlainAccounts: [mockWalletAccount],
        allVisiblePlainAccounts: [mockWalletAccount],
        allAccountsMap: { wallet: mockWalletAccount },
        allAccounts: []
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: { 3: [mockExpenseCategory] },
        allTransactionCategoriesMap: {
            '7': mockExpenseCategory,
            '8': mockExpenseSubcategory
        }
    })
}));
jest.mock('@/stores/transactionTag.ts', () => ({
    useTransactionTagsStore: () => ({
        allTransactionTags: [],
        allTransactionTagsMap: {}
    })
}));

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getImportPreviewPage: mockGetImportPreviewPage,
        acceptMatchingCandidate: mockAcceptMatchingCandidate,
        rejectMatchingCandidate: mockRejectMatchingCandidate,
        clearMatchingCandidate: mockClearMatchingCandidate,
        getMatchingSessionCandidates: mockGetMatchingSessionCandidates,
        getLLMMemoryEvents: mockGetLLMMemoryEvents,
        llmPreviewRecommendAccept: mockLlmPreviewRecommendAccept,
        llmPreviewRecommendReject: mockLlmPreviewRecommendReject,
        reviewImportTransferDecision: mockReviewImportTransferDecision,
        confirmImportPreview: mockConfirmImportPreview,
        updateImportPreviewItem: mockUpdateImportPreviewItem,
        getImportPreviewRowVersionConflict: mockGetImportPreviewRowVersionConflict,
        llmPreviewRecommend: mockLlmPreviewRecommend,
        analyzeLLMTransactions: mockAnalyzeLLMTransactions,
        getImportLearningSuggestions: mockGetImportLearningSuggestions,
        promoteImportLearning: mockPromoteImportLearning
    }
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isTransactionFromAIImageRecognitionEnabled: () => false
}));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => mockCurrentToken }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

jest.mock('@/lib/ui/mobile.ts', () => ({
    showLoading: jest.fn(),
    hideLoading: jest.fn(),
    useI18nUIComponents: () => ({
        showAlert: jest.fn(),
        showConfirm: mockShowConfirm,
        showToast: mockShowToast,
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
    '@/views/desktop/accounts/list/dialogs/EditDialog.vue',
    '@/views/desktop/transactions/list/dialogs/EditDialog.vue'
]) {
    jest.mock(componentPath, () => {
        const { defineComponent, h } = jest.requireActual('vue') as any;
        return {
            __esModule: true,
            default: defineComponent({
                name: 'P3SfcStub',
                inheritAttrs: false,
                setup: (_props: unknown, { attrs, slots }: any) => () => (
                    h('div', attrs, Object.values(slots).flatMap(slot => (
                        typeof slot === 'function' ? (slot as () => unknown[])() : []
                    )))
                )
            })
        };
    });
}

const { createSSRApp, defineComponent, h } = jest.requireActual('vue') as any;
const { renderToString } = jest.requireActual('vue/server-renderer') as any;

import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';
import ImportPreviewPage from '@/views/mobile/transactions/ImportPreviewPage.vue';
import {
    buildImportPreviewIndexSignalViewModel,
    collectImportPreviewIndexAnnotationIssues,
    getPrimaryRecurringReason as getIndexPrimaryRecurringReason,
    mapImportPreviewIndexResponseItem,
    matchesImportPreviewIndexItemFilters,
    resolveImportPreviewIndexPage,
    sortImportPreviewIndexItems
} from '@/views/desktop/transactions/import/import-preview-index/mapping.ts';
import { buildImportPreviewUpdateFromTransaction } from '@/views/desktop/transactions/import/importPreviewUpdates.ts';
import {
    resolveImportPreviewCategoryId,
    resolveImportPreviewCategoryPath,
    resolveImportPreviewDefaultTransferCategoryId
} from '@/views/desktop/transactions/import/importPreview.ts';

function createInteractiveUiStub(): any {
    return defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => (
            h('div', attrs, Object.values(slots).flatMap(slot => (
                typeof slot === 'function' ? (slot as () => unknown[])() : []
            )))
        )
    });
}

const DataTableStub = defineComponent({
    props: {
        items: { type: Array, default: () => [] }
    },
    setup: (props: any, { slots }: any) => () => {
        const children: any[] = [slots.default?.(), slots['header.data-table-select']?.()];
        for (const item of props.items) {
            for (const [name, slot] of Object.entries(slots)) {
                if (name.startsWith('item.') && typeof slot === 'function') {
                    children.push((slot as any)({ item }));
                }
            }
        }
        return h('div', { 'data-testid': 'coverage-data-table' }, children);
    }
});

function registerUiStubs(app: any): void {
    app.component('v-data-table', DataTableStub);
    for (const name of [
        'v-dialog', 'v-card', 'v-card-title', 'v-card-text', 'v-card-actions',
        'v-btn', 'v-chip', 'v-list', 'v-list-item', 'v-list-item-title', 'v-list-item-subtitle',
        'v-checkbox', 'v-menu', 'v-divider', 'v-select', 'v-autocomplete', 'v-text-field',
        'v-tabs', 'v-tab', 'two-column-select', 'icon-select', 'date-range-selection-dialog',
        'transaction-tag', 'item-icon',
        'v-icon', 'v-alert', 'v-progress-circular', 'f7-page', 'f7-navbar', 'f7-nav-left',
        'f7-nav-title', 'f7-nav-right', 'f7-link', 'f7-list', 'f7-list-item', 'f7-block',
        'f7-toolbar', 'f7-sheet', 'f7-page-content', 'f7-block-title', 'f7-button'
    ]) {
        app.component(name, createInteractiveUiStub());
    }
    app.config.warnHandler = () => undefined;
}

function createDesktopDecisionTransaction(): any {
    return {
        _previewId: 77,
        type: 3,
        categoryId: '',
        recurringTemplateId: '',
        recurringTemplateName: '',
        recurringCandidateCount: 0,
        recurringMatchScore: 0,
        recurringMatchReasons: '',
        recurringMatchedDate: '',
        sourceAccountId: '',
        destinationAccountId: '',
        counterparty: 'merchant',
        paymentMethod: 'card',
        comment: 'memo',
        selected: true,
        previewState: previewStateSnapshot(['learning', 'llm']),
        matching: {
            transfer: {
                review_status: '',
                reviewed_type: '',
                suppressed: false
            },
            learning: {
                review_status: 'pending',
                source: 'model',
                model_version: 'model-v1'
            },
            llm: {
                review_status: 'pending',
                reason: 'merchant evidence',
                confidence: 0.9
            }
        },
        getTransferSuggestionReviewStatus: () => '',
        hasTransferSuggestion: () => false,
        isTransferSuggestionAccepted: () => false,
        isTransferSuggestionRejected: () => false,
        getLearningRecommendationInputFingerprint: () => 'stable-fingerprint',
        getLearningRecommendationReviewStatus: () => 'pending',
        hasLearningRecommendation: () => true,
        hasPendingLearningRecommendation: () => true,
        isLearningRecommendationAccepted: () => false,
        isLearningRecommendationRejected: () => false,
        isLearningRecommendationSkipped: () => false,
        isTransferProtectedLearningSkip: () => false,
        isTransactionValid: () => true
    };
}

function createRenderableDesktopTransaction(type: number, index: number): any {
    const transaction = {
        ...createDesktopDecisionTransaction(),
        _previewId: 100 + index,
        index,
        type,
        valid: index % 2 === 0,
        selected: index % 2 === 0,
        isManuallyAnnotated: index === 1,
        parserId: 'wechat',
        parserTags: ['wallet'],
        dedupType: 'platform_duplicate',
        dedupSourceIds: ['source-1'],
        originalCategoryName: 'Cafe',
        categoryId: index % 2 === 0 ? '8' : '',
        sourceAccountId: index % 2 === 0 ? 'wallet' : '',
        destinationAccountId: index % 3 === 0 ? 'wallet' : '',
        counterparty: `Merchant ${index}`,
        paymentMethod: 'Card',
        comment: 'memo',
        amount: index % 2 === 0 ? -12.34 : 12.34,
        destinationAmount: 12.34,
        sourceAmountCents: index % 2 === 0 ? -1234 : 1234,
        destinationAmountCents: 1234,
        currency: 'CNY',
        time: 1783651200,
        utcOffset: index === 0 ? 480 : 0,
        tagIds: [],
        tagNames: [],
        originalTagNames: ['Imported'],
        suggestedType: 3,
        transferSuggestionScore: 0.9,
        transferSuggestionReason: 'paired transfer',
        learningRecommendationReason: 'merchant rule',
        learningRecommendationSummary: 'Food/Cafe',
        recurringTemplateId: index === 2 ? 'recurring-1' : '',
        recurringTemplateName: index === 2 ? 'Monthly transfer' : '',
        recurringCandidateCount: index === 2 ? 2 : 0,
        recurringMatchScore: index === 2 ? 0.88 : 0,
        recurringMatchReasons: index === 2 ? 'date|amount' : '',
        recurringMatchedDate: index === 2 ? '2026-07-10' : '',
        matching: {
            parser: { id: 'wechat', tags: ['wallet'], source_chain: [] },
            dedup: { type: 'platform_duplicate', source_ids: ['source-1'], source_count: 1, source_labels: ['Source'], sources: [] },
            transfer: { review_status: 'pending', reviewed_type: '', suppressed: false, reason: 'paired transfer', source_chain: [] },
            learning: { review_status: 'pending', source: 'model', model_version: 'model-v1', reason: 'merchant rule' },
            llm: {
                review_status: 'pending',
                suggested_type: 'expense',
                suggested_main_category: 'Food',
                suggested_sub_category: 'Cafe',
                reason: 'LLM evidence',
                confidence: 0.9
            },
            reconciliation: {
                planned_operation: 'update_history',
                history_bill_id: 7,
                history_bill_version: 3,
                operation_id: `operation-${index}`,
                acknowledgement_token: `ack-${index}`,
                destructive_ack_required: true,
                notice: 'history rewrite'
            },
            annotation: { history_rewrite_notice: 'history rewrite' }
        },
        previewState: previewStateSnapshot([
            'platform_duplicate',
            'transfer',
            'history',
            'learning',
            'llm'
        ])
    } as any;
    transaction.getTransferSuggestionReviewStatus = () => 'pending';
    transaction.hasTransferSuggestion = () => true;
    transaction.hasPendingLearningRecommendation = () => true;
    transaction.hasRecurringMatch = () => index === 2;
    return transaction;
}

describe('P3 import lifecycle production SFC coverage', () => {
    test('preview index mapping covers empty contracts, all sort keys, and stable pagination', () => {
        const empty = mapImportPreviewIndexResponseItem({
            id: 0,
            preview_date: 'not-a-date'
        } as any);
        const full = mapImportPreviewIndexResponseItem({
            id: 2,
            preview_date: '2026-07-10T00:00:00Z',
            type: 4,
            actual_category_name: 'Transfer',
            category_id: '8',
            actual_source_account_name: 'Wallet',
            actual_destination_account_name: 'Bank',
            source_account_id: 'wallet',
            destination_account_id: 'bank',
            comment: 'memo',
            selected: true,
            source_amount_cents: -100,
            counterparty: 'Merchant',
            payment_method: 'Card',
            parser_source: 'wechat',
            parser_tags: ['wallet'],
            dedup_type: 'platform_duplicate',
            dedup_source_ids: ['source-1'],
            transfer_status: 'pending',
            transfer_title: 'transfer',
            learning_status: null,
            learning_lifecycle_status: 'pending',
            learning_signal_state: 'pending',
            learning_title: 'learning',
            learning_summary: 'summary',
            learning_mode: 'strict',
            learning_rule_id: 7,
            learning_score: 0.8,
            learning_confidence: 0.7,
            learning_margin: 0.2,
            learning_accepted_count: 1,
            learning_rejected_count: 2,
            learning_auto_applied_count: 3,
            llm_status: null,
            llm_lifecycle_status: 'pending',
            llm_signal_state: 'pending',
            llm_title: 'llm',
            llm_confidence: 0.9,
            llm_suggested_category_id: 8,
            llm_category_path: 'Food/Cafe',
            llm_source_account: 'Wallet',
            llm_destination_account: 'Bank',
            history_status: null,
            history_title: 'history',
            history_planned_operation: 'update_history',
            history_bill_id: 9,
            history_bill_version: 2,
            history_operation_id: 'operation-2',
            history_acknowledgement_token: 'ack-2',
            history_destructive_ack_required: true,
            recurring_template_id: 'recurring-2',
            recurring_candidate_count: 2,
            recurring_match_reasons: ' date | amount ',
            recurring_matched_date: '2026-07-10'
        } as any);

        expect(empty.learningStatusAbsent).toBe(true);
        expect(full.learningStatusAbsent).toBe(false);
        expect(getIndexPrimaryRecurringReason(full)).toBe('date');
        expect(getIndexPrimaryRecurringReason({ ...full, recurringMatchReasons: '' })).toBe('');
        expect(buildImportPreviewIndexSignalViewModel(full).hasAnySignal).toBe(true);

        expect(collectImportPreviewIndexAnnotationIssues({
            ...full,
            categoryId: '',
            sourceAccountId: '',
            destinationAccountId: ''
        })).toEqual(expect.arrayContaining([
            'Missing Category',
            'Missing Source Account',
            'Missing Destination Account'
        ]));
        expect(collectImportPreviewIndexAnnotationIssues({
            ...full,
            sourceAccountId: 'same',
            destinationAccountId: 'same'
        })).toContain('Review Transfer Accounts');
        expect(collectImportPreviewIndexAnnotationIssues({
            ...full,
            type: 1,
            categoryId: '',
            sourceAccountId: 'wallet',
            destinationAccountId: ''
        })).toEqual([]);

        expect(matchesImportPreviewIndexItemFilters(full, {} as any)).toEqual(expect.any(Boolean));
        const sortable = [
            { ...full, id: 3, time: 2, type: 3, sourceAmountCents: 200, counterparty: 'B', paymentMethod: 'Z', comment: 'C' },
            { ...full, id: 2, time: 1, type: 4, sourceAmountCents: 100, counterparty: 'A', paymentMethod: 'Y', comment: 'B' },
            { ...full, id: 1, time: 1, type: 4, sourceAmountCents: 100, counterparty: 'A', paymentMethod: 'Y', comment: 'B' },
            { ...full, id: 1, time: 1, type: 4, sourceAmountCents: 100, counterparty: 'A', paymentMethod: 'Y', comment: 'B' }
        ];
        for (const sortKey of [
            'time',
            'type',
            'sourceAmountCents',
            'counterparty',
            'paymentMethod',
            'comment',
            'unknown'
        ]) {
            expect(sortImportPreviewIndexItems(sortable, sortKey, 'asc')).toHaveLength(4);
            expect(sortImportPreviewIndexItems(sortable, sortKey, 'desc')).toHaveLength(4);
        }
        expect(sortImportPreviewIndexItems(sortable, null, null)).toHaveLength(4);
        expect(resolveImportPreviewIndexPage(sortable, 0, 0)).toMatchObject({ page: 1, totalPages: 1 });
        expect(resolveImportPreviewIndexPage(sortable, 99, 2)).toMatchObject({ page: 2, previewIds: [1, 1] });

        const categoryMap: any = {
            primary: { id: 'primary', name: 'Primary', parentId: '', type: undefined, hidden: false },
            child: { id: 'child', name: 'Child', parentId: 'primary', type: undefined, hidden: false },
            hiddenParent: { id: 'hiddenParent', name: 'Hidden', parentId: '', type: 4, hidden: true },
            orphan: { id: 'orphan', name: 'Orphan', parentId: 'missing', type: 4, hidden: false }
        };
        expect(resolveImportPreviewCategoryPath('primary', categoryMap)).toMatchObject({ type: null });
        expect(resolveImportPreviewCategoryPath('child', categoryMap)).toMatchObject({ type: null });
        expect(resolveImportPreviewCategoryPath('orphan', categoryMap)).toBeNull();
        expect(resolveImportPreviewCategoryId({ category_id: 'hiddenParent' } as any, categoryMap)).toBe('');
        expect(resolveImportPreviewDefaultTransferCategoryId(categoryMap, undefined, null)).toBe('');
        expect(resolveImportPreviewDefaultTransferCategoryId(categoryMap, [{
            id: 'transfer',
            name: 'Transfer',
            type: 4,
            hidden: false,
            subCategories: [
                { id: '', name: 'No id', hidden: false },
                { id: 'hidden-child', name: 'Hidden child', hidden: true },
                { id: 'visible-child', name: 'Visible child', hidden: false }
            ]
        }] as any, null)).toBe('visible-child');

        const updateTransaction: any = {
            _previewId: 81,
            type: 3,
            sourceAmountCents: -100,
            destinationAmountCents: 0,
            sourceAccountId: 7,
            destinationAccountId: 0,
            recurringTemplateId: 9,
            recurringTemplateName: '',
            recurringCandidateCount: 0,
            recurringMatchScore: 0,
            recurringMatchReasons: '',
            recurringMatchedDate: '',
            selected: true,
            isManuallyAnnotated: false
        };
        expect(buildImportPreviewUpdateFromTransaction(updateTransaction, {
            categoryPath: null,
            validAccountIds: new Set(['7'])
        })).toMatchObject({
            preview_source_account_id: 7,
            preview_destination_account_id: null,
            preview_recurring_id: 9
        });
        updateTransaction.sourceAccountId = 8;
        expect(buildImportPreviewUpdateFromTransaction(updateTransaction, {
            categoryPath: null,
            validAccountIds: new Set(['7'])
        }).preview_source_account_id).toBeNull();
    });

    test('desktop setup preserves pending LLM state for a semantic no-op edit', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const transaction: any = {
            type: 3,
            categoryId: '',
            recurringTemplateId: '',
            recurringTemplateName: '',
            recurringCandidateCount: 0,
            recurringMatchScore: 0,
            recurringMatchReasons: '',
            recurringMatchedDate: '',
            sourceAccountId: '',
            destinationAccountId: '',
            matching: {
                transfer: {
                    review_status: '',
                    reviewed_type: '',
                    suppressed: false
                },
                llm: {
                    review_status: 'pending',
                    reason: 'merchant evidence',
                    confidence: 0.9
                }
            },
            getTransferSuggestionReviewStatus: () => '',
            getLearningRecommendationInputFingerprint: () => 'stable-fingerprint'
        };
        try {
            const bindings = (ImportTransactionCheckDataTab as any).setup(
                {
                    importTransactions: [transaction],
                    sessionId: 'session-desktop',
                    serverPaged: false
                },
                { emit: jest.fn(), expose: jest.fn() }
            );

            bindings.syncLearningDecisionBaseline(transaction);
            bindings.syncLLMDecisionDraftState(transaction);

            expect(transaction._shouldClearLlmDecision).toBe(false);
            expect(transaction.matching.llm).toMatchObject({
                review_status: 'pending',
                reason: 'merchant evidence',
                confidence: 0.9
            });
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('mobile learning action failure reloads authoritative server state and clears busy', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        mockAcceptMatchingCandidate.mockRejectedValueOnce(new Error('stale action'));
        mockGetImportPreviewPage.mockResolvedValueOnce({
            data: { result: { preview: [] } }
        });
        const back = jest.fn();
        try {
            const bindings = (ImportPreviewPage as any).setup(
                {
                    f7route: { query: { sessionId: 'session-mobile' } },
                    f7router: { back }
                },
                { expose: jest.fn() }
            );
            const row = {
                id: 44,
                record: { matching: { learning: { review_status: 'pending' } } },
                selected: true,
                signal: {},
                busy: false
            };

            await bindings.reviewLearning(row, 'accept');

            expect(mockAcceptMatchingCandidate).toHaveBeenCalledWith({
                candidateId: 'preview:44:learning',
                payload: {
                    expectedState: { sessionId: 'session-mobile' },
                    responseMode: 'preview-item'
                }
            });
            expect(mockGetImportPreviewPage).toHaveBeenCalledWith({
                sessionId: 'session-mobile',
                page: 1,
                pageSize: 500
            });
            expect(mockShowToast).toHaveBeenCalledWith('stale action');
            expect(row.busy).toBe(false);
            expect(back).not.toHaveBeenCalled();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('mobile learning row-version conflict rebases from the 409 snapshot without reloading', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const conflictError = new Error('preview row changed');
        const latest = {
            id: 44,
            row_version: 6,
            preview_selected: false,
            preview_counterparty: 'Authoritative merchant',
            preview_state: previewStateSnapshot(['learning'], { learning: 'accepted' }),
            matching: { learning: { review_status: 'accepted' } }
        };
        mockRejectMatchingCandidate.mockRejectedValueOnce(conflictError);
        mockGetImportPreviewRowVersionConflict.mockReturnValueOnce({
            expected_row_version: 5,
            actual_row_version: 6,
            previewItem: latest
        });
        mockGetImportPreviewPage.mockClear();
        try {
            const bindings = (ImportPreviewPage as any).setup(
                {
                    f7route: { query: { sessionId: 'session-mobile' } },
                    f7router: { back: jest.fn() }
                },
                { expose: jest.fn() }
            );
            const row = {
                id: 44,
                record: {
                    id: 44,
                    row_version: 5,
                    preview_selected: true,
                    preview_state: previewStateSnapshot(['learning']),
                    matching: { learning: { review_status: 'pending' } }
                },
                selected: true,
                signal: {},
                busy: false
            };
            bindings.rows.value = [row];

            await bindings.reviewLearning(row, 'reject');

            expect(mockRejectMatchingCandidate).toHaveBeenCalledWith({
                candidateId: 'preview:44:learning',
                payload: {
                    expectedState: { sessionId: 'session-mobile', rowVersion: 5 },
                    responseMode: 'preview-item'
                }
            });
            expect(row.record).toBe(latest);
            expect(row.selected).toBe(false);
            expect(mockGetImportPreviewPage).not.toHaveBeenCalled();
            expect(row.busy).toBe(false);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('desktop learning row-version conflict rebases from the 409 snapshot without candidate reload', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const transaction = createDesktopDecisionTransaction();
        transaction._rowVersion = 5;
        const latest = {
            id: 77,
            row_version: 6,
            preview_type: 'expense',
            preview_description: 'authoritative memo',
            preview_counterparty: 'authoritative merchant',
            preview_payment_method: 'card',
            matching: {
                ...transaction.matching,
                learning: { ...transaction.matching.learning, review_status: 'accepted' }
            }
        };
        const conflictError = new Error('preview row changed');
        mockRejectMatchingCandidate.mockRejectedValueOnce(conflictError);
        mockGetImportPreviewRowVersionConflict.mockReturnValueOnce({
            expected_row_version: 5,
            actual_row_version: 6,
            previewItem: latest
        });
        mockGetMatchingSessionCandidates.mockClear();
        try {
            const bindings = (ImportTransactionCheckDataTab as any).setup(
                {
                    importTransactions: [transaction],
                    sessionId: 'session-desktop',
                    serverPaged: false
                },
                { emit: jest.fn(), expose: jest.fn() }
            );
            bindings.syncLearningDecisionBaseline(transaction);

            await bindings.reviewLearningSuggestion(transaction, 'reject');

            expect(mockRejectMatchingCandidate).toHaveBeenCalledWith(expect.objectContaining({
                candidateId: 'preview:77:learning',
                payload: expect.objectContaining({
                    expectedState: expect.objectContaining({ rowVersion: 5 })
                })
            }));
            expect(transaction._rowVersion).toBe(6);
            expect(transaction.comment).toBe('authoritative memo');
            expect(mockGetMatchingSessionCandidates).not.toHaveBeenCalled();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('desktop LLM row-version conflict rebases from the 409 snapshot', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const transaction = createDesktopDecisionTransaction();
        transaction._rowVersion = 5;
        const latest = {
            id: 77,
            row_version: 6,
            preview_type: 'expense',
            preview_description: 'authoritative LLM memo',
            preview_counterparty: 'authoritative LLM merchant',
            preview_payment_method: 'card',
            matching: {
                ...transaction.matching,
                llm: { ...transaction.matching.llm, review_status: 'accepted' }
            }
        };
        const conflictError = new Error('preview row changed');
        mockLlmPreviewRecommendReject.mockRejectedValueOnce(conflictError);
        mockGetImportPreviewRowVersionConflict.mockReturnValueOnce({
            expected_row_version: 5,
            actual_row_version: 6,
            previewItem: latest
        });
        mockGetLLMMemoryEvents.mockClear();
        try {
            const bindings = (ImportTransactionCheckDataTab as any).setup(
                {
                    importTransactions: [transaction],
                    sessionId: 'session-desktop',
                    serverPaged: false
                },
                { emit: jest.fn(), expose: jest.fn() }
            );
            mockGetLLMMemoryEvents.mockClear();

            await bindings.reviewLLMRecommendation(transaction, 'reject');

            expect(mockLlmPreviewRecommendReject).toHaveBeenCalledWith(expect.objectContaining({
                sessionId: 'session-desktop',
                previewId: 77,
                expectedState: expect.objectContaining({ rowVersion: 5 })
            }));
            expect(transaction._rowVersion).toBe(6);
            expect(transaction.comment).toBe('authoritative LLM memo');
            expect(mockGetLLMMemoryEvents).not.toHaveBeenCalled();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('mobile LLM row-version conflict rebases from the 409 snapshot without reloading', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const conflictError = new Error('preview row changed');
        const latest = {
            id: 45,
            row_version: 6,
            preview_selected: false,
            preview_counterparty: 'Authoritative LLM merchant',
            preview_state: previewStateSnapshot(['llm'], { llm: 'accepted' }),
            matching: { llm: { review_status: 'accepted' } }
        };
        mockLlmPreviewRecommendAccept.mockRejectedValueOnce(conflictError);
        mockGetImportPreviewRowVersionConflict.mockReturnValueOnce({
            expected_row_version: 5,
            actual_row_version: 6,
            previewItem: latest
        });
        mockGetImportPreviewPage.mockClear();
        try {
            const bindings = (ImportPreviewPage as any).setup(
                {
                    f7route: { query: { sessionId: 'session-mobile' } },
                    f7router: { back: jest.fn() }
                },
                { expose: jest.fn() }
            );
            const row = {
                id: 45,
                record: {
                    id: 45,
                    row_version: 5,
                    preview_selected: true,
                    preview_state: previewStateSnapshot(['llm']),
                    matching: { llm: { review_status: 'pending', confidence: 0.9 } }
                },
                selected: true,
                signal: {},
                busy: false
            };
            bindings.rows.value = [row];

            await bindings.reviewLlm(row, 'accept');

            expect(mockLlmPreviewRecommendAccept).toHaveBeenCalledWith({
                sessionId: 'session-mobile',
                previewId: 45,
                suggestion: { review_status: 'pending', confidence: 0.9 },
                expectedState: { sessionId: 'session-mobile', rowVersion: 5 }
            });
            expect(row.record).toBe(latest);
            expect(row.selected).toBe(false);
            expect(mockGetImportPreviewPage).not.toHaveBeenCalled();
            expect(row.busy).toBe(false);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('desktop decision failures refresh authoritative learning and LLM state', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const transaction = createDesktopDecisionTransaction();
        mockRejectMatchingCandidate.mockRejectedValueOnce(new Error('stale learning action'));
        mockGetMatchingSessionCandidates.mockResolvedValueOnce({
            data: {
                result: {
                    candidates: [{
                        candidate_id: 'preview:77:learning',
                        details: {
                            score: 0.88,
                            reason: 'authoritative learning state',
                            review_status: 'accepted',
                            source: 'model',
                            model_version: 'model-v1'
                        }
                    }]
                }
            }
        });
        mockLlmPreviewRecommendAccept.mockRejectedValueOnce(new Error('stale LLM action'));
        mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
        try {
            const bindings = (ImportTransactionCheckDataTab as any).setup(
                {
                    importTransactions: [transaction],
                    sessionId: 'session-desktop',
                    serverPaged: false
                },
                { emit: jest.fn(), expose: jest.fn() }
            );
            bindings.syncLearningDecisionBaseline(transaction);

            await bindings.reviewLearningSuggestion(transaction, 'reject');
            await bindings.reviewLLMRecommendation(transaction, 'accept');

            mockRejectMatchingCandidate.mockRejectedValueOnce(new Error('second stale learning action'));
            mockGetMatchingSessionCandidates.mockRejectedValueOnce(new Error('authoritative reload unavailable'));
            await bindings.reviewLearningSuggestion(transaction, 'reject');

            expect(mockGetMatchingSessionCandidates).toHaveBeenCalledWith({ sessionId: 'session-desktop' });
            expect(mockGetMatchingSessionCandidates).toHaveBeenCalledTimes(2);
            expect(transaction.learningRecommendationReason).toBe('authoritative learning state');
            expect(transaction.matching.learning.review_status).toBe('accepted');
            expect(mockGetLLMMemoryEvents).toHaveBeenCalledWith({
                session_id: 'session-desktop',
                limit: 500
            });
            expect(bindings.isLearningDecisionBusy(transaction)).toBe(false);
            expect(bindings.isLLMDecisionBusy(transaction)).toBe(false);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('mobile LLM action failure reloads authoritative server state and clears busy', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        mockLlmPreviewRecommendReject.mockRejectedValueOnce(new Error('stale LLM action'));
        mockGetImportPreviewPage.mockResolvedValueOnce({ data: { result: { preview: [] } } });
        try {
            const bindings = (ImportPreviewPage as any).setup(
                {
                    f7route: { query: { sessionId: 'session-mobile' } },
                    f7router: { back: jest.fn() }
                },
                { expose: jest.fn() }
            );
            const row = {
                id: 45,
                record: {
                    matching: {
                        llm: {
                            review_status: 'pending',
                            reason: 'merchant evidence',
                            confidence: 0.9
                        }
                    }
                },
                selected: true,
                signal: {},
                busy: false
            };

            await bindings.reviewLlm(row, 'reject');

            expect(mockLlmPreviewRecommendReject).toHaveBeenCalledWith({
                sessionId: 'session-mobile',
                previewId: 45,
                suggestion: row.record.matching.llm,
                expectedState: { sessionId: 'session-mobile' }
            });
            expect(mockGetImportPreviewPage).toHaveBeenCalledWith({
                sessionId: 'session-mobile',
                page: 1,
                pageSize: 500
            });
            expect(mockShowToast).toHaveBeenCalledWith('stale LLM action');
            expect(row.busy).toBe(false);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('desktop pure-state matrix covers signal, annotation, decision, and paging helpers', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const transaction = {
            ...createDesktopDecisionTransaction(),
            index: 2,
            valid: true,
            tagIds: [],
            originalTagNames: [],
            originalCategoryName: 'Cafe',
            actualCategoryName: '',
            actualSourceAccountName: '',
            actualDestinationAccountName: '',
            currency: 'CNY',
            amount: -12.34,
            destinationAmount: 12.34,
            sourceAmountCents: -1234,
            destinationAmountCents: 1234,
            time: 1783651200,
            utcOffset: 480,
            matching: {
                parser: { source_chain: [] },
                dedup: { source_count: 0, source_labels: [], sources: [] },
                transfer: { review_status: 'pending', reviewed_type: '', suppressed: false },
                learning: {
                    review_status: 'pending',
                    source: 'model',
                    model_version: 'model-v1',
                    reason: 'merchant rule'
                },
                llm: {
                    review_status: 'pending',
                    suggested_type: 'expense',
                    suggested_main_category: 'Food',
                    suggested_sub_category: 'Cafe',
                    suggested_source_account: 'Wallet',
                    suggested_destination_account: '',
                    reason: 'LLM evidence',
                    confidence: 0.9
                },
                reconciliation: {
                    planned_operation: 'update_history',
                    history_bill_id: 7,
                    history_bill_version: 3,
                    operation_id: 'operation-77',
                    acknowledgement_token: 'ack-77',
                    destructive_ack_required: true
                },
                annotation: { history_rewrite_notice: 'history rewrite' }
            },
            previewState: previewStateSnapshot(['transfer', 'history', 'learning', 'llm'])
        } as any;
        transaction.hasRecurringMatch = () => false;
        try {
            const bindings = (ImportTransactionCheckDataTab as any).setup(
                {
                    importTransactions: [transaction],
                    sessionId: 'session-desktop',
                    serverPaged: false
                },
                { emit: jest.fn(), expose: jest.fn() }
            );

            expect(bindings.getPreviewId(transaction)).toBe(77);
            expect(bindings.buildTransferDecisionBaseline(transaction)).toMatchObject({
                type: 3,
                reviewStatus: ''
            });
            bindings.syncTransferDecisionBaseline(transaction);
            bindings.syncLearningDecisionBaseline(transaction);
            expect(bindings.getLearningDecisionBaseline(transaction).inputFingerprint).toBe('stable-fingerprint');
            expect(bindings.hasLearningDecisionTextDraftChanges(transaction)).toBe(false);
            expect(bindings.shouldBlockLearningDecisionOnSync(transaction)).toBe(false);

            const llm = transaction.matching.llm;
            expect(bindings.getLLMSignalType(llm)).toBe('expense');
            expect(bindings.getLLMSignalCategoryPath(llm)).toBe('Food/Cafe');
            expect(bindings.getLLMSignalAccountRoute(llm)).toBe('Wallet');
            expect(bindings.getLLMSignalStatus(transaction)).toBe('pending');
            expect(bindings.buildLLMSignalSummary(llm)).toContain('Food/Cafe');
            expect(bindings.getImportPreviewSignalViewModel(transaction).learning?.status).toBe('pending');
            for (const [status, expected] of [
                ['accepted', 'accepted'],
                ['rejected', 'rejected'],
                ['skipped', 'skipped'],
                ['none', null]
            ] as const) {
                const statusTransaction = createRenderableDesktopTransaction(3, 60);
                statusTransaction.matching.learning = { reason: 'legacy helper status' };
                statusTransaction.hasPendingLearningRecommendation = () => false;
                statusTransaction.isLearningRecommendationAccepted = () => status === 'accepted';
                statusTransaction.isLearningRecommendationRejected = () => status === 'rejected';
                statusTransaction.isLearningRecommendationSkipped = () => status === 'skipped';
                if (status === 'none') {
                    statusTransaction.matching.learning = {};
                    statusTransaction.learningRecommendationReason = '';
                    statusTransaction.learningRecommendationSummary = '';
                }
                statusTransaction.previewState = status === 'none'
                    ? previewStateSnapshot(['platform_duplicate', 'history', 'llm'])
                    : previewStateSnapshot(
                        ['platform_duplicate', 'history', 'learning', 'llm'],
                        { learning: status },
                    );
                const learningSignal = bindings.getImportPreviewSignalViewModel(statusTransaction).learning;
                expect(learningSignal?.status ?? null).toBe(expected);
            }
            const absentLearningTransaction = createRenderableDesktopTransaction(3, 61);
            absentLearningTransaction.matching.learning = {};
            absentLearningTransaction.learningRecommendationReason = '';
            absentLearningTransaction.learningRecommendationSummary = '';
            absentLearningTransaction.hasLearningRecommendation = () => false;
            absentLearningTransaction.previewState = previewStateSnapshot([
                'platform_duplicate',
                'transfer',
                'history',
                'llm'
            ]);
            expect(bindings.getImportPreviewSignalViewModel(absentLearningTransaction).learning).toBeNull();

            expect(bindings.shouldClearTransferDecisionOnSync(transaction)).toBe(false);
            expect(bindings.shouldClearLearningDecisionOnSync(transaction)).toBe(false);
            expect(bindings.shouldClearLlmDecisionOnSync(transaction)).toBe(false);
            expect(bindings.hasDecisionLoadingId([], 77)).toBe(false);
            expect(bindings.isPreviewMatchingDecisionBusy(null)).toBe(false);
            expect(bindings.isTransferDecisionBusy(transaction)).toBe(false);
            expect(bindings.isLearningDecisionBusy(transaction)).toBe(false);
            expect(bindings.isLLMDecisionBusy(transaction)).toBe(false);
            expect(bindings.getTransferDecisionExpectedState(transaction)).toMatchObject({ sessionId: 'session-desktop' });
            expect(bindings.getLearningDecisionExpectedState(transaction)).toMatchObject({ sessionId: 'session-desktop' });

            expect(bindings.getActionErrorMessage({ response: { data: { error: ' server conflict ' } } }, 'fallback')).toBe('server conflict');
            expect(bindings.getActionErrorMessage(new Error('local error'), 'fallback')).toBe('local error');
            expect(bindings.getActionErrorMessage({}, 'fallback')).toBe('fallback');
            expect(bindings.getLLMAnalysisErrorDetails({ response: { status: 409, data: { code: 'stale' } } })).toEqual({
                code: 'stale',
                status: 409
            });

            const preview = { id: 77, preview_category_id: '8' };
            expect(bindings.resolvePreviewCategoryId(preview)).toBe('');
            expect(bindings.resolvePreviewDecisionItem({ previewItem: preview }, 77)).toBe(preview);
            expect(bindings.resolvePreviewDecisionItem({ preview: [preview] }, 77)).toBe(preview);
            expect(bindings.getTransferDecisionMessageKey('accept')).toBe('Transfer Suggestion Accepted');
            expect(bindings.getTransferDecisionMessageKey('reject')).toBe('Transfer Suggestion Rejected');
            expect(bindings.getTransferDecisionMessageKey('clear')).toBe('Clear Transfer Decision');
            expect(bindings.getLearningDecisionMessageKey('accept')).toBe('Learning Suggestion Accepted');
            expect(bindings.getLearningDecisionMessageKey('reject')).toBe('Learning Suggestion Rejected');
            expect(bindings.getLearningDecisionMessageKey('clear')).toBe('Clear Learning Decision');

            const firstSignalViewModel = bindings.getImportPreviewSignalViewModel(transaction);
            expect(firstSignalViewModel.hasAnySignal).toBe(true);
            expect(bindings.getImportPreviewSignalViewModel(transaction)).toBe(firstSignalViewModel);
            transaction.matching.learning.score = 0.88;
            expect(bindings.getImportPreviewSignalViewModel(transaction)).not.toBe(firstSignalViewModel);
            expect(bindings.getImportPreviewHistoryRewriteOperation(transaction)).toMatchObject({
                preview_id: 77,
                planned_operation: 'update_history'
            });
            expect(bindings.getImportTransactionRowKey(transaction)).toContain('77');
            expect(bindings.getAnnotationSummary(transaction)).toEqual(expect.any(String));
            expect(bindings.needsAnnotation(transaction)).toBe(true);

            expect(bindings.metadataFacetLabels([{ label: 'Food', count: 2 }])).toEqual(['Food']);
            expect(bindings.metadataAccountFacetLabels([{ label: 'Wallet', count: 1, value: 'wallet' }])).toEqual(['Wallet']);
            expect(bindings.buildFacetValueByLabel([{ label: 'Food', count: 2, value: 'food' }])).toEqual({ Food: 'food' });
            expect(bindings.buildAccountFacetValueByLabel([{ label: 'Wallet', count: 1, value: 'wallet' }])).toEqual({ Wallet: 'wallet' });

            expect(bindings.getDisplayCount(1234)).toBeTruthy();
            expect(bindings.getTablePageOptions(25).length).toBeGreaterThan(0);
            expect(bindings.isTransactionDisplayed(transaction)).toBe(true);
            expect(bindings.isTagValid([], 0)).toBe(false);
            expect(bindings.getTransactionDisplayAmount(transaction)).toBeTruthy();
            expect(bindings.getTransactionDisplayDestinationAmount(transaction)).toBeTruthy();
            expect(bindings.isKnownAccountId('missing')).toBe(false);
            expect(bindings.getCurrentPreviewPage()).toBe(1);
            expect(bindings.getCurrentPreviewPageSize()).toBeGreaterThan(0);

            for (const helperName of [
                'getNeedsAnnotationText',
                'getSelectAllAnnotationText',
                'getAnnotationActionText',
                'getAnnotationFilterTitle',
                'getNeedsReviewOrAnnotatedText',
                'getNoAnnotationIssuesText',
                'getManuallyAnnotatedText',
                'getAnnotationDialogTitle',
                'getAnnotationDialogDescription',
                'getNoSelectedAnnotationText'
            ]) {
                expect(bindings[helperName]()).toEqual(expect.any(String));
            }

            expect(bindings.getCategoriesForType(3)).toHaveLength(1);
            expect(bindings.getDefaultCategoryIdForTransactionType(3)).toBeTruthy();
            expect(bindings.hasAvailableCategoriesForType(3)).toBe(true);
            expect(bindings.getCategoryPrimaryText(transaction)).toBe('');
            expect(bindings.getCategorySecondaryText(transaction)).toBe('');
            expect(bindings.requiresDestinationAccount({ type: 4 })).toBe(true);
            expect(bindings.getDestinationAccountTitle({ type: 4 })).toBeTruthy();
            expect(bindings.getRecurringDecisionMessageKey(true)).toBeTruthy();
            expect(bindings.getRecurringDecisionMessageKey(false)).toBeTruthy();

            const recurringCandidate = {
                id: 'recurring-1',
                name: 'Monthly transfer',
                amount: 12.34,
                currency: 'CNY',
                matchReasons: ['date', 'amount']
            };
            expect(bindings.formatRecurringCandidateSubtitle(recurringCandidate)).toEqual(expect.any(String));
            expect(bindings.isBestRecurringCandidate(recurringCandidate)).toBe(false);
            expect(bindings.getRecurringCandidatePrimaryReason(recurringCandidate)).toBe('date');
            transaction.recurringMatchReasons = ' date | amount ';
            expect(bindings.getPrimaryRecurringReason(transaction)).toBe('date');
            expect(bindings.getRecurringMatchSummary(transaction)).toContain('date');
            bindings.closeRecurringCandidateDialog();

            const mutableSignalTransaction = createRenderableDesktopTransaction(3, 8);
            bindings.syncTransferDecisionBaseline(mutableSignalTransaction);
            expect(bindings.hasTransferDecisionRelevantDraftChanges(mutableSignalTransaction)).toBe(false);
            bindings.restoreTransferSuggestionDecisionState(mutableSignalTransaction, {
                reviewStatus: 'accepted',
                reviewedType: 'expense',
                suppressed: false
            });
            expect(mutableSignalTransaction.matching.transfer.review_status).toBe('accepted');
            bindings.clearTransferSuggestionState(mutableSignalTransaction);
            expect(mutableSignalTransaction.transferSuggestionReason).toBe('');
            bindings.clearLearningRecommendationState(mutableSignalTransaction);
            expect(mutableSignalTransaction.learningRecommendationReason).toBe('');
            bindings.clearLLMRecommendationState(mutableSignalTransaction);
            expect(mutableSignalTransaction.matching.llm.review_status).toBe('');

            const blankLlmTransaction = createRenderableDesktopTransaction(3, 9);
            blankLlmTransaction.matching = {};
            expect(bindings.ensureLLMMatchingPayload(blankLlmTransaction)).toEqual({});
            bindings.mergeLLMMatchingFromPayload(blankLlmTransaction, {
                review_status: 'pending',
                reason: 'merged LLM state'
            });
            expect(blankLlmTransaction.matching.llm.reason).toBe('merged LLM state');
            blankLlmTransaction.matching.transfer = { review_status: 'pending', reviewed_type: '', suppressed: false };
            blankLlmTransaction.matching.learning = { review_status: 'pending' };
            expect(bindings.mergePreviewMatchingPayload(
                { annotation: { issue: 'stale' }, llm: { reason: 'old LLM' } },
                { transfer: { review_status: 'accepted' }, llm: { reason: 'new LLM', review_status: 'pending' } }
            )).toMatchObject({
                transfer: { review_status: 'accepted' },
                llm: { reason: 'new LLM' }
            });

            const refreshedPreview = {
                id: 77,
                preview_type: 'expense',
                preview_category_id: '8',
                preview_source_account_id: 'wallet',
                preview_destination_account_id: 'wallet',
                preview_amount_cents: -1234,
                preview_destination_amount_cents: 1234,
                preview_description: 'refreshed memo',
                preview_counterparty: 'Refreshed Merchant',
                preview_payment_method: 'Wallet',
                preview_parser_id: 'wechat',
                preview_parser_tags: ['wallet'],
                matching: {
                    parser: { source_chain: [] },
                    dedup: { source_count: 0, source_labels: [], sources: [] },
                    transfer: { review_status: 'accepted', suppressed: false, source_chain: [] },
                    learning: { review_status: 'accepted' },
                    llm: { review_status: 'accepted', reason: 'refreshed LLM' }
                }
            };
            bindings.syncTransactionFromLLMPreviewPayload(blankLlmTransaction, {
                preview: refreshedPreview,
                matching: { llm: refreshedPreview.matching.llm }
            });
            expect(blankLlmTransaction.comment).toBe('refreshed memo');
            bindings.syncTransactionFromPreviewDecision(transaction, refreshedPreview);
            expect(transaction.matching.transfer.review_status).toBe('accepted');

            expect(bindings.getLLMAnalysisErrorMessageKey({ code: 'missing_api_key', status: 400 })).toBeTruthy();
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: 'timeout', status: 504 })).toBeTruthy();
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: '', status: 500 })).toBeTruthy();

            transaction.matching.annotation = {
                type: 'missing_category',
                text: 'Missing category',
                history_rewrite_notice: 'history rewrite'
            };
            expect(bindings.hasBaselineAnnotationIssue(transaction)).toEqual(expect.any(Boolean));
            expect(bindings.hasCurrentAnnotationIssue(transaction)).toEqual(expect.any(Boolean));
            expect(bindings.getAnnotationSummary(transaction)).toBeTruthy();
            expect(bindings.getAnnotationListTitle(transaction)).toBeTruthy();

            expect(bindings.buildPreviewUpdates({ selectedOnly: false })).toEqual(expect.any(Array));
            expect(bindings.buildSelectedPreviewUpdates()).toEqual(expect.any(Array));
            expect(bindings.buildTrackedPreviewUpdates()).toEqual(expect.any(Array));
            expect(bindings.getTrackedTransactionByPreviewId(77)).toBe(transaction);
            expect(bindings.metadataFacetLabels(undefined)).toEqual([]);
            expect(bindings.metadataAccountFacetLabels(undefined)).toEqual([]);
            expect(bindings.buildFacetValueByLabel(undefined)).toEqual({});
            expect(bindings.buildAccountFacetValueByLabel(undefined)).toEqual({});
            expect(bindings.buildServerPreviewQueryFilters()).toEqual(expect.any(Object));
            expect(bindings.cloneImportTransaction(transaction)).not.toBe(transaction);
            expect(bindings.getCurrentServerPagedSortRequest()).toEqual(expect.any(Object));
            expect(bindings.getCurrentServerPagedRequestOptions()).toEqual(expect.any(Object));
            expect(bindings.isImportTransactionColumnSortable('amount')).toBe(true);
            expect(bindings.getSourceAccountTitle(transaction)).toEqual(expect.any(String));
            expect(bindings.getSourceAccountDisplayName(transaction)).toEqual(expect.any(String));
            expect(bindings.getDestinationAccountDisplayName(transaction)).toEqual(expect.any(String));
            expect(bindings.getCurrentInvalidCategoryNames(3)).toEqual(expect.any(Array));
            expect(bindings.getCurrentInvalidAccountNames()).toEqual(expect.any(Array));
            expect(bindings.getCurrentInvalidTagNames()).toEqual(expect.any(Array));
            expect(bindings.getAllOriginalTagNames()).toEqual(expect.any(Array));

            const managedTransaction = createRenderableDesktopTransaction(3, 12);
            bindings.setTransactionCategoryFromId(managedTransaction, '8');
            expect(managedTransaction.categoryId).toBe('8');
            bindings.onTransactionTypeChange(managedTransaction);
            bindings.onTransactionDataDraftChange(managedTransaction);
            bindings.openCategoryManagement();
            bindings.openAccountManagement();
            bindings.applyCreatedCategoryToTransaction(managedTransaction, undefined);
            bindings.applyCreatedCategoryToTransaction(managedTransaction, mockExpenseSubcategory);
            expect(managedTransaction.categoryId).toBe('8');
            bindings.applyCreatedAccountToTransaction(managedTransaction, 'source', undefined);
            bindings.applyCreatedAccountToTransaction(managedTransaction, 'source', mockWalletAccount);
            bindings.applyCreatedAccountToTransaction(managedTransaction, 'destination', mockWalletAccount);
            expect(managedTransaction.sourceAccountId).toBe('wallet');
            expect(managedTransaction.destinationAccountId).toBe('wallet');
            await bindings.quickCreatePrimaryCategory(managedTransaction);
            await bindings.quickCreateSecondaryCategory(managedTransaction, mockExpenseCategory);
            await bindings.quickCreateSecondaryCategory(managedTransaction, undefined);
            await bindings.quickCreateAccount(managedTransaction, 'source', { category: 1 });
            expect(bindings.getManageCategoryItems()).toHaveLength(1);
            expect(bindings.getSelectedManagePrimaryCategory()).toBeUndefined();
            bindings.openManagedPrimaryCategoryCreateDialog();
            bindings.openManagedSecondaryCategoryCreateDialog();
            bindings.openManagedAccountCreateDialog();
            bindings.openSelectedCategoryEditDialog();
            bindings.openSelectedAccountEditDialog();

            bindings.batchCategoryId.value = '8';
            bindings.batchCategoryType.value = 3;
            bindings.applyBatchCategory();
            bindings.batchAccountId.value = 'wallet';
            bindings.applyBatchAccount();
            expect(transaction.categoryId).toBe('8');
            expect(transaction.sourceAccountId).toBe('wallet');

            expect(bindings.getUniqueTrackedServerPagedTransactions()).toHaveLength(1);
            expect(bindings.getServerPagedSelectionDelta()).toEqual(expect.objectContaining({ selected: expect.any(Number) }));
            expect(bindings.getServerPagedAnnotationDelta()).toEqual(expect.any(Number));
            bindings.emitServerPagedRequest(2, 20, { force: true, sortKey: 'amount', sortDirection: 'desc' });
            bindings.updatePreviewTablePage(3);
            bindings.updatePreviewTablePageSize(25);
            bindings.updatePreviewTableSort([{ key: 'amount', order: 'asc' }]);
            expect(bindings.getCurrentPreviewPage()).toBe(1);

            for (const action of ['select_all', 'select_valid', 'select_invalid', 'select_needs_annotation', 'select_none', 'invert']) {
                bindings.applySelectionActionToLocalTransactions(action, [transaction]);
            }
            await bindings.selectAllValid();
            await bindings.selectAllInvalid();
            await bindings.selectAllNeedsAnnotation();
            await bindings.selectAll();
            await bindings.selectNone();
            await bindings.selectInvert();
            bindings.selectAllInThisPage();
            bindings.selectNoneInThisPage();
            bindings.selectInvertInThisPage();
            bindings.openAnnotationDialog();
            bindings.changeCustomDateFilter(100, 200);
            bindings.onShowDateRangeError('invalid range');
            bindings.setCountPerPage(50);
            expect(bindings.getSelectedPreviewUpdates()).toEqual(expect.any(Array));
            expect(bindings.getSelectedPreviewCount()).toEqual(expect.any(Number));
            expect(bindings.getSelectedPreviewIds()).toEqual(expect.any(Array));
            expect(bindings.getSelectedHistoryRewriteOperations()).toEqual(expect.any(Array));
            bindings.reset();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('desktop signal merge, memory, draft, and error branches preserve authoritative state', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const transaction = createRenderableDesktopTransaction(3, 20);
        const bindings = (ImportTransactionCheckDataTab as any).setup(
            {
                importTransactions: [transaction],
                sessionId: 'session-desktop-branches',
                serverPaged: false
            },
            { emit: jest.fn(), expose: jest.fn() }
        );
        try {
            const fullCandidate = {
                candidate_id: `preview:${transaction._previewId}:learning`,
                details: {
                    rule_id: 42,
                    score: 0.8,
                    level: 'high',
                    reason: 'learned merchant',
                    recommended_type: 'expense',
                    summary: 'Food/Cafe',
                    review_status: 'accepted',
                    suppressed: true,
                    source: 'rule',
                    mode: 'strict',
                    auto_apply: true,
                    model_version: 'model-v2',
                    recommendation_key: 'merchant:test',
                    lifecycle_status: 'accepted',
                    signal_state: 'accepted',
                    accepted_count: 3,
                    rejected_count: 2,
                    auto_applied_count: 1
                }
            };
            bindings.syncLearningCandidateFromSessionCandidate(transaction, fullCandidate);
            expect(transaction.learningRecommendationScore).toBe(0.8);
            expect(transaction.matching.learning).toMatchObject({
                rule_id: 42,
                review_status: 'accepted',
                suppressed: true,
                auto_apply: true
            });

            const noMatchingTransaction = createRenderableDesktopTransaction(3, 21);
            noMatchingTransaction.matching = undefined;
            bindings.syncLearningCandidateFromSessionCandidate(noMatchingTransaction, { candidate_id: 'empty' });
            expect(noMatchingTransaction.learningRecommendationScore).toBe(0);
            expect(bindings.getLLMMatchingPayload(noMatchingTransaction)).toEqual({});
            expect(bindings.ensureLLMMatchingPayload(noMatchingTransaction)).toEqual({});
            const emptyDetailsTransaction = createRenderableDesktopTransaction(3, 34);
            bindings.syncLearningCandidateFromSessionCandidate(emptyDetailsTransaction, {
                candidate_id: 'empty-details',
                details: { rule_id: 'not-a-number' }
            });
            expect(emptyDetailsTransaction.matching.learning.rule_id).toBeNull();

            expect(bindings.getLLMSignalCategoryPath({ suggested_sub_category: 'Cafe' })).toBe('Cafe');
            expect(bindings.getLLMSignalAccountRoute({ suggested_destination_account: 'Wallet' })).toBe('Wallet');
            expect(bindings.getLLMSignalStatus({ matching: { llm: { review_status: 'accepted' } } })).toBe('accepted');
            expect(bindings.getLLMSignalStatus({ matching: { llm: { review_status: 'rejected' } } })).toBe('rejected');
            expect(bindings.getLLMSignalStatus({ matching: { llm: { reason: 'evidence', suppressed: true } } })).toBeNull();
            expect(bindings.getLLMSignalStatus({ matching: { llm: { confidence: 0.5 } } })).toBe('pending');
            expect(bindings.getLLMSignalStatus({ matching: { llm: {} } })).toBeNull();
            expect(bindings.buildLLMSignalSummary({})).toBe('');

            const currentMatching = {
                annotation: { issue: 'stale' },
                identity_validation: { valid: false },
                llm: { reason: 'old reason', confidence: 0.2 }
            };
            expect(bindings.mergePreviewMatchingPayload(currentMatching, undefined)).toBe(currentMatching);
            expect(bindings.mergePreviewMatchingPayload(undefined, {
                annotation: { issue: 'new' },
                identity_validation: { valid: true },
                llm: {}
            })).toMatchObject({
                annotation: { issue: 'new' },
                identity_validation: { valid: true },
                llm: {}
            });
            expect(bindings.mergePreviewMatchingPayload(currentMatching, { transfer: { review_status: 'accepted' } })).toEqual(expect.not.objectContaining({
                annotation: expect.anything(),
                identity_validation: expect.anything()
            }));
            bindings.mergeLLMMatchingFromPayload(transaction, undefined);
            bindings.mergeLLMMatchingFromPayload(transaction, {});
            expect(transaction.matching.llm.reason).toBe('');

            const sparsePayloadTransaction = createRenderableDesktopTransaction(3, 22);
            bindings.syncTransactionFromLLMPreviewPayload(sparsePayloadTransaction, {
                preview: {
                    id: sparsePayloadTransaction._previewId,
                    preview_type: 'unknown',
                    preview_source_account_id: null,
                    preview_destination_account_id: null,
                    preview_description: '',
                    preview_counterparty: '',
                    preview_payment_method: '',
                    preview_parser_id: '',
                    preview_parser_tags: null
                },
                matching: null
            });
            expect(sparsePayloadTransaction.comment).toBe('memo');
            bindings.syncTransactionFromLLMPreviewPayload(sparsePayloadTransaction, {});
            bindings.applyLLMSignalMemoryToTransactions();

            bindings.llmSessionSignalMemory.value = new Map([
                [transaction._previewId, {
                    previewId: transaction._previewId,
                    suggestedType: 'expense',
                    suggestedCategoryId: '8',
                    suggestedMainCategory: 'Food',
                    suggestedSubCategory: 'Cafe',
                    suggestedSourceAccount: 'Wallet',
                    suggestedDestinationAccount: '',
                    confidence: 0.95,
                    reason: 'memory evidence',
                    reviewStatus: 'accepted',
                    suppressed: false
                }]
            ]);
            bindings.applyLLMSignalMemoryToTransactions([
                { matching: {} },
                createRenderableDesktopTransaction(3, 23),
                transaction
            ]);
            expect(transaction.matching.llm.reason).toBe('memory evidence');
            await bindings.refreshLLMSessionSignalMemory();

            mockGetLLMMemoryEvents.mockResolvedValueOnce({ data: { result: { events: [] } } });
            await bindings.refreshLLMSessionSignalMemory(true);
            mockGetLLMMemoryEvents.mockResolvedValueOnce({ data: { result: { events: null } } });
            await bindings.refreshLLMSessionSignalMemory(true);
            mockGetLLMMemoryEvents.mockRejectedValueOnce(new Error('memory unavailable'));
            await bindings.refreshLLMSessionSignalMemory(true);

            const emptySessionBindings = (ImportTransactionCheckDataTab as any).setup(
                { importTransactions: [], sessionId: '', serverPaged: false },
                { emit: jest.fn(), expose: jest.fn() }
            );
            await emptySessionBindings.refreshLLMSessionSignalMemory();
            expect(emptySessionBindings.llmSessionSignalMemory.value.size).toBe(0);

            const draftTransaction = createRenderableDesktopTransaction(3, 24);
            expect(bindings.hasTransferDecisionRelevantDraftChanges(draftTransaction)).toBe(false);
            bindings.syncTransferDecisionDraftState(draftTransaction);
            draftTransaction.categoryId = 'changed';
            bindings.syncTransferDecisionDraftState(draftTransaction);
            expect(bindings.shouldClearTransferDecisionOnSync(draftTransaction)).toBe(true);

            const acceptedTransfer = createRenderableDesktopTransaction(3, 25);
            acceptedTransfer.matching.transfer.review_status = 'accepted';
            bindings.syncTransferDecisionBaseline(acceptedTransfer);
            acceptedTransfer.comment = 'text-only edit';
            bindings.syncTransferDecisionDraftState(acceptedTransfer);
            expect(bindings.shouldClearTransferDecisionOnSync(acceptedTransfer)).toBe(false);
            bindings.restoreTransferSuggestionDecisionState({ matching: {} }, {
                reviewStatus: 'accepted',
                reviewedType: 'expense',
                suppressed: false
            });
            const transferWithoutMatching = createRenderableDesktopTransaction(3, 35);
            transferWithoutMatching.matching = undefined;
            bindings.clearTransferSuggestionState(transferWithoutMatching);

            const learningDraft = createRenderableDesktopTransaction(3, 26);
            learningDraft.getLearningRecommendationInputFingerprint = () => learningDraft.comment;
            bindings.syncLearningDecisionBaseline(learningDraft);
            learningDraft.comment = 'changed learning evidence';
            bindings.syncLearningDecisionDraftState(learningDraft);
            bindings.syncLearningDecisionDraftState(learningDraft);
            expect(bindings.shouldClearLearningDecisionOnSync(learningDraft)).toBe(true);

            const noPendingLearning = createRenderableDesktopTransaction(3, 27);
            noPendingLearning.hasPendingLearningRecommendation = () => false;
            bindings.syncLearningDecisionDraftState(noPendingLearning);
            expect(bindings.shouldClearLearningDecisionOnSync(noPendingLearning)).toBe(false);

            const llmDraft = createRenderableDesktopTransaction(3, 28);
            llmDraft.getLearningRecommendationInputFingerprint = () => llmDraft.counterparty;
            bindings.syncLearningDecisionBaseline(llmDraft);
            llmDraft.counterparty = 'changed merchant';
            bindings.syncLLMDecisionDraftState(llmDraft);
            bindings.syncLLMDecisionDraftState(llmDraft);
            expect(bindings.shouldClearLlmDecisionOnSync(llmDraft)).toBe(true);
            const noPendingLlm = createRenderableDesktopTransaction(3, 29);
            noPendingLlm.matching.llm = {};
            bindings.syncLLMDecisionDraftState(noPendingLlm);
            expect(bindings.shouldClearLlmDecisionOnSync(noPendingLlm)).toBe(false);

            bindings.commitEditingTransactionDraft();
            bindings.editingTransaction.value = transaction;
            bindings.editingTags.value = ['tag-1'];
            bindings.commitEditingTransactionDraft();
            expect(transaction.tagIds).toEqual(['tag-1']);

            bindings.addDecisionLoadingId(bindings.transferDecisionLoadingIds, transaction._previewId);
            bindings.addDecisionLoadingId(bindings.transferDecisionLoadingIds, transaction._previewId);
            expect(bindings.isPreviewMatchingDecisionBusy(transaction._previewId)).toBe(true);
            expect(bindings.isTransferDecisionBusy(transaction)).toBe(true);
            bindings.removeDecisionLoadingId(bindings.transferDecisionLoadingIds, transaction._previewId);
            bindings.removeDecisionLoadingId(bindings.transferDecisionLoadingIds, transaction._previewId);
            expect(bindings.isPreviewMatchingDecisionBusy(transaction._previewId)).toBe(false);

            expect(bindings.getLLMAnalysisErrorDetails({ response: { data: { error_code: 'fallback-code' } } })).toEqual({
                code: 'fallback-code',
                status: undefined
            });
            expect(bindings.getLLMAnalysisErrorDetails({ response: { data: { error: 500 } } })).toEqual({
                code: '',
                status: undefined
            });
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: 'LLM_DISABLED' })).toContain('disabled');
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: 'IMPORT_SESSION_NOT_FOUND' })).toContain('missing');
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: '', status: 404 })).toContain('missing');
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: 'PREVIEW_SELECTION_EMPTY' })).toContain('insufficient');
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: 'PREVIEW_SELECTION_INSUFFICIENT' })).toContain('insufficient');
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: '', status: 422 })).toContain('insufficient');
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: 'PREVIEW_SELECTION_TOO_LARGE' })).toContain('Too many');
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: 'LLM_PROVIDER_UNAVAILABLE' })).toContain('connection failed');
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: '', status: 503 })).toContain('connection failed');
            expect(bindings.getLLMAnalysisErrorMessageKey({ code: 'UNKNOWN' })).toContain('analysis failed');

            expect(emptySessionBindings.getTransferDecisionExpectedState(transaction)).toMatchObject({ sessionId: '' });
            expect(emptySessionBindings.getLearningDecisionExpectedState(transaction)).toMatchObject({ sessionId: '' });

            const unresolvedPreviewTransaction = createRenderableDesktopTransaction(3, 36);
            unresolvedPreviewTransaction.sourceAmountCents = 0;
            unresolvedPreviewTransaction.destinationAmountCents = 0;
            bindings.syncTransactionFromPreviewDecision(unresolvedPreviewTransaction, {
                id: unresolvedPreviewTransaction._previewId,
                preview_type: 'unknown',
                preview_main_category: 'Imported Main',
                preview_sub_category: 'Imported Sub',
                preview_source_account_id: null,
                preview_destination_account_id: null,
                preview_amount_cents: undefined,
                preview_destination_amount_cents: undefined,
                dedup_source_ids: null,
                preview_recurring_id: 'recurring-36',
                preview_recurring_name: 'Recurring 36'
            });
            expect(unresolvedPreviewTransaction.originalCategoryName).toBe('Imported Sub');
            expect(unresolvedPreviewTransaction.recurringTemplateId).toBe('recurring-36');
            bindings.syncTransactionFromPreviewDecision(unresolvedPreviewTransaction, {
                id: unresolvedPreviewTransaction._previewId,
                preview_type: 'unknown',
                preview_main_category: 'Main Only',
                preview_sub_category: ''
            });
            expect(unresolvedPreviewTransaction.originalCategoryName).toBe('Main Only');
            bindings.syncTransactionFromPreviewDecision(unresolvedPreviewTransaction, {
                id: unresolvedPreviewTransaction._previewId,
                preview_type: 'unknown',
                preview_main_category: '',
                preview_sub_category: ''
            });
            expect(unresolvedPreviewTransaction.originalCategoryName).toBe('');

            const transferPreviewTransaction = createRenderableDesktopTransaction(3, 37);
            bindings.syncTransactionFromPreviewDecision(transferPreviewTransaction, {
                id: transferPreviewTransaction._previewId,
                preview_type: 'transfer',
                preview_category_id: '',
                dedup_source_ids: ['source-37']
            });
            expect(transferPreviewTransaction.dedupSourceIds).toEqual(['source-37']);

            expect(bindings.resolvePreviewDecisionItem({ previewItem: { id: 999 } }, transaction._previewId)).toBeNull();
            expect(bindings.resolvePreviewDecisionItem(null, transaction._previewId)).toBeNull();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('desktop decision actions cover guards, authoritative success payloads, and stale responses', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const transaction = createRenderableDesktopTransaction(3, 30);
        const bindings = (ImportTransactionCheckDataTab as any).setup(
            {
                importTransactions: [transaction],
                sessionId: 'session-desktop-decisions',
                serverPaged: false
            },
            { emit: jest.fn(), expose: jest.fn() }
        );
        const emptySessionBindings = (ImportTransactionCheckDataTab as any).setup(
            { importTransactions: [transaction], sessionId: '', serverPaged: false },
            { emit: jest.fn(), expose: jest.fn() }
        );
        const previewFor = (item: any, overrides: Record<string, unknown> = {}) => ({
            id: item._previewId,
            preview_type: 'expense',
            preview_category_id: '8',
            preview_source_account_id: 'wallet',
            preview_destination_account_id: '',
            preview_amount_cents: -1234,
            preview_destination_amount_cents: 0,
            preview_description: item.comment,
            preview_counterparty: item.counterparty,
            preview_payment_method: item.paymentMethod,
            preview_parser_id: 'wechat',
            preview_parser_tags: ['wallet'],
            matching: item.matching,
            ...overrides
        });
        const fetchSpy = jest.spyOn(global, 'fetch');
        try {
            bindings.syncTransferDecisionBaseline(transaction);
            bindings.syncLearningDecisionBaseline(transaction);

            await emptySessionBindings.reviewTransferSuggestion(transaction, 'accept');
            await emptySessionBindings.reviewLearningSuggestion(transaction, 'accept');
            await emptySessionBindings.reviewLLMRecommendation(transaction, 'accept');
            expect(await emptySessionBindings.fetchMatchingSessionCandidate('missing')).toBeNull();
            expect(await emptySessionBindings.syncLearningDecisionDraftToPreview(transaction, 'missing')).toBe(false);

            bindings.editingTransaction.value = transaction;
            await bindings.reviewTransferSuggestion(transaction, 'accept');
            await bindings.reviewLearningSuggestion(transaction, 'accept');
            await bindings.reviewLLMRecommendation(transaction, 'accept');
            bindings.editingTransaction.value = null;

            bindings.addDecisionLoadingId(bindings.learningDecisionLoadingIds, transaction._previewId);
            await bindings.reviewTransferSuggestion(transaction, 'accept');
            await bindings.reviewLearningSuggestion(transaction, 'accept');
            await bindings.reviewLLMRecommendation(transaction, 'accept');
            bindings.removeDecisionLoadingId(bindings.learningDecisionLoadingIds, transaction._previewId);

            bindings.getPreviewState(transaction)._shouldClearTransferDecision = true;
            await bindings.reviewTransferSuggestion(transaction, 'accept');
            bindings.getPreviewState(transaction)._shouldClearTransferDecision = false;

            mockCurrentToken = 'coverage-token';
            fetchSpy
                .mockResolvedValueOnce({
                    ok: true,
                    json: async () => ({
                        success: true,
                        data: {
                            sessionId: 'session-desktop-decisions',
                            previewItem: previewFor(transaction)
                        }
                    })
                } as Response)
                .mockResolvedValueOnce({
                    ok: true,
                    json: async () => ({ success: false, error: 'business reject' })
                } as Response)
                .mockResolvedValueOnce({
                    ok: true,
                    json: async () => ({ success: false })
                } as Response)
                .mockResolvedValueOnce({
                    ok: true,
                    json: async () => ({
                        success: true,
                        data: { sessionId: 'stale-session', previewItem: previewFor(transaction) }
                    })
                } as Response)
                .mockResolvedValueOnce({
                    ok: true,
                    json: async () => ({
                        success: true,
                        data: { sessionId: 'session-desktop-decisions' }
                    })
                } as Response)
                .mockResolvedValueOnce({
                    ok: true,
                    json: async () => ({ success: true })
                } as Response);
            await bindings.reviewTransferSuggestion(transaction, 'accept');
            await bindings.reviewTransferSuggestion(transaction, 'reject');
            await bindings.reviewTransferSuggestion(transaction, 'clear');
            await bindings.reviewTransferSuggestion(transaction, 'accept');
            await bindings.reviewTransferSuggestion(transaction, 'reject');
            await bindings.reviewTransferSuggestion(transaction, 'clear');

            const blockedLearning = createRenderableDesktopTransaction(3, 38);
            bindings.syncLearningDecisionBaseline(blockedLearning);
            blockedLearning.categoryId = 'changed-category';
            await bindings.reviewLearningSuggestion(blockedLearning, 'accept');

            const ruleCandidate = createRenderableDesktopTransaction(3, 31);
            ruleCandidate.matching.learning.source = 'rule';
            ruleCandidate.matching.learning.rule_id = null;
            bindings.syncLearningDecisionBaseline(ruleCandidate);
            await bindings.reviewLearningSuggestion(ruleCandidate, 'accept');

            ruleCandidate.matching.learning.rule_id = 42;
            mockAcceptMatchingCandidate.mockResolvedValueOnce({
                data: {
                    result: {
                        sessionId: 'session-desktop-decisions',
                        previewItem: previewFor(ruleCandidate, {
                            matching: {
                                ...ruleCandidate.matching,
                                learning: { ...ruleCandidate.matching.learning, review_status: 'accepted' }
                            }
                        })
                    }
                }
            });
            await bindings.reviewLearningSuggestion(ruleCandidate, 'accept');

            mockRejectMatchingCandidate.mockResolvedValueOnce({
                data: {
                    result: {
                        sessionId: 'session-desktop-decisions',
                        preview: [previewFor(ruleCandidate, {
                            matching: {
                                ...ruleCandidate.matching,
                                learning: { ...ruleCandidate.matching.learning, review_status: 'rejected' }
                            }
                        })]
                    }
                }
            });
            await bindings.reviewLearningSuggestion(ruleCandidate, 'reject');

            mockClearMatchingCandidate.mockResolvedValueOnce({
                data: {
                    result: {
                        sessionId: 'session-desktop-decisions',
                        previewItem: previewFor(ruleCandidate, {
                            matching: {
                                ...ruleCandidate.matching,
                                learning: { ...ruleCandidate.matching.learning, review_status: '' }
                            }
                        })
                    }
                }
            });
            await bindings.reviewLearningSuggestion(ruleCandidate, 'clear');

            const textDriftModelCandidate = createRenderableDesktopTransaction(3, 39);
            textDriftModelCandidate.matching.learning.model_version = '';
            textDriftModelCandidate.getLearningRecommendationInputFingerprint = () => textDriftModelCandidate.comment;
            bindings.syncLearningDecisionBaseline(textDriftModelCandidate);
            textDriftModelCandidate.comment = 'updated learning text';
            mockUpdateImportPreviewItem.mockResolvedValueOnce({
                data: {
                    result: {
                        updated: true,
                        previewItem: previewFor(textDriftModelCandidate)
                    }
                }
            });
            mockAcceptMatchingCandidate.mockResolvedValueOnce({
                data: {
                    result: {
                        sessionId: 'session-desktop-decisions',
                        previewItem: previewFor(textDriftModelCandidate)
                    }
                }
            });
            await bindings.reviewLearningSuggestion(textDriftModelCandidate, 'accept');

            const failedTextSyncCandidate = createRenderableDesktopTransaction(3, 41);
            failedTextSyncCandidate.getLearningRecommendationInputFingerprint = () => failedTextSyncCandidate.comment;
            bindings.syncLearningDecisionBaseline(failedTextSyncCandidate);
            failedTextSyncCandidate.comment = 'text sync must fail';
            mockUpdateImportPreviewItem.mockResolvedValueOnce({ data: { result: { updated: false } } });
            await bindings.reviewLearningSuggestion(failedTextSyncCandidate, 'accept');

            const staleTextSyncCandidate = createRenderableDesktopTransaction(3, 43);
            staleTextSyncCandidate._rowVersion = 2;
            staleTextSyncCandidate.getLearningRecommendationInputFingerprint = () => staleTextSyncCandidate.comment;
            bindings.syncLearningDecisionBaseline(staleTextSyncCandidate);
            staleTextSyncCandidate.comment = 'stale local text';
            mockUpdateImportPreviewItem.mockRejectedValueOnce(new Error('preview row conflict'));
            mockGetImportPreviewRowVersionConflict.mockReturnValueOnce({
                expected_row_version: 2,
                actual_row_version: 4,
                previewItem: previewFor(staleTextSyncCandidate, {
                    row_version: 4,
                    preview_description: 'latest server text'
                })
            });
            await bindings.reviewLearningSuggestion(staleTextSyncCandidate, 'accept');
            expect(staleTextSyncCandidate._rowVersion).toBe(4);
            expect(staleTextSyncCandidate.comment).toBe('latest server text');

            const missingResultCandidate = createRenderableDesktopTransaction(3, 42);
            mockAcceptMatchingCandidate.mockResolvedValueOnce({ data: { result: undefined } });
            mockGetMatchingSessionCandidates.mockResolvedValueOnce({ data: { result: { candidates: [] } } });
            await bindings.reviewLearningSuggestion(missingResultCandidate, 'accept');

            mockAcceptMatchingCandidate.mockResolvedValueOnce({
                data: { result: { sessionId: 'session-desktop-decisions' } }
            });
            mockGetMatchingSessionCandidates.mockResolvedValueOnce({
                data: {
                    result: {
                        candidates: [{
                            candidate_id: `preview:${ruleCandidate._previewId}:learning`,
                            details: { review_status: 'pending' }
                        }]
                    }
                }
            });
            await bindings.reviewLearningSuggestion(ruleCandidate, 'accept');

            mockAcceptMatchingCandidate.mockResolvedValueOnce({
                data: { result: { sessionId: 'stale-session', previewItem: previewFor(ruleCandidate) } }
            });
            mockGetMatchingSessionCandidates.mockResolvedValueOnce({
                data: {
                    result: {
                        candidates: [{
                            candidate_id: `preview:${ruleCandidate._previewId}:learning`,
                            details: { score: 0.7, review_status: 'pending' }
                        }]
                    }
                }
            });
            await bindings.reviewLearningSuggestion(ruleCandidate, 'accept');

            const llmTransaction = createRenderableDesktopTransaction(3, 32);
            mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
            mockLlmPreviewRecommendAccept.mockResolvedValueOnce({
                data: {
                    result: {
                        session_id: 'session-desktop-decisions',
                        preview: previewFor(llmTransaction),
                        matching: { llm: { review_status: 'accepted', reason: 'accepted evidence' } }
                    }
                }
            });
            await bindings.reviewLLMRecommendation(llmTransaction, 'accept');
            mockLlmPreviewRecommendReject.mockResolvedValueOnce({
                data: {
                    result: {
                        sessionId: 'session-desktop-decisions',
                        preview: previewFor(llmTransaction),
                        matching: { llm: { review_status: 'rejected', reason: 'rejected evidence' } }
                    }
                }
            });
            await bindings.reviewLLMRecommendation(llmTransaction, 'reject');
            mockLlmPreviewRecommendAccept.mockResolvedValueOnce({
                data: { result: { session_id: 'stale-session' } }
            });
            await bindings.reviewLLMRecommendation(llmTransaction, 'accept');
            mockLlmPreviewRecommendAccept.mockResolvedValueOnce({ data: { result: undefined } });
            await bindings.reviewLLMRecommendation(llmTransaction, 'accept');

            const syncTransaction = createRenderableDesktopTransaction(3, 33);
            const invalidPreviewIdTransaction = createRenderableDesktopTransaction(3, 40);
            invalidPreviewIdTransaction._previewId = Number.POSITIVE_INFINITY;
            expect(await bindings.syncLearningDecisionDraftToPreview(invalidPreviewIdTransaction, 'invalid-id')).toBe(false);
            mockUpdateImportPreviewItem.mockResolvedValueOnce({ data: { result: { updated: false } } });
            expect(await bindings.syncLearningDecisionDraftToPreview(syncTransaction, 'candidate-33')).toBe(false);

            mockUpdateImportPreviewItem.mockResolvedValueOnce({
                data: { result: { updated: true, previewItem: { id: 999 } } }
            });
            mockGetMatchingSessionCandidates.mockReset();
            mockGetMatchingSessionCandidates.mockResolvedValueOnce({
                data: {
                    result: {
                        candidates: [{
                            candidate_id: 'candidate-33',
                            details: { score: 0.6, review_status: 'pending' }
                        }]
                    }
                }
            });
            expect(await bindings.syncLearningDecisionDraftToPreview(syncTransaction, 'candidate-33')).toBe(true);

            mockUpdateImportPreviewItem.mockResolvedValueOnce({
                data: { result: { updated: true, previewItem: null } }
            });
            mockGetMatchingSessionCandidates.mockReset();
            mockGetMatchingSessionCandidates.mockResolvedValueOnce({ data: { result: { candidates: null } } });
            expect(await bindings.syncLearningDecisionDraftToPreview(syncTransaction, 'candidate-missing')).toBe(false);

            mockUpdateImportPreviewItem.mockRejectedValueOnce(new Error('sync unavailable'));
            expect(await bindings.syncLearningDecisionDraftToPreview(syncTransaction, 'candidate-error')).toBe(false);
        } finally {
            mockCurrentToken = '';
            fetchSpy.mockRestore();
            warnSpy.mockRestore();
        }
    });

    test('mobile state matrix covers signal projection, selection, confirmation, and successful decisions', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const back = jest.fn();
        mockConfirmImportPreview.mockResolvedValueOnce({ data: { result: {} } });
        mockReviewImportTransferDecision.mockResolvedValueOnce({ data: { result: {} } });
        mockAcceptMatchingCandidate.mockResolvedValueOnce({ data: { result: {} } });
        mockRejectMatchingCandidate.mockResolvedValueOnce({ data: { result: {} } });
        mockClearMatchingCandidate.mockResolvedValueOnce({ data: { result: {} } });
        mockLlmPreviewRecommendAccept.mockResolvedValueOnce({ data: { result: {} } });
        mockLlmPreviewRecommendReject.mockResolvedValueOnce({ data: { result: {} } });
        mockGetImportPreviewPage.mockResolvedValue({ data: { result: { preview: [] } } });
        try {
            const bindings = (ImportPreviewPage as any).setup(
                {
                    f7route: { query: { sessionId: 'session-mobile' } },
                    f7router: { back }
                },
                { expose: jest.fn() }
            );
            const record: any = {
                id: 91,
                preview_selected: true,
                preview_counterparty: 'Coffee Shop',
                preview_payment_method: 'Card',
                preview_description: 'Latte',
                preview_date: '2026-07-10',
                preview_main_category: 'Food',
                preview_sub_category: 'Cafe',
                preview_amount_cents: -1234,
                preview_parser_id: 'wechat',
                preview_parser_tags: ['wallet'],
                dedup_type: 'platform_duplicate',
                dedup_source_ids: ['source-1'],
                transfer_suggestion_reason: 'paired transfer',
                learning_recommendation_reason: 'merchant rule',
                learning_recommendation_summary: 'Food/Cafe',
                preview_state: previewStateSnapshot([
                    'platform_duplicate',
                    'transfer',
                    'history',
                    'learning',
                    'llm'
                ]),
                matching: {
                    parser: { id: 'wechat', tags: ['wallet'] },
                    dedup: { type: 'platform_duplicate', source_ids: ['source-1'] },
                    reconciliation: {
                        planned_operation: 'update_history',
                        history_bill_id: 7,
                        history_bill_version: 3,
                        operation_id: 'operation-91',
                        acknowledgement_token: 'ack-91',
                        destructive_ack_required: true,
                        notice: 'history rewrite'
                    },
                    transfer: { review_status: 'pending', reason: 'paired transfer' },
                    learning: { review_status: 'pending', reason: 'merchant rule' },
                    llm: { review_status: 'pending', reason: 'LLM evidence', confidence: 0.92 }
                }
            };
            const [row] = bindings.normalizeRows([record]);
            bindings.rows.value = [row];

            expect(bindings.normalizeSignalStatus).toBeUndefined();
            expect(bindings.rowTitle(row)).toBe('Coffee Shop');
            expect(bindings.rowSubtitle(row)).toContain('Food');
            expect(bindings.formatAmount(row)).toBe('-12.34');
            expect(bindings.signalChips(row).map((chip: any) => chip.label)).toContain('History Rewrite');
            expect(bindings.detailLines(row).length).toBeGreaterThan(0);
            expect(bindings.historyOperation(row)).toMatchObject({
                preview_id: 91,
                planned_operation: 'update_history',
                history_bill_id: 7
            });
            expect(bindings.buildPreviewUpdates()).toEqual([{ id: 91, selected: true }]);
            expect(bindings.buildHistoryAcknowledgement()).toMatchObject({
                selection_scope: { mode: 'mobile-visible-preview', selected_visible_count: 1 }
            });

            bindings.toggleRowSelection(row, { target: { checked: false } } as unknown as Event);
            expect(row.selected).toBe(false);
            bindings.selectAllVisible();
            expect(row.selected).toBe(true);
            bindings.selectNoneVisible();
            expect(row.selected).toBe(false);
            bindings.openRowSheet(row);
            expect(bindings.detailRow.value).toMatchObject({ id: 91 });

            row.selected = true;
            await bindings.confirmSelectedNow();
            await bindings.reviewTransfer(row, 'accept');
            await bindings.reviewLearning(row, 'accept');
            await bindings.reviewLearning(row, 'reject');
            await bindings.reviewLearning(row, 'clear');
            await bindings.reviewLlm(row, 'accept');
            await bindings.reviewLlm(row, 'reject');
            await bindings.reviewLlm(row, 'clear');

            expect(mockConfirmImportPreview).toHaveBeenCalledWith(expect.objectContaining({
                sessionId: 'session-mobile',
                preserveUnpatchedSelection: true
            }));
            expect(mockReviewImportTransferDecision).toHaveBeenCalled();
            expect(mockAcceptMatchingCandidate).toHaveBeenCalled();
            expect(mockRejectMatchingCandidate).toHaveBeenCalled();
            expect(mockClearMatchingCandidate).toHaveBeenCalled();
            expect(mockLlmPreviewRecommendAccept).toHaveBeenCalled();
            expect(mockLlmPreviewRecommendReject).toHaveBeenCalled();
            expect(back).toHaveBeenCalled();
            expect(row.busy).toBe(false);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('mobile likely transfer belongs to transfer and read-only learning filters', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const bindings = (ImportPreviewPage as any).setup(
                {
                    f7route: { query: { sessionId: 'session-mobile-overlap' } },
                    f7router: { back: jest.fn() }
                },
                { expose: jest.fn() }
            );
            const [row] = bindings.normalizeRows([{
                id: 92,
                preview_selected: true,
                preview_parser_id: 'wechat',
                transfer_suggestion_level: 'green',
                transfer_suggestion_reason: 'paired account movement',
                learning_recommendation_reason: 'transfer preview is protected from learning type/category overrides',
                preview_state: previewStateSnapshot(
                    ['transfer', 'learning'],
                    { transfer: 'pending', learning: 'skipped' }
                ),
                matching: {
                    parser: { id: 'wechat', tags: ['wallet'] },
                    transfer: {
                        candidate_type: 'transfer',
                        review_status: 'pending',
                        learning_level: 'green',
                        reason: 'paired account movement'
                    },
                    learning: {
                        review_status: 'skipped',
                        reason: 'transfer preview is protected from learning type/category overrides'
                    }
                }
            }]);
            bindings.rows.value = [row];

            expect(row.signal.transferSuggestion?.status).toBe('pending');
            expect(row.signal.learning).toMatchObject({ status: 'pending', actions: [] });
            bindings.signalFilter.value = 'transfer';
            expect(bindings.filteredRows.value).toEqual([row]);
            bindings.signalFilter.value = 'learning';
            expect(bindings.filteredRows.value).toEqual([row]);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('mobile fallback, confirmation, and failure branches always clear busy state', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const back = jest.fn();
        mockShowConfirm.mockReset();
        mockShowToast.mockClear();
        mockGetImportPreviewPage.mockResolvedValue({ data: { result: { preview: [] } } });
        const bindings = (ImportPreviewPage as any).setup(
            {
                f7route: { query: { session_id: 'session-mobile-fallback' } },
                f7router: { back }
            },
            { expose: jest.fn() }
        );
        try {
            const record: any = {
                id: 101,
                selected: true,
                preview_payment_method: 'Cash',
                preview_amount_cents: 'not-a-number',
                preview_state: previewStateSnapshot(
                    ['transfer', 'learning', 'llm'],
                    { transfer: 'accepted', learning: 'accepted', llm: 'accepted' }
                ),
                matching: {
                    parser: { id: 'alipay', tags: ['mobile'], source_chain: [] },
                    dedup: { type: 'similar_duplicate', source_ids: ['source-101'], source_count: 1 },
                    reconciliation: { destructive_ack_required: false },
                    transfer: { review_status: 'accepted', reason: '' },
                    learning: {
                        review_status: 'accepted',
                        summary: '',
                        mode: '',
                        signal_state: '',
                        auto_apply: false
                    },
                    llm: {
                        review_status: 'accepted',
                        suggested_sub_category: 'Cafe',
                        suggested_destination_account: 'Wallet'
                    },
                    recurring: { candidate_count: 1, name: 'Monthly', match_reasons: 'date' }
                }
            };
            const normalized = bindings.normalizeRows([
                { ...record, id: 0 },
                { ...record, id: Number.NaN },
                record
            ]);
            expect(normalized).toHaveLength(1);
            const [row] = normalized;
            bindings.rows.value = [row];

            expect(bindings.normalizeSignalStatus).toBeUndefined();
            expect(bindings.rowTitle(row)).toBe('Cash');
            expect(bindings.rowTitle({ record: {} })).toBe('Imported Transaction');
            expect(bindings.rowSubtitle({ record: {} })).toBe('');
            expect(bindings.formatAmount(row)).toBe('0.00');

            row.signal.learning = { labelKey: 'Learning Suggestion', color: 'success', detailLines: [] };
            expect(bindings.signalChips(row)).toEqual(expect.arrayContaining([
                expect.objectContaining({ label: 'Learning Suggestion', tone: 'success' })
            ]));
            expect(bindings.signalChips({ signal: {} })).toEqual([]);
            expect(bindings.detailLines({ signal: {} })).toEqual([]);
            bindings.toggleRowSelection(row, { target: null } as unknown as Event);
            expect(row.selected).toBe(false);

            bindings.confirmSelected();
            expect(mockShowConfirm).not.toHaveBeenCalled();
            bindings.selectAllVisible();
            bindings.confirming.value = true;
            bindings.confirmSelected();
            expect(mockShowConfirm).not.toHaveBeenCalled();
            bindings.confirming.value = false;
            bindings.selectAllVisible();
            mockConfirmImportPreview.mockResolvedValueOnce({ data: { result: {} } });
            mockShowConfirm.mockImplementationOnce((...args: unknown[]) => (args[1] as () => void)());
            bindings.confirmSelected();
            await Promise.resolve();
            expect(mockShowConfirm).toHaveBeenCalledWith('format.misc.confirmImportTransactions', expect.any(Function));

            const historyRecord = {
                ...record,
                id: 102,
                matching: {
                    ...record.matching,
                    reconciliation: {
                        planned_operation: 'update_history',
                        history_bill_id: 7,
                        history_bill_version: 3,
                        operation_id: 'history-102',
                        acknowledgement_token: 'ack-102',
                        destructive_ack_required: true
                    }
                }
            };
            const [historyRow] = bindings.normalizeRows([historyRecord]);
            bindings.rows.value = [historyRow];
            bindings.selectAllVisible();
            mockShowConfirm.mockImplementationOnce(() => undefined);
            bindings.confirmSelected();
            expect(mockShowConfirm).toHaveBeenLastCalledWith('History Rewrite', expect.any(Function));

            mockConfirmImportPreview.mockRejectedValueOnce(new Error('confirm failed'));
            await bindings.confirmSelectedNow();
            mockConfirmImportPreview.mockRejectedValueOnce({ code: 'failed' });
            await bindings.confirmSelectedNow();
            expect(bindings.confirming.value).toBe(false);

            mockReviewImportTransferDecision.mockRejectedValueOnce(new Error('transfer failed'));
            await expect(bindings.reviewTransfer(historyRow, 'reject')).rejects.toThrow('transfer failed');
            expect(historyRow.busy).toBe(false);

            mockAcceptMatchingCandidate.mockRejectedValueOnce(new Error('learning failed'));
            await bindings.reviewLearning(historyRow, 'accept');
            mockRejectMatchingCandidate.mockRejectedValueOnce({ code: 'learning failed' });
            await bindings.reviewLearning(historyRow, 'reject');
            expect(historyRow.busy).toBe(false);

            mockLlmPreviewRecommendAccept.mockRejectedValueOnce(new Error('llm failed'));
            await bindings.reviewLlm(historyRow, 'accept');
            mockLlmPreviewRecommendReject.mockRejectedValueOnce({ code: 'llm failed' });
            await bindings.reviewLlm(historyRow, 'reject');
            expect(historyRow.busy).toBe(false);

            const done = jest.fn();
            mockGetImportPreviewPage.mockRejectedValueOnce(new Error('reload failed'));
            await bindings.reload(done);
            mockGetImportPreviewPage.mockRejectedValueOnce({ code: 'reload failed' });
            await bindings.reload(done);
            expect(done).toHaveBeenCalledTimes(2);
            expect(bindings.loading.value).toBe(false);

            const missingSessionBack = jest.fn();
            const missingSessionBindings = (ImportPreviewPage as any).setup(
                { f7route: { query: {} }, f7router: { back: missingSessionBack } },
                { expose: jest.fn() }
            );
            const missingDone = jest.fn();
            await missingSessionBindings.reload(missingDone);
            expect(missingSessionBack).toHaveBeenCalled();
            expect(missingDone).toHaveBeenCalled();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('imports and SSR renders the mobile import lifecycle page', async () => {
        const back = jest.fn();
        const app = createSSRApp(ImportPreviewPage, {
            f7route: { query: { sessionId: 'session-mobile' } },
            f7router: { back }
        });
        registerUiStubs(app);

        const html = await renderToString(app);

        expect(html).toContain('data-testid="mobile.import.preview.page"');
        expect(html).toContain('Select All');
        expect(back).not.toHaveBeenCalled();
    });
});
