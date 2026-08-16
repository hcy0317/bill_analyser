import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockFetch = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockShowMessage = jest.fn();
const mockGetLLMMemoryEvents = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockLlmPreviewRecommend = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAnalyzeLLMTransactions = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockGetImportLearningSuggestions = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockPromoteImportLearning = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockReclassifyImportPreview = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockGetImportPreviewRowVersionConflict = jest.fn<(...args: Array<any>) => any>();
const mockLoggerError = jest.fn();

const mockExpenseChild = {
    id: '8',
    parentId: '7',
    name: 'Cafe',
    type: 3,
    icon: 'food',
    color: '#ffaa00',
    hidden: false,
    subCategories: []
};
const mockExpenseParent = {
    id: '7',
    parentId: '0',
    name: 'Food',
    type: 3,
    icon: 'food',
    color: '#ffaa00',
    hidden: false,
    subCategories: [mockExpenseChild]
};
const mockWallet = {
    id: 'wallet',
    name: 'Wallet',
    currency: 'CNY',
    category: 1,
    icon: 'wallet',
    color: '#0088ff',
    hidden: false
};
const mockBank = {
    ...mockWallet,
    id: 'bank',
    name: 'Bank'
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(null)
    };
});
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, params?: Record<string, unknown>) => params ? `${key}:${JSON.stringify(params)}` : key,
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
        currentUserCoordinateDisplayType: 0,
        currentUserCashTransferCategoryId: ''
    })
}));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => ({
        allPlainAccounts: [mockWallet, mockBank],
        allVisiblePlainAccounts: [mockWallet, mockBank],
        allAccountsMap: { wallet: mockWallet, bank: mockBank },
        allAccounts: [mockWallet, mockBank],
        loadAllAccounts: jest.fn()
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: { 3: [mockExpenseParent] },
        allTransactionCategoriesMap: { '7': mockExpenseParent, '8': mockExpenseChild },
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
        llmPreviewRecommend: mockLlmPreviewRecommend,
        analyzeLLMTransactions: mockAnalyzeLLMTransactions,
        getImportLearningSuggestions: mockGetImportLearningSuggestions,
        promoteImportLearning: mockPromoteImportLearning,
        reclassifyImportPreview: mockReclassifyImportPreview,
        getImportPreviewRowVersionConflict: mockGetImportPreviewRowVersionConflict
    }
}));
jest.mock('@/lib/server_settings.ts', () => ({ isTransactionFromAIImageRecognitionEnabled: () => false }));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => 'signal-matrix-token' }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: mockLoggerError }
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
    '@/views/desktop/accounts/list/dialogs/EditDialog.vue',
    '@/views/desktop/transactions/list/dialogs/EditDialog.vue'
]) {
    jest.mock(componentPath, () => ({ __esModule: true, default: { name: 'SignalHistoryMatrixStub' } }));
}

import { ImportTransaction } from '@/models/imported_transaction.ts';
import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';
import { previewStateSnapshot } from '../../../../helpers/importPreviewState.ts';

type MatchingOverrides = Record<string, any>;

function createTransaction(
    id: number,
    options: { selected?: boolean, matching?: MatchingOverrides } = {}
): ImportTransaction {
    const matching: MatchingOverrides = {
        parser: {
            id: 'alipay',
            tags: ['wallet'],
            source_chain: [{ role: 'current', parser_id: 'alipay', label: 'Alipay row' }]
        },
        dedup: {
            type: '',
            source_ids: [],
            source_count: 0,
            source_labels: [],
            sources: []
        },
        transfer: { review_status: '', reviewed_type: '', suppressed: false },
        learning: {},
        llm: {},
        reconciliation: {},
        annotation: {},
        ...options.matching
    };
    const transaction = ImportTransaction.of({
        type: 3,
        categoryId: '8',
        originalCategoryName: 'Cafe',
        time: 1_788_480_000 + id,
        utcOffset: 480,
        sourceAccountId: 'wallet',
        originalSourceAccountName: 'Wallet',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: '',
        originalDestinationAccountName: '',
        originalDestinationAccountCurrency: 'CNY',
        sourceAmountCents: -1_000 - id,
        destinationAmountCents: 0,
        tagIds: [],
        originalTagNames: [],
        comment: `row-${id}`,
        counterparty: `merchant-${id}`,
        paymentMethod: 'card',
        selected: options.selected ?? true,
        matching
    } as never, id);
    const signals: Array<'parser' | 'platform_duplicate' | 'transfer' | 'history' | 'learning' | 'llm'> = [];
    const statuses: Record<string, 'pending' | 'accepted' | 'rejected' | 'skipped'> = {};
    const meaningfulFamily = (family: 'learning' | 'llm'): boolean => {
        const section = matching[family] as Record<string, unknown>;
        if (section['review_status'] === null || section['suppressed'] === true) {
            return false;
        }
        return Object.entries(section).some(([key, value]) => (
            key !== 'review_status' && key !== 'suppressed' && value !== '' && value !== null && value !== 0
        )) || ['accepted', 'rejected', 'skipped'].includes(String(section['review_status'] || ''));
    };
    if (matching['dedup'].type === 'platform_bank') {
        signals.push('platform_duplicate');
    }
    if (['pending', 'accepted'].includes(String(matching['transfer'].review_status || ''))) {
        signals.push('transfer');
        statuses['transfer'] = matching['transfer'].review_status as 'pending' | 'accepted';
    }
    if (matching['reconciliation'].planned_operation) {
        signals.push('history');
        statuses['history'] = 'pending';
    }
    for (const family of ['learning', 'llm'] as const) {
        if (meaningfulFamily(family)) {
            signals.push(family);
            const status = String(matching[family].review_status || 'pending');
            statuses[family] = ['accepted', 'rejected', 'skipped'].includes(status)
                ? status as 'accepted' | 'rejected' | 'skipped'
                : 'pending';
        }
    }
    if (signals.length === 0) {
        signals.push('parser');
    }
    transaction.previewState = previewStateSnapshot(signals, statuses);
    (transaction as ImportTransaction & { _previewId: number })._previewId = id;
    return transaction;
}

function createBindings(
    transactions: ImportTransaction[],
    sessionId = 'signal-session'
): { bindings: any, emit: jest.Mock } {
    const emit = jest.fn();
    const bindings = (ImportTransactionCheckDataTab as any).setup({
        importTransactions: transactions,
        sessionId,
        serverPaged: false,
        totalImportTransactionCount: transactions.length,
        previewMetadata: null
    }, { emit, expose: jest.fn() });
    bindings.snackbar.value = { showMessage: mockShowMessage };
    return { bindings, emit };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
    mockGetImportPreviewRowVersionConflict.mockReturnValue(null);
    Object.defineProperty(globalThis, 'fetch', { configurable: true, writable: true, value: mockFetch });
});

describe('desktop import signal, history, and annotation matrix', () => {
    test('builds each visible signal from payload evidence and respects authoritative absence', () => {
        const parser = createTransaction(1);
        const { bindings } = createBindings([parser]);

        expect(bindings.getImportPreviewSignalViewModel(parser)).toMatchObject({
            parser: { parserId: 'alipay', label: '支付宝' },
            dedup: null,
            transferSuggestion: null,
            historyRewrite: null,
            learning: null,
            llm: null
        });

        const platformDuplicate = createTransaction(2, {
            matching: {
                dedup: {
                    type: 'platform_bank',
                    source_ids: ['bank-1'],
                    source_count: 1,
                    source_labels: ['Bank row'],
                    sources: [{ role: 'duplicate', parser_id: 'ccb', label: 'Bank row' }]
                }
            }
        });
        const transfer = createTransaction(3, {
            matching: {
                transfer: {
                    candidate_type: 'cash_transfer',
                    review_status: 'pending',
                    reviewed_type: '',
                    suppressed: false,
                    reason: 'paired transfer'
                }
            }
        });
        const history = createTransaction(4, {
            matching: {
                reconciliation: {
                    candidate_type: 'history',
                    signal_label: 'History candidate',
                    planned_operation: 'update_history',
                    history_bill_id: 91,
                    history_bill_version: 2,
                    operation_id: 'operation-4',
                    acknowledgement_token: 'ack-4',
                    destructive_ack_required: true,
                    notice: 'history rewrite warning'
                }
            }
        });
        const learning = createTransaction(5, {
            matching: {
                learning: {
                    review_status: 'pending',
                    rule_id: 'rule-5',
                    score: 0.91,
                    confidence: 0.88,
                    margin: 0.12,
                    accepted_count: 3,
                    rejected_count: 1,
                    auto_applied_count: 2,
                    reason: 'merchant rule',
                    summary: 'Food/Cafe'
                }
            }
        });
        const llm = createTransaction(6, {
            matching: {
                llm: {
                    review_status: 'pending',
                    suggested_type: 'expense',
                    suggested_category_id: '8',
                    suggested_main_category: 'Food',
                    suggested_sub_category: 'Cafe',
                    suggested_source_account: 'Wallet',
                    reason: 'model evidence',
                    confidence: 0.83
                }
            }
        });

        expect(createBindings([platformDuplicate]).bindings.getImportPreviewSignalViewModel(platformDuplicate)).toMatchObject({
            parser: null,
            dedup: { dedupType: 'platform_bank', sourceCount: 1 }
        });
        expect(createBindings([transfer]).bindings.getImportPreviewSignalViewModel(transfer)).toMatchObject({
            parser: null,
            transferSuggestion: { status: 'pending', title: expect.stringContaining('paired transfer') }
        });
        expect(createBindings([history]).bindings.getImportPreviewSignalViewModel(history)).toMatchObject({
            parser: null,
            historyRewrite: { status: 'pending', labelKey: 'History Rewrite' }
        });
        expect(createBindings([learning]).bindings.getImportPreviewSignalViewModel(learning)).toMatchObject({
            parser: null,
            learning: {
                status: 'pending',
                title: expect.stringContaining('Food/Cafe'),
                detailLines: [expect.stringContaining('Food/Cafe')]
            }
        });
        expect(createBindings([llm]).bindings.getImportPreviewSignalViewModel(llm)).toMatchObject({
            parser: null,
            llm: {
                status: 'pending',
                title: expect.stringContaining('Food-Cafe'),
                detailLines: expect.arrayContaining([
                    expect.stringContaining('Food-Cafe'),
                    expect.stringContaining('Wallet'),
                    expect.stringContaining('83%')
                ])
            }
        });

        for (const family of ['learning', 'llm'] as const) {
            const authoritativeNull = createTransaction(20, {
                matching: { [family]: { review_status: null, reason: 'stale evidence', confidence: 0.9 } }
            });
            const absentStatus = createTransaction(21, {
                matching: { [family]: { reason: 'live evidence', confidence: 0.9 } }
            });
            const suppressed = createTransaction(22, {
                matching: { [family]: { review_status: 'pending', reason: 'hidden evidence', suppressed: true } }
            });
            const accepted = createTransaction(23, {
                matching: { [family]: { review_status: 'accepted' } }
            });
            authoritativeNull.matching![family]!.review_status = null as any;
            const familyBindings = createBindings([authoritativeNull, absentStatus, suppressed, accepted]).bindings;

            expect(familyBindings.getImportPreviewSignalViewModel(authoritativeNull)[family]).toBeNull();
            expect(familyBindings.getImportPreviewSignalViewModel(absentStatus)[family]?.status).toBe('pending');
            expect(familyBindings.getImportPreviewSignalViewModel(suppressed)[family]).toBeNull();
            expect(familyBindings.getImportPreviewSignalViewModel(accepted)[family]?.status).toBe('accepted');
        }
    });

    test('invalidates the view-model cache for learning, LLM, history, parser, and dedup business fields', () => {
        const transaction = createTransaction(30, {
            matching: {
                learning: { review_status: 'pending', rule_id: 'rule-a', score: 0.6, reason: 'first rule' },
                llm: {
                    review_status: 'pending',
                    suggested_category_id: '8',
                    suggested_main_category: 'Food',
                    suggested_sub_category: 'Cafe',
                    reason: 'first model',
                    confidence: 0.7
                },
                reconciliation: {
                    planned_operation: 'update_history',
                    history_bill_id: 91,
                    history_bill_version: 1,
                    history_role: 'source',
                    group_key: 'group-a',
                    operation_id: 'operation-a',
                    acknowledgement_token: 'ack-a',
                    destructive_ack_required: true
                }
            }
        });
        const { bindings } = createBindings([transaction]);
        const first = bindings.getImportPreviewSignalViewModel(transaction);
        expect(bindings.getImportPreviewSignalViewModel(transaction)).toBe(first);

        transaction.matching!.learning!.rule_id = 12;
        transaction.matching!.learning!.score = 0.92;
        transaction.matching!.learning!.confidence = 0.9;
        transaction.matching!.learning!.margin = 0.18;
        const afterLearning = bindings.getImportPreviewSignalViewModel(transaction);
        expect(afterLearning).not.toBe(first);
        expect(bindings.getImportPreviewSignalViewModel(transaction)).toBe(afterLearning);

        transaction.matching!.llm!.suggested_sub_category = 'Dinner';
        transaction.matching!.llm!.suggested_source_account = 'Cash';
        transaction.matching!.llm!.confidence = 0.94;
        const afterLlm = bindings.getImportPreviewSignalViewModel(transaction);
        expect(afterLlm).not.toBe(afterLearning);
        expect(afterLlm.llm.title).toContain('Food-Dinner');
        expect(afterLlm.llm.title).toContain('Cash');

        transaction.matching!.reconciliation!.group_key = 'group-b';
        transaction.matching!.reconciliation!.history_role = 'destination';
        transaction.matching!.reconciliation!.acknowledgement_token = 'ack-b';
        const afterHistory = bindings.getImportPreviewSignalViewModel(transaction);
        expect(afterHistory).not.toBe(afterLlm);

        transaction.parserId = 'wechat';
        transaction.parserTags = ['wallet', 'mobile'];
        transaction.dedupType = 'platform_bank';
        transaction.previewState!.signals.push('platform_duplicate');
        transaction.matching!.dedup!.source_count = 2;
        transaction.matching!.dedup!.source_labels = ['first', 'second'];
        const afterParserDedup = bindings.getImportPreviewSignalViewModel(transaction);
        expect(afterParserDedup).not.toBe(afterHistory);
        expect(afterParserDedup.dedup?.detailLines.join(' ')).toContain('second');
    });

    test('requires a complete history operation before exposing selected acknowledgement data', () => {
        const complete = createTransaction(41, {
            matching: {
                reconciliation: {
                    planned_operation: 'merge_transfer_history',
                    history_bill_id: 701,
                    history_bill_version: 4,
                    operation_id: 'operation-41',
                    acknowledgement_token: 'ack-41',
                    destructive_ack_required: true
                }
            }
        });
        const missingToken = createTransaction(42, {
            matching: {
                reconciliation: {
                    planned_operation: 'update_history',
                    history_bill_id: 702,
                    operation_id: 'operation-42',
                    destructive_ack_required: true
                }
            }
        });
        const incompleteIdentity = createTransaction(43, {
            matching: {
                reconciliation: {
                    planned_operation: 'update_history',
                    operation_id: 'operation-43',
                    acknowledgement_token: 'ack-43',
                    destructive_ack_required: true
                }
            }
        });
        const { bindings } = createBindings([complete, missingToken, incompleteIdentity]);

        expect(bindings.getImportPreviewHistoryRewriteOperation(complete)).toEqual({
            preview_id: 41,
            operation_id: 'operation-41',
            planned_operation: 'merge_transfer_history',
            history_bill_id: 701,
            history_bill_version: 4,
            acknowledgement_token: 'ack-41'
        });
        expect(bindings.getImportPreviewHistoryRewriteOperation(missingToken)).toBeNull();
        expect(bindings.getImportPreviewHistoryRewriteOperation(incompleteIdentity)).toBeNull();
        expect(bindings.getSelectedHistoryRewriteOperations()).toEqual([
            expect.objectContaining({ preview_id: 41, acknowledgement_token: 'ack-41' })
        ]);
        expect(bindings.getSelectedVisibleHistoryRewriteOperationCount()).toBe(1);
    });

    test('distinguishes raw persisted, current persisted, and baseline annotation issues', () => {
        const category = createTransaction(51, { matching: { annotation: { type: 'missing_category' } } });
        const source = createTransaction(52, { matching: { annotation: { status: 'missing_source_account' } } });
        const destination = createTransaction(53, { matching: { annotation: { reason: 'missing_destination_account' } } });
        const transfer = createTransaction(54, { matching: { annotation: { review_status: 'same_transfer_accounts' } } });
        const unknown = createTransaction(55, { matching: { annotation: 'manual_review' } });
        const empty = createTransaction(56, { matching: { annotation: { level: '   ' } } });
        category.matching!.annotation = { type: 'missing_category' } as any;
        source.matching!.annotation = { status: 'missing_source_account' } as any;
        destination.matching!.annotation = { reason: 'missing_destination_account' } as any;
        transfer.matching!.annotation = { review_status: 'same_transfer_accounts' } as any;
        unknown.matching!.annotation = 'manual_review' as any;
        empty.matching!.annotation = { level: '   ' } as any;
        category.categoryId = '';
        source.sourceAccountId = '';
        destination.type = 4;
        destination.destinationAccountId = '';
        transfer.type = 4;
        transfer.destinationAccountId = 'wallet';
        const { bindings } = createBindings([category, source, destination, transfer, unknown, empty]);

        for (const transaction of [category, source, destination, transfer, unknown]) {
            expect(bindings.hasCurrentAnnotationIssue(transaction)).toBe(true);
            expect(bindings.hasBaselineAnnotationIssue(transaction)).toBe(true);
            expect(bindings.getAnnotationSummary(transaction)).toEqual(expect.any(String));
        }
        expect(bindings.hasCurrentAnnotationIssue(empty)).toBe(false);

        category.categoryId = '8';
        source.sourceAccountId = 'wallet';
        destination.destinationAccountId = 'bank';
        transfer.type = 3;
        expect(bindings.hasCurrentAnnotationIssue(category)).toBe(false);
        expect(bindings.hasCurrentAnnotationIssue(source)).toBe(false);
        expect(bindings.hasCurrentAnnotationIssue(destination)).toBe(false);
        expect(bindings.hasCurrentAnnotationIssue(transfer)).toBe(false);
        expect(bindings.hasCurrentAnnotationIssue(unknown)).toBe(true);
    });

    test('canonical category patch clears stale missing-category signal while preserving missing-account review', () => {
        const transaction = createTransaction(57, {
            matching: {
                annotation: { status: 'missing_category' },
                identity_validation: {
                    review_status: 'requires_identity_review',
                    issues: [
                        { field: 'category_id', reason: 'missing' },
                        { field: 'source_account_id', reason: 'missing' }
                    ]
                }
            }
        });
        transaction.categoryId = '';
        transaction.sourceAccountId = '';
        const { bindings } = createBindings([transaction]);

        expect(bindings.getAnnotationIssues(transaction)).toStrictEqual([
            'Missing Category',
            'Missing Source Account'
        ]);

        bindings.syncTransactionFromPreviewDecision(transaction, {
            id: 57,
            preview_type: 'expense',
            category_id: 8,
            preview_source_account_id: null,
            preview_destination_account_id: null,
            matching: {
                annotation: { status: 'missing_category' },
                identity_validation: {
                    review_status: 'requires_identity_review',
                    issues: [
                        { field: 'category_id', reason: 'missing' },
                        { field: 'source_account_id', reason: 'missing' }
                    ]
                }
            }
        });

        expect(transaction.categoryId).toBe('8');
        expect(bindings.getAnnotationIssues(transaction)).toStrictEqual(['Missing Source Account']);
        expect(bindings.getAnnotationSummary(transaction)).toBe('Missing Source Account');
        expect(bindings.hasCurrentAnnotationIssue(transaction)).toBe(true);
        expect(transaction.matching?.identity_validation).toMatchObject({
            issues: [{ field: 'source_account_id', reason: 'missing' }]
        });
    });
});

describe('desktop import async action branch matrix', () => {
    test('reclassify handles missing session, updated rows, zero rows, and failures', async () => {
        const selected = createTransaction(61);
        (selected as ImportTransaction & { _rowVersion?: number })._rowVersion = 5;
        const missing = createBindings([selected], '');
        await missing.bindings.reclassifySelected();
        expect(mockReclassifyImportPreview).not.toHaveBeenCalled();
        expect(mockShowMessage).toHaveBeenCalledWith('No session ID available');

        mockReclassifyImportPreview.mockResolvedValueOnce({
            data: { result: { preview: [{ id: 61, row_version: 6 }], session_samples_saved: 1 } }
        });
        const success = createBindings([selected]);
        await success.bindings.reclassifySelected();
        expect(mockReclassifyImportPreview).toHaveBeenCalledWith({
            sessionId: 'signal-session',
            previewUpdates: [expect.objectContaining({
                id: 61,
                expected_row_version: 5
            })]
        });
        expect(success.emit).toHaveBeenCalledWith('reclassified', [{ id: 61, row_version: 6 }], [61]);
        expect(mockShowMessage).toHaveBeenCalledWith(
            'format.misc.youHaveUpdatedTransactions',
            { count: '1' }
        );

        mockReclassifyImportPreview.mockResolvedValueOnce({
            data: { result: { preview: [] } }
        });
        await createBindings([selected]).bindings.reclassifySelected();
        expect(mockShowMessage).toHaveBeenCalledWith('No transactions updated');

        const conflictError = new Error('stale preview');
        mockReclassifyImportPreview.mockRejectedValueOnce(conflictError);
        mockGetImportPreviewRowVersionConflict.mockReturnValueOnce({
            expected_row_version: 5,
            actual_row_version: 7,
            previewItem: {
                id: 61,
                row_version: 7,
                preview_type: 'expense',
                preview_description: 'server reclassify description',
                preview_counterparty: 'server counterparty',
                preview_payment_method: 'server payment',
                preview_selected: true,
                matching: {}
            }
        });
        await createBindings([selected]).bindings.reclassifySelected();
        expect((selected as ImportTransaction & { _rowVersion?: number })._rowVersion).toBe(7);
        expect(selected.comment).toBe('server reclassify description');
        expect(mockShowMessage).toHaveBeenCalledWith('stale preview');
    });

    test('LLM bulk recommendation guards empty state and uses all-matching when no rows are selected', async () => {
        const selected = createTransaction(71);
        const noSession = createBindings([selected], '').bindings;
        await noSession.applyLLMPreviewRecommendations();
        expect(mockLlmPreviewRecommend).not.toHaveBeenCalled();
        expect(mockShowMessage).toHaveBeenCalledWith('No session ID available');

        const busy = createBindings([selected]).bindings;
        busy.llmPreviewRecommending.value = true;
        await busy.applyLLMPreviewRecommendations();
        expect(mockLlmPreviewRecommend).not.toHaveBeenCalled();

        const empty = createBindings([]).bindings;
        await empty.applyLLMPreviewRecommendations();
        expect(mockLlmPreviewRecommend).not.toHaveBeenCalled();
        expect(mockShowMessage).toHaveBeenCalledWith('No preview rows available for LLM recommendation');

        mockLlmPreviewRecommend.mockResolvedValueOnce({ data: { result: { suggestions: [] } } });
        const unselected = createBindings([createTransaction(72, { selected: false })]).bindings;
        await unselected.applyLLMPreviewRecommendations();
        expect(mockLlmPreviewRecommend).toHaveBeenLastCalledWith(expect.objectContaining({
            sessionId: 'signal-session',
            previewUpdates: [],
            actionScope: expect.objectContaining({
                kind: 'all_matching',
                filter_hash: expect.stringMatching(/^fnv1a32:/)
            })
        }));
        expect(mockShowMessage).toHaveBeenCalledWith('LLM preview recommendation completed, but no suggestions were generated');
        expect(unselected.llmPreviewRecommending.value).toBe(false);

        mockLlmPreviewRecommend.mockResolvedValueOnce({
            data: { result: { suggestions: [{ preview_id: 71, matching: { llm: { review_status: 'pending', reason: 'fresh model' } } }] } }
        });
        const success = createBindings([selected]).bindings;
        await success.applyLLMPreviewRecommendations();
        expect(mockLlmPreviewRecommend).toHaveBeenCalledWith(expect.objectContaining({
            sessionId: 'signal-session',
            previewUpdates: [expect.objectContaining({ id: 71 })],
            actionScope: expect.objectContaining({
                kind: 'selected',
                selection_hash: expect.stringMatching(/^fnv1a32:/)
            })
        }));
        expect(selected.matching?.llm?.reason).toBe('fresh model');
        expect(mockGetLLMMemoryEvents).toHaveBeenCalledWith({ session_id: 'signal-session', limit: 500 });
        expect(mockShowMessage).toHaveBeenCalledWith(expect.stringContaining('LLM preview recommendation updated'));
        expect(success.llmPreviewRecommending.value).toBe(false);

        mockLlmPreviewRecommend.mockResolvedValueOnce({ data: { result: { suggestions: [] } } });
        await createBindings([selected]).bindings.applyLLMPreviewRecommendations();
        expect(mockShowMessage).toHaveBeenCalledWith('LLM preview recommendation completed, but no suggestions were generated');

        mockLlmPreviewRecommend.mockRejectedValueOnce(new Error('model unavailable'));
        const failure = createBindings([selected]).bindings;
        await failure.applyLLMPreviewRecommendations();
        expect(mockShowMessage).toHaveBeenCalledWith('model unavailable');
        expect(failure.llmPreviewRecommending.value).toBe(false);
    });

    test('LLM analysis guards empty state and analyzes all matching rows when no rows are selected', async () => {
        const selected = createTransaction(81);
        const noSession = createBindings([selected], '').bindings;
        await noSession.analyzeSelectedPreviewWithLLM();
        expect(mockAnalyzeLLMTransactions).not.toHaveBeenCalled();

        const busy = createBindings([selected]).bindings;
        busy.llmSessionAnalyzing.value = true;
        await busy.analyzeSelectedPreviewWithLLM();
        expect(mockAnalyzeLLMTransactions).not.toHaveBeenCalled();

        const empty = createBindings([]).bindings;
        await empty.analyzeSelectedPreviewWithLLM();
        expect(mockAnalyzeLLMTransactions).not.toHaveBeenCalled();
        expect(mockShowMessage).toHaveBeenCalledWith('No preview rows available for LLM analysis');
        expect(empty.llmSessionAnalyzing.value).toBe(false);

        mockAnalyzeLLMTransactions.mockResolvedValueOnce({ data: { result: { candidates_created: 0 } } });
        const unselected = createBindings([createTransaction(82, { selected: false })]).bindings;
        await unselected.analyzeSelectedPreviewWithLLM();
        expect(mockAnalyzeLLMTransactions).toHaveBeenLastCalledWith(expect.objectContaining({
            sessionId: 'signal-session',
            previewUpdates: [],
            actionScope: expect.objectContaining({ kind: 'all_matching' })
        }));
        expect(mockShowMessage).toHaveBeenCalledWith(expect.stringContaining('no candidate rules'));

        mockAnalyzeLLMTransactions.mockResolvedValueOnce({ data: { result: { candidates_created: 2 } } });
        const created = createBindings([selected]).bindings;
        await created.analyzeSelectedPreviewWithLLM();
        expect(mockAnalyzeLLMTransactions).toHaveBeenCalledWith(expect.objectContaining({
            sessionId: 'signal-session',
            previewUpdates: [expect.objectContaining({ id: 81 })],
            actionScope: expect.objectContaining({ kind: 'selected' })
        }));
        expect(mockShowMessage).toHaveBeenCalledWith(expect.stringContaining('created'));
        expect(created.llmSessionAnalyzing.value).toBe(false);

        mockAnalyzeLLMTransactions.mockResolvedValueOnce({ data: { result: { candidates_created: 0 } } });
        await createBindings([selected]).bindings.analyzeSelectedPreviewWithLLM();
        expect(mockShowMessage).toHaveBeenCalledWith(expect.stringContaining('no candidate rules'));

        mockAnalyzeLLMTransactions.mockRejectedValueOnce({ response: { status: 409, data: { code: 'stale_preview' } } });
        const failed = createBindings([selected]).bindings;
        await failed.analyzeSelectedPreviewWithLLM();
        expect(mockShowMessage).toHaveBeenCalledWith(expect.any(String));
        expect(mockLoggerError).toHaveBeenCalledWith(expect.stringContaining('code=stale_preview'));
        expect(failed.llmSessionAnalyzing.value).toBe(false);
    });

    test('long-term learning guards empty state and uses explicit selected promotion after scoped suggestions', async () => {
        const selected = createTransaction(91);
        await createBindings([selected], '').bindings.promoteSelectedToLongTermLearning();
        expect(mockGetImportLearningSuggestions).not.toHaveBeenCalled();

        await createBindings([]).bindings.promoteSelectedToLongTermLearning();
        expect(mockGetImportLearningSuggestions).not.toHaveBeenCalled();
        expect(mockShowMessage).toHaveBeenCalledWith('No preview rows available for learning');

        mockGetImportLearningSuggestions.mockResolvedValueOnce({ data: { result: { suggestions: [] } } });
        await createBindings([createTransaction(92, { selected: false })]).bindings.promoteSelectedToLongTermLearning();
        expect(mockGetImportLearningSuggestions).toHaveBeenLastCalledWith(expect.objectContaining({
            sessionId: 'signal-session',
            previewUpdates: [],
            actionScope: expect.objectContaining({ kind: 'all_matching' })
        }));

        mockGetImportLearningSuggestions.mockResolvedValueOnce({ data: { result: { suggestions: [] } } });
        await createBindings([selected]).bindings.promoteSelectedToLongTermLearning();
        expect(mockShowMessage).toHaveBeenCalledWith('No learning suggestions available for the selected preview rows');

        mockGetImportLearningSuggestions.mockResolvedValueOnce({ data: { result: { suggestions: [{ preview_id: 91 }] } } });
        const cancelled = createBindings([selected]).bindings;
        cancelled.importLearningSuggestionDialog.value = { open: jest.fn(async () => ({ previewIds: [] as number[] })) };
        await cancelled.promoteSelectedToLongTermLearning();
        expect(mockPromoteImportLearning).not.toHaveBeenCalled();

        mockGetImportLearningSuggestions.mockResolvedValueOnce({ data: { result: { suggestions: [{ preview_id: 91 }] } } });
        mockPromoteImportLearning.mockResolvedValueOnce({ data: { result: { rules_total: 3 } } });
        const success = createBindings([selected]).bindings;
        success.importLearningSuggestionDialog.value = { open: jest.fn(async () => ({ previewIds: [91] })) };
        await success.promoteSelectedToLongTermLearning();
        expect(mockGetImportLearningSuggestions).toHaveBeenCalledWith(expect.objectContaining({
            sessionId: 'signal-session',
            previewUpdates: [expect.objectContaining({ id: 91 })],
            actionScope: expect.objectContaining({ kind: 'selected' })
        }));
        expect(mockPromoteImportLearning).toHaveBeenCalledWith({
            sessionId: 'signal-session',
            actionScope: { kind: 'explicit_selected', preview_ids: [91] }
        });
        expect(mockShowMessage).toHaveBeenCalledWith(expect.stringContaining('Long-term learning saved'));

        mockGetImportLearningSuggestions.mockRejectedValueOnce(new Error('learning unavailable'));
        await createBindings([selected]).bindings.promoteSelectedToLongTermLearning();
        expect(mockShowMessage).toHaveBeenCalledWith('Promote failed: Error: learning unavailable');
    });
});
