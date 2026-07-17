import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockTransactionType = {
    ModifyBalance: 1,
    Income: 2,
    Expense: 3,
    Transfer: 4,
    Investment: 5
} as const;

const mockPageType = {
    Transaction: 'transaction',
    Template: 'template'
} as const;

const mockPageMode = {
    Add: 'add',
    Edit: 'edit',
    View: 'view'
} as const;

const mockGeoStatus = {
    Getting: 'getting',
    Success: 'success',
    Error: 'error'
} as const;

class MockTransaction {
    public id = '';
    public type: number = mockTransactionType.Expense;
    public sourceAmountCents = 1234;
    public destinationAmountCents = 1234;
    public expenseCategoryId = 'expense-category';
    public incomeCategoryId = 'income-category';
    public transferCategoryId = 'transfer-category';
    public investmentCategoryId = 'investment-category';
    public categoryId = 'expense-category';
    public accountId = 'account-1';
    public destinationAccountId = 'account-2';
    public tagIds: string[] = [];
    public tags: Array<{ id: string; name: string }> = [];
    public category: { id: string; name: string; type: number } | null = null;
    public editable = true;
    public time = 1_700_000_000;
    public timeZone = 'Asia/Shanghai';
    public utcOffset = 480;
    public geoLocation: { latitude: number; longitude: number } | null = null;
    public pictures: Array<{ pictureId: string }> = [];

    public static ofDraft(draft: Partial<MockTransaction>): MockTransaction {
        return Object.assign(new MockTransaction(), draft);
    }

    public setLatitudeAndLongitude(latitude: number, longitude: number): void {
        this.geoLocation = { latitude, longitude };
    }

    public removeGeoLocation(): void {
        this.geoLocation = null;
    }

    public clearPictures(): void {
        this.pictures = [];
    }

    public addPicture(picture: { pictureId: string }): void {
        this.pictures.push(picture);
    }

    public removePicture(picture: { pictureId: string }): void {
        this.pictures = this.pictures.filter(item => item.pictureId !== picture.pictureId);
    }
}

class MockTransactionTemplate extends MockTransaction {
    public templateType = 1;
    public scheduledFrequencyType = -1;
    public scheduledFrequency = 'monthly';
    public name = 'Template';

    public static createNewTransactionTemplate(transaction: MockTransaction): MockTransactionTemplate {
        return Object.assign(new MockTransactionTemplate(), transaction);
    }

    public fillFrom(template: MockTransactionTemplate): void {
        Object.assign(this, template);
    }
}

type MockBase = ReturnType<typeof createMockBase>;

let mockLastBase: MockBase;
let mockMountedCallback: (() => void) | undefined;
let mockUnmountedCallback: (() => void) | undefined;

const mockSetTransactionModel = jest.fn<(...args: any[]) => void>();
const mockApplyReceiptField = jest.fn<(...args: any[]) => boolean>();
const mockApplyReceiptAutoFill = jest.fn<(...args: any[]) => void>();
const mockBuildReceiptHints = jest.fn<(...args: any[]) => any[]>();
const mockGetCurrentToken = jest.fn<() => string>();
const mockIsSupportMapClick = jest.fn<() => boolean>();
const mockGenerateUuid = jest.fn<() => string>();
const mockGetDefaultCategories = jest.fn<(...args: any[]) => any[]>();
const mockLocalizeCategories = jest.fn<(items: any[]) => any[]>();
let mockGeoLocationSupported = true;

const mockLogger = {
    debug: jest.fn(),
    error: jest.fn(),
    info: jest.fn(),
    warn: jest.fn()
};

const mockSettingsStore = {
    appSettings: {
        autoSaveTransactionDraft: 'disabled',
        autoGetCurrentGeoLocation: false,
        timeZone: 'Asia/Shanghai'
    }
};

const mockUserStore = {
    updateUserTransactionEditScope: jest.fn<(args: any) => Promise<void>>()
};

const mockAccountsStore = {
    loadAllAccounts: jest.fn<(args: any) => Promise<void>>()
};

const mockCategoryStore = {
    loadAllCategories: jest.fn<(args: any) => Promise<void>>(),
    addPresetCategories: jest.fn<(args: any) => Promise<void>>()
};

const mockTagStore = {
    loadAllTags: jest.fn<(args: any) => Promise<void>>(),
    saveTag: jest.fn<(args: any) => Promise<any>>()
};

const mockTransactionsStore = {
    transactionDraft: null as Partial<MockTransaction> | null,
    loadAllTransactions: jest.fn(),
    getTransaction: jest.fn<(args: any) => Promise<any>>(),
    saveTransaction: jest.fn<(args: any) => Promise<void>>(),
    deleteTransaction: jest.fn<(args: any) => Promise<void>>(),
    uploadTransactionPicture: jest.fn<(args: any) => Promise<any>>(),
    recognizeReceiptImage: jest.fn<(args: any) => Promise<any>>(),
    removeUnusedTransactionPicture: jest.fn<(args: any) => Promise<boolean>>(),
    isTransactionDraftModified: jest.fn<(...args: any[]) => boolean>(),
    saveTransactionDraft: jest.fn(),
    clearTransactionDraft: jest.fn()
};

const mockTemplateStore = {
    getTemplate: jest.fn<(args: any) => Promise<any>>(),
    saveTemplateContent: jest.fn<(args: any) => Promise<void>>()
};

function createMockBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const transaction = ref(new MockTransaction());

    return {
        mode: ref(mockPageMode.Add),
        isSupportGeoLocation: mockGeoLocationSupported,
        editId: ref(null as string | null),
        addByTemplateId: ref(null as string | null),
        duplicateFromId: ref(null as string | null),
        clientSessionId: ref(''),
        loading: ref(false),
        submitting: ref(false),
        uploadingPicture: ref(false),
        geoLocationStatus: ref(null as string | null),
        setGeoLocationByClickMap: ref(false),
        transaction,
        defaultCurrency: ref('CNY'),
        defaultAccountId: ref('account-1'),
        coordinateDisplayType: ref('decimal'),
        allTimezones: ref([]),
        allVisibleAccounts: ref([]),
        allAccountsMap: ref({}),
        allVisibleCategorizedAccounts: ref([]),
        allCategories: ref([]),
        allCategoriesMap: ref({}),
        allTags: ref([] as Array<{ id: string; name: string; hidden: boolean }>),
        allTagsMap: ref({}),
        firstVisibleAccountId: ref('account-1'),
        hasAvailableExpenseCategories: ref(true),
        hasAvailableIncomeCategories: ref(true),
        hasAvailableTransferCategories: ref(true),
        hasAvailableInvestmentCategories: ref(true),
        canAddTransactionPicture: computed(() => true),
        title: computed(() => 'Edit transaction'),
        saveButtonTitle: computed(() => 'Save'),
        cancelButtonTitle: computed(() => 'Cancel'),
        sourceAmountName: computed(() => 'Amount'),
        sourceAmountTitle: computed(() => 'Amount'),
        sourceAccountTitle: computed(() => 'Account'),
        transferInAmountTitle: computed(() => 'Destination amount'),
        sourceAccountName: computed(() => 'Source'),
        destinationAccountName: computed(() => 'Destination'),
        sourceAccountCurrency: computed(() => 'CNY'),
        destinationAccountCurrency: computed(() => 'CNY'),
        transactionDisplayTimezone: computed(() => 'Asia/Shanghai'),
        transactionTimezoneTimeDifference: computed(() => 0),
        geoLocationStatusInfo: computed(() => ''),
        inputEmptyProblemMessage: ref(''),
        inputIsEmpty: computed(() => false),
        createNewTransactionModel: jest.fn((type?: number) => {
            const created = new MockTransaction();
            if (type) {
                created.type = type;
            }
            return created;
        }),
        swapTransactionData: jest.fn(),
        getTransactionPictureUrl: jest.fn((picture: { pictureId: string }) => `/pictures/${picture.pictureId}`)
    };
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(null),
        onMounted: (callback: () => void) => {
            mockMountedCallback = callback;
        },
        onUnmounted: (callback: () => void) => {
            mockUnmountedCallback = callback;
        }
    };
});

jest.mock('@/lib/vue_external_template.ts', () => ({
    useExternalTemplateBindings: jest.fn()
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentLanguageTag: () => 'zh-Hans',
        getAllTransactionDefaultCategories: (...args: any[]) => mockGetDefaultCategories(...args)
    })
}));

jest.mock('@/views/base/transactions/TransactionEditPageBase.ts', () => ({
    TransactionEditPageMode: mockPageMode,
    TransactionEditPageType: mockPageType,
    GeoLocationStatus: mockGeoStatus,
    useTransactionEditPageBase: () => {
        mockLastBase = createMockBase();
        return mockLastBase;
    }
}));

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoryStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/stores/transactionTemplate.ts', () => ({ useTransactionTemplatesStore: () => mockTemplateStore }));

jest.mock('@/core/category.ts', () => ({ CategoryType: { Expense: { type: 1 } } }));
jest.mock('@/core/transaction.ts', () => ({
    TransactionType: mockTransactionType,
    TransactionEditScopeType: { All: { type: 1 } }
}));
jest.mock('@/core/template.ts', () => ({
    TemplateType: { Normal: { type: 1 }, Schedule: { type: 2 } },
    ScheduledTemplateFrequencyType: { Disabled: { type: 0 } }
}));
jest.mock('@/consts/api.ts', () => ({
    KnownErrorCode: {
        TransactionCannotCreateInThisTime: 'cannot_create',
        TransactionCannotModifyInThisTime: 'cannot_modify',
        TransactionPictureNotFound: 'picture_not_found'
    }
}));

jest.mock('@/models/transaction.ts', () => ({ Transaction: MockTransaction }));
jest.mock('@/models/transaction_template.ts', () => ({ TransactionTemplate: MockTransactionTemplate }));
jest.mock('@/models/transaction_tag.ts', () => ({
    TransactionTag: { createNewTag: (name: string) => ({ id: '', name }) }
}));
jest.mock('@/models/large_language_model.ts', () => ({
    RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD: 0.6
}));

jest.mock('@/lib/datetime.ts', () => ({
    getTimezoneOffsetMinutes: () => 480,
    getCurrentUnixTime: () => 1_800_000_000
}));
jest.mock('@/lib/common.ts', () => ({
    categorizedArrayToPlainArray: (items: any[]) => items
}));
jest.mock('@/lib/coordinate.ts', () => ({
    formatCoordinate: (value: number) => String(value)
}));
jest.mock('@/lib/misc.ts', () => ({
    generateRandomUUID: () => mockGenerateUuid()
}));
jest.mock('@/lib/userstate.ts', () => ({
    getCurrentToken: () => mockGetCurrentToken()
}));
jest.mock('@/lib/category.ts', () => ({
    getTransactionPrimaryCategoryName: () => 'Primary',
    getTransactionSecondaryCategoryName: () => 'Secondary',
    localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories: (items: any[]) => mockLocalizeCategories(items)
}));
jest.mock('@/lib/transaction.ts', () => ({
    setTransactionModelByTransaction: (...args: any[]) => mockSetTransactionModel(...args)
}));
jest.mock('@/lib/receiptDraft.ts', () => ({
    applyReceiptDraftAutoFillToTransaction: (...args: any[]) => mockApplyReceiptAutoFill(...args),
    applyReceiptDraftFieldToTransaction: (...args: any[]) => mockApplyReceiptField(...args),
    buildReceiptDraftCandidateHints: (...args: any[]) => mockBuildReceiptHints(...args),
    getReceiptDraftCandidateDisplayValue: () => 'candidate'
}));
jest.mock('@/views/desktop/transactions/list/dialogs/edit-dialog/recurringCandidateDisplay.ts', () => ({
    createRecurringCandidateSubtitleFormatter: () => (candidate: any) => candidate.subtitle || '',
    getRecurringCandidatePrimaryReason: (candidate: any) => candidate.primaryReason || ''
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isTransactionPicturesEnabled: () => true,
    getMapProvider: () => 'test-map'
}));
jest.mock('@/lib/map/index.ts', () => ({
    isSupportGetGeoLocationByClick: () => mockIsSupportMapClick()
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: mockLogger
}));

for (const componentPath of [
    '@/components/common/MapView.vue',
    '@/components/desktop/ConfirmDialog.vue',
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/transactions/list/dialogs/BillMatchingPanel.vue',
    '@/views/desktop/transactions/list/dialogs/TransactionPicturesPanel.vue'
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: { name: 'TransactionEditCoverageStub' }
    }));
}

import EditDialog from '@/views/desktop/transactions/list/dialogs/EditDialog.vue';

function makeBindings(type: string = mockPageType.Transaction): any {
    const expose = jest.fn();
    const bindings = (EditDialog as any).setup({ type, show: true }, { expose });
    expect(expose).toHaveBeenCalledWith({ open: bindings.open });
    return bindings;
}

async function flushPromises(times = 4): Promise<void> {
    for (let index = 0; index < times; index++) {
        await Promise.resolve();
    }
}

function installDialogSpies(bindings: any): {
    confirm: { open: ReturnType<typeof jest.fn<(...args: any[]) => Promise<any>>> };
    snackbar: { showMessage: jest.Mock; showError: jest.Mock };
} {
    const confirm = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    bindings.confirmDialog.value = confirm;
    bindings.snackbar.value = snackbar;
    return { confirm, snackbar };
}

function mockResponse(options: {
    ok?: boolean;
    status?: number;
    json?: any;
    text?: string;
} = {}): any {
    return {
        ok: options.ok ?? true,
        status: options.status ?? 200,
        json: jest.fn<() => Promise<any>>().mockResolvedValue(options.json ?? { success: true, result: {} }),
        text: jest.fn<() => Promise<string>>().mockResolvedValue(options.text ?? '')
    };
}

function fetchMock(): ReturnType<typeof jest.fn<(...args: any[]) => Promise<any>>> {
    return globalThis.fetch as ReturnType<typeof jest.fn<(...args: any[]) => Promise<any>>>;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockMountedCallback = undefined;
    mockUnmountedCallback = undefined;
    mockSettingsStore.appSettings.autoSaveTransactionDraft = 'disabled';
    mockSettingsStore.appSettings.autoGetCurrentGeoLocation = false;
    mockTransactionsStore.transactionDraft = null;
    mockGetCurrentToken.mockReturnValue('test-token');
    mockGenerateUuid.mockReturnValue('session-uuid');
    mockIsSupportMapClick.mockReturnValue(true);
    mockGetDefaultCategories.mockReturnValue([{ id: 'preset' }]);
    mockLocalizeCategories.mockImplementation(items => items);
    mockGeoLocationSupported = true;
    mockApplyReceiptField.mockReturnValue(true);
    mockBuildReceiptHints.mockReturnValue([]);
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
    mockCategoryStore.addPresetCategories.mockResolvedValue(undefined);
    mockTagStore.loadAllTags.mockResolvedValue(undefined);
    mockTagStore.saveTag.mockResolvedValue({ id: 'tag-new', name: 'New tag' });
    mockTransactionsStore.getTransaction.mockResolvedValue(null);
    mockTransactionsStore.saveTransaction.mockResolvedValue(undefined);
    mockTransactionsStore.deleteTransaction.mockResolvedValue(undefined);
    mockTransactionsStore.uploadTransactionPicture.mockResolvedValue({ pictureId: 'picture-new' });
    mockTransactionsStore.recognizeReceiptImage.mockResolvedValue({ draft: {}, confidence: 0.9 });
    mockTransactionsStore.removeUnusedTransactionPicture.mockResolvedValue(true);
    mockTransactionsStore.isTransactionDraftModified.mockReturnValue(false);
    mockTemplateStore.getTemplate.mockResolvedValue(null);
    mockTemplateStore.saveTemplateContent.mockResolvedValue(undefined);
    mockUserStore.updateUserTransactionEditScope.mockResolvedValue(undefined);
    mockSetTransactionModel.mockImplementation((target: MockTransaction, source: MockTransaction | null, _categories: any, _categoryMap: any, _accounts: any, _accountMap: any, _tagMap: any, _defaultAccount: any, options: any) => {
        if (source) {
            Object.assign(target, source);
        }
        if (options.type) {
            target.type = options.type;
        }
        if (options.sourceAmountCents !== undefined) {
            target.sourceAmountCents = options.sourceAmountCents;
        }
        if (options.destinationAmountCents !== undefined) {
            target.destinationAmountCents = options.destinationAmountCents;
        }
    });

    Object.defineProperty(globalThis, 'fetch', {
        configurable: true,
        value: jest.fn(),
        writable: true
    });
    Object.defineProperty(globalThis, 'navigator', {
        configurable: true,
        value: { geolocation: { getCurrentPosition: jest.fn() } },
        writable: true
    });
    Object.defineProperty(globalThis, 'HTMLInputElement', {
        configurable: true,
        value: class HTMLInputElement {},
        writable: true
    });
    Object.defineProperty(globalThis, 'HTMLTextAreaElement', {
        configurable: true,
        value: class HTMLTextAreaElement {},
        writable: true
    });
    Object.assign(globalThis.window, {
        addEventListener: jest.fn(),
        removeEventListener: jest.fn(),
        open: jest.fn()
    });
});

describe('desktop transaction EditDialog production behavior', () => {
    test('loads the production SFC and exposes core computed contracts', async () => {
        const bindings = makeBindings();
        installDialogSpies(bindings);

        expect(mockMountedCallback).toBeDefined();
        expect(mockUnmountedCallback).toBeDefined();
        mockMountedCallback?.();
        mockUnmountedCallback?.();
        expect(window.addEventListener).toHaveBeenCalledWith('keydown', bindings.onKeydown);
        expect(window.removeEventListener).toHaveBeenCalledWith('keydown', bindings.onKeydown);
        await flushPromises();

        expect(bindings.sourceAmountColor.value).toBe('expense');
        bindings.transaction.value.type = mockTransactionType.Income;
        expect(bindings.sourceAmountColor.value).toBe('income');
        bindings.transaction.value.type = mockTransactionType.Transfer;
        expect(bindings.sourceAmountColor.value).toBe('primary');
        bindings.transaction.value.type = mockTransactionType.Investment;
        expect(bindings.sourceAmountColor.value).toBeUndefined();

        bindings.allTags.value = [
            { id: 'hidden', name: 'Hidden coffee', hidden: true },
            { id: 'visible', name: 'Visible tea', hidden: false }
        ];
        bindings.tagSearchContent.value = 'coffee';
        expect(bindings.isAllFilteredTagHidden.value).toBe(true);
        bindings.tagSearchContent.value = 'tea';
        expect(bindings.isAllFilteredTagHidden.value).toBe(false);
        bindings.tagSearchContent.value = 'missing';
        expect(bindings.isAllFilteredTagHidden.value).toBe(false);

        bindings.mode.value = mockPageMode.Add;
        mockTransactionsStore.isTransactionDraftModified.mockReturnValueOnce(true);
        expect(bindings.isTransactionModified.value).toBe(true);
        bindings.mode.value = mockPageMode.Edit;
        expect(bindings.isTransactionModified.value).toBe(true);
        bindings.mode.value = mockPageMode.View;
        expect(bindings.isTransactionModified.value).toBe(false);
    });

    test('tracks recurring display, candidates, auth, and reset state', () => {
        const bindings = makeBindings();

        expect(bindings.recurringMatchDisplayText.value).toBe('None');
        bindings.recurringCandidateCount.value = 2;
        expect(bindings.recurringMatchDisplayText.value).toBe('Scheduled Candidates 2');
        bindings.linkedRecurringId.value = '42';
        expect(bindings.recurringMatchDisplayText.value).toBe('#42');
        bindings.linkedRecurringName.value = 'Monthly rent';
        expect(bindings.recurringMatchDisplayText.value).toBe('Monthly rent');

        bindings.recurringCandidates.value = [
            { id: 42, name: 'Monthly rent', primaryReason: 'same merchant' },
            { id: 43, name: 'Utilities', primaryReason: 'same amount' }
        ];
        expect(bindings.recurringMatchPrimaryReason.value).toBe('same merchant');
        expect(bindings.isBestRecurringCandidate({ id: '42' })).toBe(true);
        expect(bindings.isBestRecurringCandidate({ id: 43 })).toBe(false);
        bindings.linkedRecurringId.value = '43';
        expect(bindings.recurringMatchPrimaryReason.value).toBe('same amount');
        bindings.recurringCandidates.value = [];
        expect(bindings.recurringMatchPrimaryReason.value).toBe('');
        expect(bindings.isBestRecurringCandidate({ id: 1 })).toBe(false);

        expect(bindings.buildRecurringAuthHeaders()).toStrictEqual({ Authorization: 'Bearer test-token' });
        mockGetCurrentToken.mockReturnValueOnce('');
        expect(bindings.buildRecurringAuthHeaders()).toStrictEqual({});

        bindings.loadingRecurringCandidates.value = true;
        bindings.recurringBindingSubmitting.value = true;
        bindings.showRecurringCandidateDialog.value = true;
        bindings.selectedRecurringCandidateId.value = '42';
        bindings.resetBillRecurringState();
        expect(bindings.loadingRecurringCandidates.value).toBe(false);
        expect(bindings.recurringBindingSubmitting.value).toBe(false);
        expect(bindings.showRecurringCandidateDialog.value).toBe(false);
        expect(bindings.selectedRecurringCandidateId.value).toBe('');
    });

    test('loads, binds, clears, and reports recurring candidates', async () => {
        const bindings = makeBindings();
        const { snackbar } = installDialogSpies(bindings);

        bindings.editId.value = 'bill-7';
        bindings.mode.value = mockPageMode.Add;
        bindings.recurringCandidates.value = [{ id: 1 }];
        await bindings.refreshBillRecurringCandidates();
        expect(bindings.recurringCandidates.value).toStrictEqual([]);

        bindings.mode.value = mockPageMode.View;
        fetchMock().mockResolvedValueOnce(mockResponse({
            json: {
                success: true,
                result: {
                    candidates: [
                        { id: 42, name: 'Rent', primaryReason: 'same merchant' },
                        { id: 43, name: 'Utilities' }
                    ],
                    linkedRecurringId: 42,
                    linkedRecurringName: ''
                }
            }
        }));
        await bindings.refreshBillRecurringCandidates();
        expect(fetchMock()).toHaveBeenLastCalledWith('/api/bills/bill-7/recurring-candidates?toleranceDays=3', {
            method: 'GET',
            headers: { Authorization: 'Bearer test-token' }
        });
        expect(bindings.linkedRecurringName.value).toBe('Rent');
        expect(bindings.selectedRecurringCandidateId.value).toBe('42');
        expect(bindings.loadingRecurringCandidates.value).toBe(false);

        fetchMock().mockResolvedValueOnce(mockResponse({
            json: {
                success: true,
                result: { candidates: [{ id: 9 }], linkedRecurringName: 'Named schedule' }
            }
        }));
        await bindings.openBillRecurringCandidateDialog();
        expect(bindings.showRecurringCandidateDialog.value).toBe(true);
        expect(bindings.selectedRecurringCandidateId.value).toBe('9');
        bindings.closeBillRecurringCandidateDialog();
        expect(bindings.showRecurringCandidateDialog.value).toBe(false);

        fetchMock().mockResolvedValueOnce(mockResponse({ ok: false, status: 503, text: 'down' }));
        await bindings.refreshBillRecurringCandidates();
        expect(snackbar.showMessage).toHaveBeenCalledWith('Load Scheduled Candidates Failed');
        expect(mockLogger.error).toHaveBeenCalled();

        snackbar.showMessage.mockClear();
        fetchMock().mockResolvedValueOnce(mockResponse({ json: { success: false, error: 'bad envelope' } }));
        await bindings.refreshBillRecurringCandidates(true);
        expect(snackbar.showMessage).not.toHaveBeenCalled();

        bindings.editId.value = '';
        bindings.selectedRecurringCandidateId.value = '';
        await bindings.applyBillRecurringCandidate();
        bindings.linkedRecurringId.value = '';
        await bindings.clearBillRecurringMatch();
        expect(fetchMock()).toHaveBeenCalledTimes(4);

        bindings.editId.value = 'bill-7';
        bindings.selectedRecurringCandidateId.value = '43';
        fetchMock()
            .mockResolvedValueOnce(mockResponse({ json: { success: true } }))
            .mockResolvedValueOnce(mockResponse({ json: { success: true, result: { candidates: [] } } }));
        await bindings.applyBillRecurringCandidate();
        expect(fetchMock()).toHaveBeenNthCalledWith(5, '/api/bills/bill-7/recurring-match', expect.objectContaining({
            method: 'PUT',
            body: JSON.stringify({ recurringId: '43' })
        }));
        expect(bindings.showRecurringCandidateDialog.value).toBe(false);
        expect(bindings.recurringBindingSubmitting.value).toBe(false);

        bindings.linkedRecurringId.value = '43';
        fetchMock()
            .mockResolvedValueOnce(mockResponse({ json: { success: true } }))
            .mockResolvedValueOnce(mockResponse({ json: { success: true, result: { candidates: [] } } }));
        await bindings.clearBillRecurringMatch();
        expect(fetchMock()).toHaveBeenNthCalledWith(7, '/api/bills/bill-7/recurring-match', expect.objectContaining({
            method: 'DELETE'
        }));

        bindings.linkedRecurringId.value = '43';
        bindings.clearBillRecurringMatchFromDialog();
        await flushPromises();
    });

    test('surfaces recurring mutation failures for Error and non-Error values', async () => {
        const bindings = makeBindings();
        const { snackbar } = installDialogSpies(bindings);
        bindings.editId.value = 'bill-9';
        bindings.mode.value = mockPageMode.View;
        bindings.selectedRecurringCandidateId.value = 'recurring-1';
        bindings.linkedRecurringId.value = 'recurring-1';

        fetchMock().mockResolvedValueOnce(mockResponse({ ok: false, status: 409, text: 'conflict' }));
        await bindings.applyBillRecurringCandidate();
        expect(snackbar.showError).toHaveBeenCalledWith('Bind recurring match failed: 409 conflict');

        fetchMock().mockResolvedValueOnce(mockResponse({ json: { success: false, error: 'bind rejected' } }));
        await bindings.applyBillRecurringCandidate();
        expect(snackbar.showError).toHaveBeenCalledWith('bind rejected');

        fetchMock().mockResolvedValueOnce(mockResponse({ ok: false, status: 500, text: 'clear failed' }));
        await bindings.clearBillRecurringMatch();
        expect(snackbar.showError).toHaveBeenCalledWith('Clear recurring match failed: 500 clear failed');

        fetchMock().mockResolvedValueOnce(mockResponse({ json: { success: false, error: '' } }));
        await bindings.clearBillRecurringMatch();
        expect(snackbar.showError).toHaveBeenCalledWith('Unknown error');

        bindings.showRecurringOperationError({ reason: 'plain' });
        expect(snackbar.showError).toHaveBeenCalledWith('[object Object]');
    });

    test('opens new transactions from explicit input, templates, drafts, and geo defaults', async () => {
        const bindings = makeBindings();
        installDialogSpies(bindings);

        const openPromise = bindings.open({
            type: mockTransactionType.Income,
            sourceAmountCents: 2501,
            destinationAmountCents: 2501,
            accountId: 'account-1',
            noTransactionDraft: true
        });
        await flushPromises();
        expect(bindings.mode.value).toBe(mockPageMode.Add);
        expect(bindings.editId.value).toBeNull();
        expect(bindings.clientSessionId.value).toBe('session-uuid');
        expect(bindings.transaction.value.sourceAmountCents).toBe(2501);
        expect(bindings.noTransactionDraft.value).toBe(true);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(mockTagStore.loadAllTags).toHaveBeenCalledWith({ force: true });
        expect(bindings.loading.value).toBe(false);
        void openPromise.catch(() => undefined);

        const template = Object.assign(new MockTransactionTemplate(), { id: 'template-1', sourceAmountCents: 8800 });
        bindings.open({ type: mockTransactionType.Expense, template });
        await flushPromises();
        expect(bindings.addByTemplateId.value).toBe('template-1');

        mockSettingsStore.appSettings.autoSaveTransactionDraft = 'enabled';
        mockTransactionsStore.transactionDraft = { sourceAmountCents: 9900, type: mockTransactionType.Expense };
        bindings.open({ type: mockTransactionType.Expense });
        await flushPromises();
        expect(mockSetTransactionModel).toHaveBeenCalledWith(
            expect.any(MockTransaction),
            expect.any(MockTransaction),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            false,
            false
        );

        mockSettingsStore.appSettings.autoGetCurrentGeoLocation = true;
        mockTransactionsStore.transactionDraft = null;
        bindings.open({ type: mockTransactionType.Expense, noTransactionDraft: true });
        await flushPromises();
        expect(navigator.geolocation.getCurrentPosition).toHaveBeenCalled();
    });

    test('opens existing transactions and reports missing or failed loads', async () => {
        const loaded = Object.assign(new MockTransaction(), {
            id: 'bill-11',
            tagIds: ['tag-1', 'missing-tag'],
            tags: [{ id: 'tag-1', name: 'Coffee' }],
            category: { id: 'category-1', name: 'Food', type: mockTransactionType.Expense },
            editable: false
        });
        mockTransactionsStore.getTransaction.mockResolvedValueOnce(loaded);
        fetchMock().mockResolvedValueOnce(mockResponse({ json: { success: true, result: { candidates: [] } } }));
        const bindings = makeBindings();
        bindings.allTags.value = [{ id: 'tag-1', name: 'Coffee', hidden: false }];
        bindings.allTagsMap.value = { 'tag-1': bindings.allTags.value[0] };
        const pending = bindings.open({
            type: mockTransactionType.Expense,
            id: 'bill-11',
            currentTransaction: loaded
        });
        await flushPromises(8);
        expect(bindings.mode.value).toBe(mockPageMode.View);
        expect(bindings.originalTransactionEditable.value).toBe(false);
        expect(mockLogger.debug).toHaveBeenCalled();
        void pending.catch(() => undefined);

        const missingBindings = makeBindings();
        mockTransactionsStore.getTransaction.mockResolvedValueOnce(null);
        const missingPromise = missingBindings.open({ type: mockTransactionType.Expense, id: 'missing' });
        await expect(missingPromise).rejects.toBe('Unable to retrieve transaction');

        const failedBindings = makeBindings();
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'accounts failed' });
        const failedPromise = failedBindings.open({ type: mockTransactionType.Expense });
        await expect(failedPromise).rejects.toMatchObject({ message: 'accounts failed' });
        expect(failedBindings.showState.value).toBe(false);

        const processedBindings = makeBindings();
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: true });
        const processedPromise = processedBindings.open({ type: mockTransactionType.Expense });
        void processedPromise.catch(() => undefined);
        await flushPromises();
        expect(processedBindings.showState.value).toBe(false);
    });

    test('opens scheduled templates for add and edit contracts', async () => {
        const addBindings = makeBindings(mockPageType.Template);
        const addPromise = addBindings.open({
            type: mockTransactionType.Expense,
            templateType: 2
        });
        await flushPromises();
        expect(addBindings.mode.value).toBe(mockPageMode.Add);
        expect(addBindings.transaction.value).toBeInstanceOf(MockTransactionTemplate);
        expect(addBindings.transaction.value.templateType).toBe(2);
        expect(addBindings.transaction.value.scheduledFrequencyType).toBe(0);
        expect(addBindings.transaction.value.scheduledFrequency).toBe('');
        void addPromise.catch(() => undefined);

        const currentTemplate = Object.assign(new MockTransactionTemplate(), {
            id: 'template-8',
            templateType: 1,
            name: 'Current'
        });
        const loadedTemplate = Object.assign(new MockTransactionTemplate(), {
            id: 'template-8',
            templateType: 1,
            name: 'Loaded'
        });
        mockTemplateStore.getTemplate.mockResolvedValueOnce(loadedTemplate);
        const editBindings = makeBindings(mockPageType.Template);
        const editPromise = editBindings.open({
            type: mockTransactionType.Expense,
            id: 'template-8',
            currentTemplate
        });
        await flushPromises();
        expect(editBindings.mode.value).toBe(mockPageMode.Edit);
        expect(editBindings.transaction.value.name).toBe('Loaded');
        void editPromise.catch(() => undefined);

        mockTemplateStore.getTemplate.mockResolvedValueOnce(null);
        const missingBindings = makeBindings(mockPageType.Template);
        await expect(missingBindings.open({
            type: mockTransactionType.Expense,
            id: 'missing-template'
        })).rejects.toBe('Unable to retrieve template');
    });

    test('saves add, edit, zero-cent, and template flows with integer-cent payloads', async () => {
        const addBindings = makeBindings();
        const addDialog = installDialogSpies(addBindings);
        const addResult = addBindings.open({
            type: mockTransactionType.Expense,
            sourceAmountCents: 1234
        });
        await flushPromises();
        addBindings.save();
        await expect(addResult).resolves.toStrictEqual({ message: 'You have added a new transaction' });
        expect(mockTransactionsStore.saveTransaction).toHaveBeenCalledWith({
            transaction: expect.objectContaining({ sourceAmountCents: 1234 }),
            defaultCurrency: 'CNY',
            isEdit: false,
            clientSessionId: 'session-uuid'
        });
        expect(mockTransactionsStore.clearTransactionDraft).toHaveBeenCalled();
        expect(addBindings.showState.value).toBe(false);
        expect(addDialog.confirm.open).not.toHaveBeenCalled();

        const zeroBindings = makeBindings();
        const zeroDialog = installDialogSpies(zeroBindings);
        zeroDialog.confirm.open.mockResolvedValue(undefined);
        const zeroResult = zeroBindings.open({
            type: mockTransactionType.Expense,
            sourceAmountCents: 0,
            noTransactionDraft: true
        });
        await flushPromises();
        zeroBindings.save();
        await expect(zeroResult).resolves.toStrictEqual({ message: 'You have added a new transaction' });
        expect(zeroDialog.confirm.open).toHaveBeenCalledWith('Are you sure you want to save this transaction with a zero amount?');

        const loaded = Object.assign(new MockTransaction(), { id: 'bill-edit', sourceAmountCents: 5678 });
        mockTransactionsStore.getTransaction.mockResolvedValueOnce(loaded);
        fetchMock().mockResolvedValueOnce(mockResponse({ json: { success: true, result: { candidates: [] } } }));
        const editBindings = makeBindings();
        installDialogSpies(editBindings);
        const editResult = editBindings.open({ type: mockTransactionType.Expense, id: 'bill-edit' });
        await flushPromises(8);
        editBindings.edit();
        editBindings.save();
        await expect(editResult).resolves.toStrictEqual({ message: 'You have saved this transaction' });
        expect(mockTransactionsStore.saveTransaction).toHaveBeenLastCalledWith(expect.objectContaining({ isEdit: true }));

        const templateAdd = makeBindings(mockPageType.Template);
        installDialogSpies(templateAdd);
        const templateAddResult = templateAdd.open({ type: mockTransactionType.Expense });
        await flushPromises();
        templateAdd.save();
        await expect(templateAddResult).resolves.toStrictEqual({ message: 'You have added a new template' });
        expect(mockTemplateStore.saveTemplateContent).toHaveBeenLastCalledWith(expect.objectContaining({ isEdit: false }));

        const template = Object.assign(new MockTransactionTemplate(), { id: 'template-save', templateType: 1 });
        mockTemplateStore.getTemplate.mockResolvedValueOnce(template);
        const templateEdit = makeBindings(mockPageType.Template);
        installDialogSpies(templateEdit);
        const templateEditResult = templateEdit.open({
            type: mockTransactionType.Expense,
            id: 'template-save',
            currentTemplate: template
        });
        await flushPromises();
        templateEdit.save();
        await expect(templateEditResult).resolves.toStrictEqual({ message: 'You have saved this template' });
        expect(mockTemplateStore.saveTemplateContent).toHaveBeenLastCalledWith(expect.objectContaining({ isEdit: true }));
    });

    test('validates save input and handles transaction and template failures', async () => {
        const bindings = makeBindings();
        const { confirm, snackbar } = installDialogSpies(bindings);
        bindings.mode.value = mockPageMode.Add;
        bindings.inputEmptyProblemMessage.value = 'Amount is required';
        bindings.save();
        expect(snackbar.showMessage).toHaveBeenCalledWith('Amount is required');

        bindings.inputEmptyProblemMessage.value = '';
        mockTransactionsStore.saveTransaction.mockRejectedValueOnce({ processed: false, message: 'save failed' });
        bindings.save();
        await flushPromises();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'save failed' }));

        snackbar.showError.mockClear();
        mockTransactionsStore.saveTransaction.mockRejectedValueOnce({ processed: true });
        bindings.save();
        await flushPromises();
        expect(snackbar.showError).not.toHaveBeenCalled();

        confirm.open.mockResolvedValue(undefined);
        mockTransactionsStore.saveTransaction.mockRejectedValueOnce({
            processed: false,
            error: { errorCode: 'cannot_create' }
        });
        bindings.save();
        await flushPromises(8);
        expect(mockUserStore.updateUserTransactionEditScope).toHaveBeenCalledWith({ transactionEditScope: 1 });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Your editable transaction range has been set to All');

        mockUserStore.updateUserTransactionEditScope.mockRejectedValueOnce({ processed: false, message: 'scope failed' });
        mockTransactionsStore.saveTransaction.mockRejectedValueOnce({
            processed: false,
            error: { errorCode: 'cannot_modify' }
        });
        bindings.save();
        await flushPromises(8);
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'scope failed' }));

        const templateBindings = makeBindings(mockPageType.Template);
        const templateDialog = installDialogSpies(templateBindings);
        templateBindings.mode.value = mockPageMode.Add;
        mockTemplateStore.saveTemplateContent.mockRejectedValueOnce({ processed: false, message: 'template failed' });
        templateBindings.save();
        await flushPromises();
        expect(templateDialog.snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'template failed' }));

        templateDialog.snackbar.showError.mockClear();
        mockTemplateStore.saveTemplateContent.mockRejectedValueOnce({ processed: true });
        templateBindings.save();
        await flushPromises();
        expect(templateDialog.snackbar.showError).not.toHaveBeenCalled();

        templateBindings.mode.value = mockPageMode.View;
        templateBindings.save();
        bindings.mode.value = mockPageMode.View;
        bindings.save();
    });

    test('adds preset categories and gates selector prompts across UI states', async () => {
        const bindings = makeBindings();
        const { confirm, snackbar } = installDialogSpies(bindings);

        expect(await bindings.addDefaultCategories()).toBe(true);
        expect(mockCategoryStore.addPresetCategories).toHaveBeenCalledWith({ categories: [{ id: 'preset' }] });
        expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledWith({ force: true });
        expect(bindings.addingDefaultCategories.value).toBe(false);

        bindings.addingDefaultCategories.value = true;
        expect(await bindings.addDefaultCategories()).toBe(false);
        bindings.addingDefaultCategories.value = false;

        mockGetDefaultCategories.mockReturnValueOnce([]);
        expect(await bindings.addDefaultCategories()).toBe(false);
        expect(snackbar.showMessage).toHaveBeenCalledWith('No available category');

        mockCategoryStore.addPresetCategories.mockRejectedValueOnce({ processed: false, message: 'preset failed' });
        expect(await bindings.addDefaultCategories()).toBe(false);
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'preset failed' }));

        snackbar.showError.mockClear();
        mockCategoryStore.addPresetCategories.mockRejectedValueOnce({ processed: true });
        expect(await bindings.addDefaultCategories()).toBe(false);
        expect(snackbar.showError).not.toHaveBeenCalled();

        bindings.onCategorySelectorClick(true);
        bindings.mode.value = mockPageMode.View;
        bindings.onCategorySelectorClick(false);
        bindings.mode.value = mockPageMode.Add;
        bindings.loading.value = true;
        bindings.onCategorySelectorClick(false);
        bindings.loading.value = false;
        bindings.submitting.value = true;
        bindings.onCategorySelectorClick(false);
        bindings.submitting.value = false;
        bindings.addingDefaultCategories.value = true;
        bindings.onCategorySelectorClick(false);
        expect(confirm.open).not.toHaveBeenCalled();

        bindings.addingDefaultCategories.value = false;
        confirm.open.mockResolvedValueOnce(undefined);
        bindings.onCategorySelectorClick(false);
        await flushPromises(8);
        expect(snackbar.showMessage).toHaveBeenCalledWith('You have added preset categories');

        confirm.open.mockRejectedValueOnce(new Error('cancelled'));
        bindings.onCategorySelectorClick(false);
        await flushPromises();
    });

    test('duplicates and edits readonly transactions while preserving cent values', () => {
        const bindings = makeBindings();
        bindings.mode.value = mockPageMode.Add;
        bindings.duplicate();
        bindings.edit();
        expect(bindings.mode.value).toBe(mockPageMode.Add);

        bindings.mode.value = mockPageMode.View;
        bindings.transaction.value.id = 'bill-source';
        bindings.transaction.value.sourceAmountCents = 4321;
        bindings.transaction.value.geoLocation = { latitude: 1, longitude: 2 };
        bindings.transaction.value.pictures = [{ pictureId: 'old-picture' }];
        bindings.duplicate();
        expect(bindings.duplicateFromId.value).toBe('bill-source');
        expect(bindings.transaction.value.id).toBe('');
        expect(bindings.transaction.value.sourceAmountCents).toBe(4321);
        expect(bindings.transaction.value.time).toBe(1_800_000_000);
        expect(bindings.transaction.value.utcOffset).toBe(480);
        expect(bindings.transaction.value.geoLocation).toBeNull();
        expect(bindings.transaction.value.pictures).toStrictEqual([]);
        expect(bindings.mode.value).toBe(mockPageMode.Add);

        bindings.mode.value = mockPageMode.View;
        bindings.transaction.value.id = 'bill-source-2';
        bindings.transaction.value.time = 111;
        bindings.transaction.value.geoLocation = { latitude: 3, longitude: 4 };
        bindings.duplicate(true, true);
        expect(bindings.transaction.value.time).toBe(111);
        expect(bindings.transaction.value.geoLocation).toStrictEqual({ latitude: 3, longitude: 4 });

        bindings.mode.value = mockPageMode.View;
        bindings.edit();
        expect(bindings.mode.value).toBe(mockPageMode.Edit);

        const templateBindings = makeBindings(mockPageType.Template);
        templateBindings.mode.value = mockPageMode.View;
        templateBindings.duplicate();
        templateBindings.edit();
        expect(templateBindings.mode.value).toBe(mockPageMode.View);
    });

    test('deletes transactions and reports deletion failures', async () => {
        const bindings = makeBindings();
        const { confirm, snackbar } = installDialogSpies(bindings);
        bindings.mode.value = mockPageMode.Add;
        bindings.remove();
        expect(confirm.open).not.toHaveBeenCalled();

        bindings.mode.value = mockPageMode.View;
        confirm.open.mockResolvedValueOnce(undefined);
        bindings.remove();
        expect(confirm.open).toHaveBeenCalledWith('Are you sure you want to delete this transaction?');
        await flushPromises();
        expect(mockTransactionsStore.deleteTransaction).toHaveBeenCalledWith({
            transaction: bindings.transaction.value,
            defaultCurrency: 'CNY'
        });
        expect(bindings.showState.value).toBe(false);

        bindings.showState.value = true;
        confirm.open.mockResolvedValueOnce(undefined);
        mockTransactionsStore.deleteTransaction.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.remove();
        await flushPromises();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'delete failed' }));

        snackbar.showError.mockClear();
        confirm.open.mockResolvedValueOnce(undefined);
        mockTransactionsStore.deleteTransaction.mockRejectedValueOnce({ processed: true });
        bindings.remove();
        await flushPromises();
        expect(snackbar.showError).not.toHaveBeenCalled();

        const templateBindings = makeBindings(mockPageType.Template);
        const templateDialog = installDialogSpies(templateBindings);
        templateBindings.mode.value = mockPageMode.View;
        templateBindings.remove();
        expect(templateDialog.confirm.open).not.toHaveBeenCalled();
    });

    test('cancels add flows according to draft persistence policy', async () => {
        const directBindings = makeBindings();
        directBindings.mode.value = mockPageMode.Edit;
        directBindings.showState.value = true;
        directBindings.cancel();
        expect(directBindings.showState.value).toBe(false);

        const noDraftBindings = makeBindings();
        noDraftBindings.mode.value = mockPageMode.Add;
        noDraftBindings.noTransactionDraft.value = true;
        noDraftBindings.showState.value = true;
        noDraftBindings.cancel();
        expect(noDraftBindings.showState.value).toBe(false);

        const templateBindings = makeBindings(mockPageType.Template);
        templateBindings.mode.value = mockPageMode.Add;
        templateBindings.showState.value = true;
        templateBindings.cancel();
        expect(templateBindings.showState.value).toBe(false);

        const confirmationBindings = makeBindings();
        const confirmationDialog = installDialogSpies(confirmationBindings);
        confirmationBindings.mode.value = mockPageMode.Add;
        confirmationBindings.showState.value = true;
        mockSettingsStore.appSettings.autoSaveTransactionDraft = 'confirmation';
        mockTransactionsStore.isTransactionDraftModified.mockReturnValueOnce(true);
        confirmationDialog.confirm.open.mockResolvedValueOnce(undefined);
        confirmationBindings.cancel();
        await flushPromises();
        expect(mockTransactionsStore.saveTransactionDraft).toHaveBeenCalled();
        expect(confirmationBindings.showState.value).toBe(false);

        confirmationBindings.showState.value = true;
        mockTransactionsStore.isTransactionDraftModified.mockReturnValueOnce(true);
        confirmationDialog.confirm.open.mockRejectedValueOnce(new Error('discard'));
        confirmationBindings.cancel();
        await flushPromises();
        expect(mockTransactionsStore.clearTransactionDraft).toHaveBeenCalled();
        expect(confirmationBindings.showState.value).toBe(false);

        confirmationBindings.showState.value = true;
        mockTransactionsStore.isTransactionDraftModified.mockReturnValueOnce(false);
        confirmationBindings.cancel();
        expect(mockTransactionsStore.clearTransactionDraft).toHaveBeenCalled();

        const enabledBindings = makeBindings();
        enabledBindings.mode.value = mockPageMode.Add;
        enabledBindings.showState.value = true;
        mockSettingsStore.appSettings.autoSaveTransactionDraft = 'enabled';
        enabledBindings.cancel();
        expect(mockTransactionsStore.saveTransactionDraft).toHaveBeenCalled();
        expect(enabledBindings.showState.value).toBe(false);

        const disabledBindings = makeBindings();
        disabledBindings.mode.value = mockPageMode.Add;
        disabledBindings.showState.value = true;
        mockSettingsStore.appSettings.autoSaveTransactionDraft = 'disabled';
        disabledBindings.cancel();
        expect(disabledBindings.showState.value).toBe(false);

        const bypassBindings = makeBindings();
        bypassBindings.mode.value = mockPageMode.Add;
        bypassBindings.addByTemplateId.value = 'template';
        bypassBindings.cancel();
        bypassBindings.addByTemplateId.value = null;
        bypassBindings.duplicateFromId.value = 'bill';
        bypassBindings.cancel();
    });

    test('updates, selects, and clears geolocation states', () => {
        mockGeoLocationSupported = false;
        const unsupported = makeBindings();
        const unsupportedDialog = installDialogSpies(unsupported);
        unsupported.updateGeoLocation(false);
        unsupported.updateGeoLocation(true);
        expect(mockLogger.warn).toHaveBeenCalled();
        expect(unsupportedDialog.snackbar.showMessage).toHaveBeenCalledWith('Unable to retrieve current position');

        mockGeoLocationSupported = true;
        const bindings = makeBindings();
        const { snackbar } = installDialogSpies(bindings);
        const getPosition = navigator.geolocation.getCurrentPosition as ReturnType<typeof jest.fn<(...args: any[]) => void>>;
        getPosition.mockImplementationOnce((success: (position: any) => void) => {
            success({ coords: { latitude: 31.2, longitude: 121.5 } });
        });
        bindings.updateGeoLocation(true);
        expect(bindings.geoLocationStatus.value).toBe(mockGeoStatus.Getting);
        expect(bindings.transaction.value.geoLocation).toStrictEqual({ latitude: 31.2, longitude: 121.5 });

        getPosition.mockImplementationOnce((success: (position: any) => void) => success(null));
        bindings.updateGeoLocation(false);
        expect(bindings.geoLocationStatus.value).toBe(mockGeoStatus.Getting);

        getPosition.mockImplementationOnce((success: (position: any) => void) => success({}));
        bindings.updateGeoLocation(true);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Unable to retrieve current position');

        getPosition.mockImplementationOnce((_success: any, failure: (error: any) => void) => failure(new Error('denied')));
        bindings.updateGeoLocation(true);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Unable to retrieve current position');

        bindings.mode.value = mockPageMode.View;
        bindings.updateSpecifiedGeoLocation({ latitude: 1, longitude: 2 });
        expect(bindings.transaction.value.geoLocation).toStrictEqual({ latitude: 31.2, longitude: 121.5 });

        bindings.mode.value = mockPageMode.Edit;
        bindings.setGeoLocationByClickMap.value = false;
        bindings.updateSpecifiedGeoLocation({ latitude: 2, longitude: 3 });
        expect(bindings.transaction.value.geoLocation).toStrictEqual({ latitude: 31.2, longitude: 121.5 });

        const map = { setMarkerPosition: jest.fn(), initMapView: jest.fn() };
        bindings.map.value = map;
        bindings.setGeoLocationByClickMap.value = true;
        bindings.updateSpecifiedGeoLocation({ latitude: 22.3, longitude: 114.2 });
        expect(bindings.transaction.value.geoLocation).toStrictEqual({ latitude: 22.3, longitude: 114.2 });
        expect(map.setMarkerPosition).toHaveBeenCalledWith(bindings.transaction.value.geoLocation);

        bindings.geoMenuState.value = true;
        bindings.clearGeoLocation();
        expect(bindings.geoMenuState.value).toBe(false);
        expect(bindings.geoLocationStatus.value).toBeNull();
        expect(bindings.transaction.value.geoLocation).toBeNull();
    });

    test('creates tags and treats tag-list refresh as best effort', async () => {
        const bindings = makeBindings();
        const { snackbar } = installDialogSpies(bindings);
        bindings.saveNewTag('Coffee');
        await flushPromises(8);
        expect(mockTagStore.saveTag).toHaveBeenCalledWith({ tag: { id: '', name: 'Coffee' } });
        expect(bindings.transaction.value.tagIds).toContain('tag-new');
        expect(mockTagStore.loadAllTags).toHaveBeenCalledWith({ force: true });

        mockTagStore.loadAllTags.mockRejectedValueOnce(new Error('refresh failed'));
        bindings.saveNewTag('Tea');
        await flushPromises(8);
        expect(mockLogger.warn).toHaveBeenCalledWith('Failed to refresh tags after creating new tag:', expect.any(Error));

        mockTagStore.saveTag.mockResolvedValueOnce(null);
        bindings.saveNewTag('No id');
        await flushPromises();

        mockTagStore.saveTag.mockRejectedValueOnce({ processed: false, message: 'tag failed' });
        bindings.saveNewTag('Failure');
        await flushPromises();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'tag failed' }));

        snackbar.showError.mockClear();
        mockTagStore.saveTag.mockRejectedValueOnce({ processed: true });
        bindings.saveNewTag('Processed failure');
        await flushPromises();
        expect(snackbar.showError).not.toHaveBeenCalled();
        expect(bindings.submitting.value).toBe(false);
    });

    test('applies OCR drafts, candidate fields, and typed recognition errors', async () => {
        const bindings = makeBindings();
        const { snackbar } = installDialogSpies(bindings);
        const candidate = { id: 'amount-low', key: 'amount', field: { value: 12.34 } };
        bindings.receiptDraftCandidateHints.value = [candidate, { id: 'merchant-low', key: 'merchant', field: {} }];

        mockApplyReceiptField.mockReturnValueOnce(false);
        bindings.applyReceiptDraftCandidate(candidate);
        expect(bindings.receiptDraftCandidateHints.value).toHaveLength(2);
        mockApplyReceiptField.mockReturnValueOnce(true);
        bindings.applyReceiptDraftCandidate(candidate);
        expect(bindings.receiptDraftCandidateHints.value).toStrictEqual([
            expect.objectContaining({ id: 'merchant-low' })
        ]);

        mockBuildReceiptHints.mockReturnValueOnce([{ id: 'low-field' }]);
        bindings.applyReceiptRecognitionResult({ draft: { amount: { value: 12.34 } }, confidence: 0.4 });
        expect(mockApplyReceiptAutoFill).toHaveBeenCalledWith(bindings.transaction.value, expect.objectContaining({ confidence: 0.4 }));
        expect(bindings.receiptDraftCandidateHints.value).toStrictEqual([{ id: 'low-field' }]);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Low confidence recognition, please verify');

        snackbar.showMessage.mockClear();
        bindings.applyReceiptRecognitionResult({ draft: {}, confidence: null });
        bindings.applyReceiptRecognitionResult({ draft: {}, confidence: 0.9 });
        expect(snackbar.showMessage).not.toHaveBeenCalled();

        const expectedMessages: Array<[any, string]> = [
            [{ errorCode: 'provider_unconfigured' }, 'OCR recognition requires configuration in Rule Center'],
            [{ errorCode: 'timeout' }, 'Recognition timed out, please try again'],
            [{ errorCode: 'parse_error' }, 'Could not parse this image, please try a clearer one'],
            [{ errorCode: 'rate_limited' }, 'Too many requests, please wait a moment'],
            [{ errorCode: 42 }, 'Unable to recognize image'],
            [null, 'Unable to recognize image']
        ];
        for (const [error, message] of expectedMessages) {
            bindings.showReceiptRecognitionError(error);
            expect(snackbar.showError).toHaveBeenLastCalledWith(message);
        }

        bindings.mode.value = mockPageMode.View;
        expect(bindings.shouldRecognizeUploadedPicture()).toBe(false);
        await bindings.recognizeUploadedPicture({ name: 'receipt.png' });
        expect(mockTransactionsStore.recognizeReceiptImage).not.toHaveBeenCalled();

        bindings.mode.value = mockPageMode.Add;
        expect(bindings.shouldRecognizeUploadedPicture()).toBe(true);
        mockTransactionsStore.recognizeReceiptImage.mockResolvedValueOnce({ draft: {}, confidence: 0.9 });
        await bindings.recognizeUploadedPicture({ name: 'receipt.png' });
        expect(bindings.activeTab.value).toBe('basicInfo');
        expect(snackbar.showMessage).toHaveBeenCalledWith('Image recognized and filled');
        expect(bindings.recognizingPicture.value).toBe(false);

        mockTransactionsStore.recognizeReceiptImage.mockRejectedValueOnce({ errorCode: 'timeout' });
        await bindings.recognizeUploadedPicture({ name: 'receipt.png' });
        expect(snackbar.showError).toHaveBeenCalledWith('Recognition timed out, please try again');

        const templateBindings = makeBindings(mockPageType.Template);
        templateBindings.mode.value = mockPageMode.Add;
        expect(templateBindings.shouldRecognizeUploadedPicture()).toBe(false);
    });

    test('uploads pictures with client sessions and handles upload failures', async () => {
        const bindings = makeBindings();
        const { snackbar } = installDialogSpies(bindings);
        await bindings.uploadPicture(undefined);
        await bindings.uploadPicture({});
        await bindings.uploadPicture({ target: {} });
        await bindings.uploadPicture({ target: { files: [] } });
        expect(mockTransactionsStore.uploadTransactionPicture).not.toHaveBeenCalled();

        const file = { name: 'receipt.png' };
        const input = { files: [file], value: 'C:/fake/receipt.png' };
        bindings.mode.value = mockPageMode.Add;
        bindings.clientSessionId.value = 'upload-session';
        await bindings.uploadPicture({ target: input });
        expect(input.value).toBe('');
        expect(mockTransactionsStore.uploadTransactionPicture).toHaveBeenCalledWith({
            pictureFile: file,
            clientSessionId: 'upload-session'
        });
        expect(bindings.transaction.value.pictures).toContainEqual({ pictureId: 'picture-new' });
        expect(mockTransactionsStore.recognizeReceiptImage).toHaveBeenCalledWith({ imageFile: file });
        expect(bindings.uploadingPicture.value).toBe(false);
        expect(bindings.submitting.value).toBe(false);

        mockTransactionsStore.uploadTransactionPicture.mockRejectedValueOnce({ processed: false });
        await bindings.uploadPicture({ target: { files: [file], value: 'x' } });
        expect(snackbar.showError).toHaveBeenCalledWith('Unable to upload transaction picture');

        snackbar.showError.mockClear();
        mockTransactionsStore.uploadTransactionPicture.mockRejectedValueOnce({ processed: true });
        await bindings.uploadPicture({ target: { files: [file], value: 'x' } });
        expect(snackbar.showError).not.toHaveBeenCalled();

        mockTransactionsStore.uploadTransactionPicture.mockRejectedValueOnce(null);
        await bindings.uploadPicture({ target: { files: [file], value: 'x' } });
        expect(snackbar.showError).toHaveBeenCalledWith('Unable to upload transaction picture');
    });

    test('views and removes pictures across readonly and editable modes', async () => {
        const picture = { pictureId: 'picture-7' };
        const bindings = makeBindings();
        const { confirm, snackbar } = installDialogSpies(bindings);
        bindings.mode.value = mockPageMode.View;
        bindings.viewOrRemovePicture(picture);
        expect(window.open).toHaveBeenCalledWith('/pictures/picture-7', '_blank');

        bindings.mode.value = mockPageMode.Add;
        bindings.transaction.value.pictures = [picture];
        confirm.open.mockResolvedValueOnce(undefined);
        mockTransactionsStore.removeUnusedTransactionPicture.mockResolvedValueOnce(true);
        bindings.viewOrRemovePicture(picture);
        await flushPromises();
        expect(bindings.transaction.value.pictures).toStrictEqual([]);
        expect(bindings.removingPictureId.value).toBe('');

        bindings.transaction.value.pictures = [picture];
        confirm.open.mockResolvedValueOnce(undefined);
        mockTransactionsStore.removeUnusedTransactionPicture.mockResolvedValueOnce(false);
        bindings.viewOrRemovePicture(picture);
        await flushPromises();
        expect(bindings.transaction.value.pictures).toStrictEqual([picture]);

        confirm.open.mockResolvedValueOnce(undefined);
        mockTransactionsStore.removeUnusedTransactionPicture.mockRejectedValueOnce({
            error: { errorCode: 'picture_not_found' }
        });
        bindings.viewOrRemovePicture(picture);
        await flushPromises();
        expect(bindings.transaction.value.pictures).toStrictEqual([]);

        bindings.transaction.value.pictures = [picture];
        confirm.open.mockResolvedValueOnce(undefined);
        mockTransactionsStore.removeUnusedTransactionPicture.mockRejectedValueOnce({ processed: false, message: 'remove failed' });
        bindings.viewOrRemovePicture(picture);
        await flushPromises();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'remove failed' }));

        snackbar.showError.mockClear();
        confirm.open.mockResolvedValueOnce(undefined);
        mockTransactionsStore.removeUnusedTransactionPicture.mockRejectedValueOnce({ processed: true });
        bindings.viewOrRemovePicture(picture);
        await flushPromises();
        expect(snackbar.showError).not.toHaveBeenCalled();
        expect(bindings.submitting.value).toBe(false);
    });

    test('forwards date and matching notices and refreshes readonly matches', async () => {
        const bindings = makeBindings();
        const { snackbar } = installDialogSpies(bindings);
        bindings.onShowDateTimeError('bad date');
        bindings.onBillMatchingNotify('matched');
        bindings.onBillMatchingError('match failed');
        expect(snackbar.showError).toHaveBeenCalledWith('bad date');
        expect(snackbar.showMessage).toHaveBeenCalledWith('matched');
        expect(snackbar.showError).toHaveBeenCalledWith('match failed');

        await bindings.onBillMatchingUpdated();
        bindings.editId.value = 'bill-20';
        bindings.mode.value = mockPageMode.Edit;
        await bindings.onBillMatchingUpdated();
        bindings.mode.value = mockPageMode.View;
        const latest = Object.assign(new MockTransaction(), { id: 'bill-20', editable: false });
        mockTransactionsStore.getTransaction.mockResolvedValueOnce(latest);
        await bindings.onBillMatchingUpdated();
        expect(mockSetTransactionModel).toHaveBeenCalledWith(
            bindings.transaction.value,
            latest,
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            true,
            true
        );
        expect(bindings.originalTransactionEditable.value).toBe(false);
        expect(bindings.loading.value).toBe(false);

        mockTransactionsStore.getTransaction.mockResolvedValueOnce({ id: 'plain-object' });
        await bindings.onBillMatchingUpdated();

        mockTransactionsStore.getTransaction.mockRejectedValueOnce(new Error('refresh failed'));
        await bindings.onBillMatchingUpdated();
        expect(snackbar.showError).toHaveBeenCalledWith('refresh failed');

        mockTransactionsStore.getTransaction.mockRejectedValueOnce('plain failure');
        await bindings.onBillMatchingUpdated();
        expect(snackbar.showError).toHaveBeenCalledWith('Failed to refresh transaction after matching update');
    });

    test('reacts to tabs, type changes, investment cents, keyboard, and geo-menu setters', async () => {
        const bindings = makeBindings();
        const { snackbar } = installDialogSpies(bindings);
        const map = { initMapView: jest.fn(), setMarkerPosition: jest.fn() };
        bindings.map.value = map;
        bindings.activeTab.value = 'map';
        await flushPromises();
        expect(map.initMapView).toHaveBeenCalled();
        bindings.activeTab.value = 'basicInfo';
        await flushPromises();

        bindings.mode.value = mockPageMode.View;
        bindings.geoMenuState.value = true;
        expect(bindings.editableGeoMenuState.value).toBe(false);
        bindings.editableGeoMenuState.value = true;
        expect(bindings.geoMenuState.value).toBe(false);
        bindings.mode.value = mockPageMode.Edit;
        bindings.editableGeoMenuState.value = true;
        expect(bindings.geoMenuState.value).toBe(true);
        bindings.editableGeoMenuState.value = false;
        expect(bindings.geoMenuState.value).toBe(false);

        bindings.showState.value = true;
        bindings.loading.value = false;
        bindings.transaction.value.type = mockTransactionType.Income;
        await flushPromises();
        expect(bindings.transaction.value.expenseCategoryId).toBe('');
        expect(bindings.transaction.value.incomeCategoryId).toBe('');
        expect(bindings.transaction.value.transferCategoryId).toBe('');
        expect(bindings.transaction.value.investmentCategoryId).toBe('');
        expect(mockLogger.info).toHaveBeenCalled();

        bindings.loading.value = true;
        bindings.transaction.value.expenseCategoryId = 'keep';
        bindings.transaction.value.type = mockTransactionType.Expense;
        await flushPromises();
        expect(bindings.transaction.value.expenseCategoryId).toBe('keep');

        bindings.loading.value = false;
        bindings.showState.value = false;
        bindings.transaction.value.expenseCategoryId = 'still-keep';
        bindings.transaction.value.type = mockTransactionType.Transfer;
        await flushPromises();
        expect(bindings.transaction.value.expenseCategoryId).toBe('still-keep');

        bindings.transaction.value.type = mockTransactionType.Investment;
        bindings.transaction.value.sourceAmountCents = 7654;
        await flushPromises();
        expect(bindings.transaction.value.destinationAmountCents).toBe(7654);
        bindings.transaction.value.destinationAmountCents = 8765;
        await flushPromises();
        expect(bindings.transaction.value.sourceAmountCents).toBe(8765);

        bindings.inputEmptyProblemMessage.value = 'blocked save';
        const preventDefault = jest.fn();
        bindings.showState.value = false;
        bindings.onKeydown({ key: 'Enter', target: {}, preventDefault });
        expect(preventDefault).not.toHaveBeenCalled();

        bindings.showState.value = true;
        const Input = globalThis.HTMLInputElement as any;
        const Textarea = globalThis.HTMLTextAreaElement as any;
        bindings.onKeydown({ key: 'Enter', target: new Input(), preventDefault });
        bindings.onKeydown({ key: 'Enter', target: new Textarea(), preventDefault });
        expect(preventDefault).not.toHaveBeenCalled();

        bindings.mode.value = mockPageMode.Add;
        bindings.onKeydown({ key: 'Enter', target: {}, preventDefault });
        expect(snackbar.showMessage).toHaveBeenCalledWith('blocked save');
        expect(preventDefault).toHaveBeenCalled();
        preventDefault.mockClear();
        bindings.mode.value = mockPageMode.View;
        bindings.onKeydown({ key: 'Enter', target: {}, preventDefault });
        expect(preventDefault).not.toHaveBeenCalled();

        bindings.onKeydown({ key: 'Backspace', target: {}, preventDefault });
        expect(preventDefault).toHaveBeenCalled();
        preventDefault.mockClear();

        bindings.showState.value = true;
        bindings.mode.value = mockPageMode.View;
        const confirm = bindings.confirmDialog.value.open as ReturnType<typeof jest.fn<(...args: any[]) => Promise<any>>>;
        confirm.mockResolvedValueOnce(undefined);
        bindings.onKeydown({ key: 'Delete', target: {}, preventDefault });
        expect(preventDefault).toHaveBeenCalled();
        await flushPromises();

        preventDefault.mockClear();
        bindings.showState.value = true;
        bindings.mode.value = mockPageMode.Edit;
        bindings.onKeydown({ key: 'Delete', target: {}, preventDefault });
        bindings.onKeydown({ key: 'Escape', target: {}, preventDefault });
        expect(preventDefault).not.toHaveBeenCalled();
    });

    test('covers promise settlement and template re-wrapping fallbacks', async () => {
        const loaded = Object.assign(new MockTransaction(), { id: 'delete-with-result' });
        mockTransactionsStore.getTransaction.mockResolvedValueOnce(loaded);
        fetchMock().mockResolvedValueOnce(mockResponse({ json: { success: true, result: { candidates: [] } } }));
        const deleteBindings = makeBindings();
        const deleteDialog = installDialogSpies(deleteBindings);
        deleteDialog.confirm.open.mockResolvedValueOnce(undefined);
        const deleteResult = deleteBindings.open({ type: mockTransactionType.Expense, id: loaded.id });
        await flushPromises(8);
        deleteBindings.remove();
        await expect(deleteResult).resolves.toStrictEqual({
            message: 'Transaction has been deleted successfully',
            deleted: true
        });

        const cancelBindings = makeBindings();
        const cancelResult = cancelBindings.open({
            type: mockTransactionType.Expense,
            noTransactionDraft: true
        });
        await flushPromises();
        cancelBindings.cancel();
        await expect(cancelResult).rejects.toBeUndefined();

        const currentTemplate = Object.assign(new MockTransactionTemplate(), {
            id: 'rewrap-template',
            templateType: 1
        });
        const loadedTemplate = Object.assign(new MockTransactionTemplate(), currentTemplate);
        mockTemplateStore.getTemplate.mockResolvedValueOnce(loadedTemplate);
        mockSetTransactionModel.mockImplementationOnce((target: MockTransaction, source: MockTransaction | null) => {
            if (source) {
                Object.assign(target, source);
            }
        }).mockImplementationOnce((target: MockTransaction, source: MockTransaction | null) => {
            if (source) {
                Object.assign(target, source);
                Object.setPrototypeOf(target, MockTransaction.prototype);
                (target as any).fillFrom = MockTransactionTemplate.prototype.fillFrom.bind(target);
            }
        });
        const templateBindings = makeBindings(mockPageType.Template);
        const templateResult = templateBindings.open({
            type: mockTransactionType.Expense,
            id: currentTemplate.id,
            currentTemplate
        });
        await flushPromises();
        expect(templateBindings.transaction.value).toBeInstanceOf(MockTransactionTemplate);
        void templateResult.catch(() => undefined);
    });
});
