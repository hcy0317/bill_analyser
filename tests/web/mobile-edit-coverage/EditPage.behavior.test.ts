import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const transactionType = {
    ModifyBalance: 1,
    Income: 2,
    Expense: 3,
    Transfer: 4,
    Investment: 5
} as const;

const pageType = { Transaction: 'transaction', Template: 'template' } as const;
const pageMode = { Add: 'add', Edit: 'edit', View: 'view' } as const;
const geoStatus = { Getting: 'getting', Success: 'success', Error: 'error' } as const;

class MockTransaction {
    public id = 'transaction-1';
    public type: number = transactionType.Expense;
    public sourceAmountCents = -1_234;
    public destinationAmountCents = 1_234;
    public expenseCategoryId = 'expense-category';
    public incomeCategoryId = 'income-category';
    public transferCategoryId = 'transfer-category';
    public investmentCategoryId = 'investment-category';
    public categoryId = 'expense-category';
    public sourceAccountId = 'wallet';
    public destinationAccountId = 'bank';
    public tagIds: string[] = [];
    public time = 1_800_000_000;
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

    public setGeoLocation(value: { latitude: number; longitude: number } | null): void {
        this.geoLocation = value;
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
    public scheduledFrequency = '';
    public scheduledStartDate = '';
    public scheduledEndDate = '';
    public name = 'Template';

    public static createNewTransactionTemplate(value: MockTransaction): MockTransactionTemplate {
        return Object.assign(new MockTransactionTemplate(), value);
    }

    public fillFrom(value: MockTransactionTemplate): void {
        Object.assign(this, value);
    }
}

const showAlert = jest.fn<(...args: any[]) => void>();
const showConfirm = jest.fn<(...args: any[]) => void>();
const showToast = jest.fn<(...args: any[]) => void>();
const routeBackOnError = jest.fn<(...args: any[]) => void>();
const showLoading = jest.fn<(...args: any[]) => void>();
const hideLoading = jest.fn<(...args: any[]) => void>();
const getCurrentPosition = jest.fn<(
    success: (position: any) => void,
    error?: (reason: any) => void
) => void>();
let presetCategories: unknown[] = [{ id: 'preset' }];
const logger = { debug: jest.fn(), error: jest.fn(), info: jest.fn(), warn: jest.fn() };

const settingsStore = {
    appSettings: {
        autoSaveTransactionDraft: 'disabled',
        autoGetCurrentGeoLocation: false,
        timeZone: 'Asia/Shanghai'
    }
};
const environmentStore = { framework7DarkMode: false };
const userStore = { updateUserTransactionEditScope: jest.fn<(...args: any[]) => Promise<void>>() };
const accountStore = { loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>() };
const categoryStore = {
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>(),
    addPresetCategories: jest.fn<(...args: any[]) => Promise<void>>()
};
const tagStore = { loadAllTags: jest.fn<(...args: any[]) => Promise<void>>() };
const transactionStore = {
    transactionDraft: null as Partial<MockTransaction> | null,
    getTransaction: jest.fn<(...args: any[]) => Promise<MockTransaction | null>>(),
    saveTransaction: jest.fn<(...args: any[]) => Promise<void>>(),
    uploadTransactionPicture: jest.fn<(...args: any[]) => Promise<{ pictureId: string }>>(),
    recognizeReceiptImage: jest.fn<(...args: any[]) => Promise<any>>(),
    removeUnusedTransactionPicture: jest.fn<(...args: any[]) => Promise<boolean>>(),
    isTransactionDraftModified: jest.fn<(...args: any[]) => boolean>(),
    saveTransactionDraft: jest.fn<(...args: any[]) => void>(),
    clearTransactionDraft: jest.fn<(...args: any[]) => void>()
};
const templateStore = {
    allTransactionTemplatesMap: {} as Record<number, Record<string, MockTransactionTemplate>>,
    loadAllTemplates: jest.fn<(...args: any[]) => Promise<void>>(),
    getTemplate: jest.fn<(...args: any[]) => Promise<MockTransactionTemplate | null>>(),
    saveTemplateContent: jest.fn<(...args: any[]) => Promise<void>>()
};

let lastBase: ReturnType<typeof createBase>;
let supportGeoLocation = true;

function createBase(mode: string): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const transaction = ref(new MockTransaction());
    return {
        mode: ref(mode),
        isSupportGeoLocation: supportGeoLocation,
        editId: ref(''),
        addByTemplateId: ref(''),
        duplicateFromId: ref(''),
        clientSessionId: ref(''),
        loading: ref(false),
        submitting: ref(false),
        uploadingPicture: ref(false),
        geoLocationStatus: ref(null),
        setGeoLocationByClickMap: ref(false),
        transaction,
        numeralSystem: ref({ replaceWesternArabicDigitsToLocalizedDigits: (value: string) => value }),
        currentTimezoneOffsetMinutes: ref(480),
        defaultCurrency: ref('CNY'),
        defaultAccountId: ref('wallet'),
        firstDayOfWeek: ref(1),
        coordinateDisplayType: ref('decimal'),
        allTimezones: ref([{ name: 'Asia/Shanghai', displayName: 'Shanghai' }]),
        allVisibleAccounts: ref([]),
        allAccountsMap: ref({ wallet: { id: 'wallet' }, bank: { id: 'bank' } }),
        allVisibleCategorizedAccounts: ref([]),
        allCategories: ref({}),
        allCategoriesMap: ref({}),
        allTags: ref([{ id: 'tag-1', name: 'Daily' }]),
        allTagsMap: ref({}),
        firstVisibleAccountId: ref('wallet'),
        hasAvailableExpenseCategories: ref(true),
        hasAvailableIncomeCategories: ref(true),
        hasAvailableTransferCategories: ref(true),
        hasAvailableInvestmentCategories: ref(true),
        canAddTransactionPicture: computed(() => true),
        title: computed(() => 'Transaction'),
        saveButtonTitle: computed(() => 'Save'),
        sourceAmountTitle: computed(() => 'Amount'),
        sourceAccountTitle: computed(() => 'Account'),
        transferInAmountTitle: computed(() => 'Transfer amount'),
        sourceAccountName: computed(() => 'Wallet'),
        destinationAccountName: computed(() => 'Bank'),
        sourceAccountCurrency: computed(() => 'CNY'),
        destinationAccountCurrency: computed(() => 'CNY'),
        transactionDisplayTimezone: computed(() => 'Asia/Shanghai'),
        transactionTimezoneTimeDifference: computed(() => 0),
        geoLocationStatusInfo: computed(() => ''),
        inputEmptyProblemMessage: ref(''),
        inputIsEmpty: computed(() => false),
        swapTransactionData: jest.fn(),
        getDisplayAmount: (amount: number) => String(amount),
        getTransactionPictureUrl: (picture: { pictureId: string }) => `/pictures/${picture.pictureId}`
    };
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return { ...actual, useTemplateRef: () => actual.ref(null) };
});

jest.mock('@/lib/vue_external_template.ts', () => ({ useExternalTemplateBindings: jest.fn() }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: Record<string, unknown>) => values ? `${key}:${JSON.stringify(values)}` : key,
        getCurrentLanguageTag: () => 'zh-Hans',
        getAllTransactionDefaultCategories: () => presetCategories,
        getMultiMonthdayShortNames: (values: number[]) => values.join('/'),
        getMultiWeekdayLongNames: (values: number[]) => values.join('/'),
        formatUnixTimeToLongDate: (value: number) => `date:${value}`,
        formatUnixTimeToLongTime: (value: number) => `time:${value}`,
        formatGregorianTextualYearMonthDayToLongDate: (value: string) => `day:${value}`
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showAlert, showConfirm, showToast, routeBackOnError }),
    showLoading,
    hideLoading
}));
jest.mock('@/views/base/transactions/TransactionEditPageBase.ts', () => ({
    TransactionEditPageMode: pageMode,
    TransactionEditPageType: pageType,
    GeoLocationStatus: geoStatus,
    useTransactionEditPageBase: (_type: string, mode: string) => {
        lastBase = createBase(mode);
        return lastBase;
    }
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => settingsStore }));
jest.mock('@/stores/environment.ts', () => ({ useEnvironmentsStore: () => environmentStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => userStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => accountStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => categoryStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => tagStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => transactionStore }));
jest.mock('@/stores/transactionTemplate.ts', () => ({ useTransactionTemplatesStore: () => templateStore }));
jest.mock('@/core/category.ts', () => ({ CategoryType: { Expense: 3, Income: 2, Transfer: 4, Investment: 5 } }));
jest.mock('@/core/transaction.ts', () => ({ TransactionType: transactionType, TransactionEditScopeType: { All: { type: 1 } } }));
jest.mock('@/core/template.ts', () => ({
    TemplateType: { Normal: { type: 1 }, Schedule: { type: 2 } },
    ScheduledTemplateFrequencyType: {
        Disabled: { type: -1 },
        Weekly: { type: 1 },
        Monthly: { type: 2 }
    }
}));
jest.mock('@/consts/api.ts', () => ({
    KnownErrorCode: {
        TransactionCannotCreateInThisTime: 'cannot-create',
        TransactionCannotModifyInThisTime: 'cannot-modify',
        TransactionPictureNotFound: 'picture-not-found'
    }
}));
jest.mock('@/consts/file.ts', () => ({ SUPPORTED_IMAGE_EXTENSIONS: ['image/png'] }));
jest.mock('@/consts/transaction.ts', () => ({ TRANSACTION_MAX_AMOUNT: 1_000_000, TRANSACTION_MIN_AMOUNT: -1_000_000 }));
jest.mock('@/models/transaction.ts', () => ({ Transaction: MockTransaction }));
jest.mock('@/models/transaction_template.ts', () => ({ TransactionTemplate: MockTransactionTemplate }));
jest.mock('@/models/large_language_model.ts', () => ({ RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD: 0.6 }));
jest.mock('@/lib/datetime.ts', () => ({
    getActualUnixTimeForStore: (value: number) => value,
    getBrowserTimezoneOffsetMinutes: () => 480,
    getTimezoneOffset: () => '+08:00',
    getTimezoneOffsetMinutes: () => 480
}));
jest.mock('@/lib/common.ts', () => ({ categorizedArrayToPlainArray: (values: unknown[]) => values }));
jest.mock('@/lib/coordinate.ts', () => ({ formatCoordinate: (value: number) => String(value) }));
jest.mock('@/lib/misc.ts', () => ({ generateRandomUUID: () => 'session-uuid' }));
jest.mock('@/lib/category.ts', () => ({
    getTransactionPrimaryCategoryName: () => 'Primary',
    getTransactionSecondaryCategoryName: () => 'Secondary',
    localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories: (values: unknown[]) => values
}));
jest.mock('@/lib/transaction.ts', () => ({
    setTransactionModelByTransaction: (
        target: MockTransaction,
        source: MockTransaction | null,
        ...args: any[]
    ) => {
        if (source) Object.assign(target, source);
        const options = args[6] as { type?: number; sourceAmountCents?: number; destinationAmountCents?: number };
        if (options?.type) target.type = options.type;
        if (options?.sourceAmountCents !== undefined) target.sourceAmountCents = options.sourceAmountCents;
        if (options?.destinationAmountCents !== undefined) target.destinationAmountCents = options.destinationAmountCents;
    }
}));
const applyReceiptField = jest.fn<(...args: any[]) => boolean>();
const applyReceiptAutoFill = jest.fn<(...args: any[]) => void>();
const buildReceiptHints = jest.fn<(...args: any[]) => any[]>();
jest.mock('@/lib/receiptDraft.ts', () => ({
    applyReceiptDraftAutoFillToTransaction: (...args: unknown[]) => applyReceiptAutoFill(...args),
    applyReceiptDraftFieldToTransaction: (...args: unknown[]) => applyReceiptField(...args),
    buildReceiptDraftCandidateHints: (...args: unknown[]) => buildReceiptHints(...args),
    getReceiptDraftCandidateDisplayValue: () => 'candidate'
}));
jest.mock('@/views/mobile/transactions/edit-page/displayHelpers.ts', () => ({
    buildTransactionPictureItems: (values: unknown[]) => values,
    buildTransactionThumbs: (values: unknown[]) => values,
    getFontClassByAmount: (value: number) => value < 0 ? 'negative' : 'positive',
    parseStrictQueryCents: (value?: string) => value ? Number(value) : undefined
}));
jest.mock('@/lib/server_settings.ts', () => ({ getMapProvider: () => 'map', isTransactionPicturesEnabled: () => true }));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: logger }));
jest.mock('@/views/mobile/transactions/components/MobileTransactionPicturesPanel.vue', () => ({
    __esModule: true,
    default: { name: 'MobileTransactionPicturesPanelStub' }
}));

import EditPage from '@/views/mobile/transactions/EditPage.vue';

function setup(path = '/transaction/add', query: Record<string, string> = {}): any {
    const router = { back: jest.fn(), navigate: jest.fn() };
    const bindings = (EditPage as any).setup({ f7route: { path, query }, f7router: router }, { expose: jest.fn() });
    return { bindings, router };
}

async function flush(times = 5): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function render(bindings: any): unknown {
    const { proxyRefs } = jest.requireActual('vue') as any;
    const exposed = proxyRefs(bindings);
    return (EditPage as any).render(exposed, [], {}, exposed, {}, {});
}

beforeEach(() => {
    jest.clearAllMocks();
    settingsStore.appSettings.autoSaveTransactionDraft = 'disabled';
    settingsStore.appSettings.autoGetCurrentGeoLocation = false;
    environmentStore.framework7DarkMode = false;
    supportGeoLocation = true;
    presetCategories = [{ id: 'preset' }];
    transactionStore.transactionDraft = null;
    templateStore.allTransactionTemplatesMap = {};
    accountStore.loadAllAccounts.mockResolvedValue(undefined);
    categoryStore.loadAllCategories.mockResolvedValue(undefined);
    categoryStore.addPresetCategories.mockResolvedValue(undefined);
    tagStore.loadAllTags.mockResolvedValue(undefined);
    templateStore.loadAllTemplates.mockResolvedValue(undefined);
    transactionStore.getTransaction.mockResolvedValue(null);
    transactionStore.saveTransaction.mockResolvedValue(undefined);
    transactionStore.uploadTransactionPicture.mockResolvedValue({ pictureId: 'picture-new' });
    transactionStore.recognizeReceiptImage.mockResolvedValue({ draft: {}, confidence: 0.9 });
    transactionStore.removeUnusedTransactionPicture.mockResolvedValue(true);
    transactionStore.isTransactionDraftModified.mockReturnValue(false);
    templateStore.getTemplate.mockResolvedValue(null);
    templateStore.saveTemplateContent.mockResolvedValue(undefined);
    userStore.updateUserTransactionEditScope.mockResolvedValue(undefined);
    applyReceiptField.mockReturnValue(true);
    buildReceiptHints.mockReturnValue([]);
    showConfirm.mockImplementation((_message: unknown, accept?: () => void) => accept?.());
    showLoading.mockImplementation((predicate?: () => unknown) => { predicate?.(); });
    Object.defineProperty(globalThis, 'navigator', {
        configurable: true,
        value: { geolocation: { getCurrentPosition } }
    });
});

describe('mobile transaction EditPage production behavior', () => {
    test('renders production template branches for add, view, transfer, and scheduled-template states', async () => {
        const added = setup('/transaction/add');
        await flush();
        expect(render(added.bindings)).toBeDefined();

        added.bindings.transaction.value.type = transactionType.Transfer;
        added.bindings.transaction.value.pictures = [{ pictureId: 'picture-1' }];
        expect(render(added.bindings)).toBeDefined();

        const viewed = setup('/transaction/detail');
        await flush();
        viewed.bindings.transaction.value.pictures = [{ pictureId: 'picture-2' }];
        expect(render(viewed.bindings)).toBeDefined();

        const scheduled = setup('/template/add', { templateType: '2' });
        await flush();
        const template = scheduled.bindings.transaction.value as MockTransactionTemplate;
        template.scheduledFrequencyType = 2;
        template.scheduledFrequency = '1,15';
        expect(render(scheduled.bindings)).toBeDefined();
    });

    test('maps route modes and exposes amount, timezone, category, and tag contracts', async () => {
        const { bindings } = setup('/transaction/add');
        await flush();
        expect(bindings.getPageTypeNameMode()).toStrictEqual({ type: pageType.Transaction, mode: pageMode.Add });
        expect(bindings.sourceAmountClass.value).toMatchObject({ 'text-expense': true, negative: true });
        expect(bindings.destinationAmountClass.value).toMatchObject({ positive: true });
        expect(bindings.transactionDisplayDate.value).toBe('date:1800000000');
        expect(bindings.transactionDisplayTime.value).toBe('time:1800000000');
        expect(bindings.transactionDisplayTimezoneName.value).toBe('Shanghai');
        bindings.transaction.value.timeZone = 'Unknown/Zone';
        expect(bindings.transactionDisplayTimezoneName.value).toBe('');
        bindings.transaction.value.timeZone = 'Asia/Shanghai';
        expect(bindings.getTagName('tag-1')).toBe('Daily');
        expect(bindings.getTagName('missing')).toBe('');
        expect([
            transactionType.Expense,
            transactionType.Income,
            transactionType.Transfer,
            transactionType.Investment
        ].map(bindings.hasAvailableCategoriesForType)).toStrictEqual([true, true, true, true]);
        expect(bindings.hasAvailableCategoriesForType(99)).toBe(false);
        expect(bindings.transactionDisplayScheduledFrequency.value).toBe('');
        expect(bindings.transactionDisplayScheduledStartDate.value).toBe('');
        expect(bindings.transactionDisplayScheduledEndDate.value).toBe('');
        expect(bindings.transactionPictures.value).toStrictEqual([]);
        expect(bindings.transactionThumbs.value).toStrictEqual([]);

        lastBase.mode.value = pageMode.View;
        bindings.showDateTimeDialog('date');
        expect(bindings.showTimeInDefaultTimezone.value).toBe(true);
        expect(bindings.transactionDisplayDate.value).toBe('date:1800000000');
        expect(bindings.transactionDisplayTime.value).toContain('UTC+08:00');
        expect(bindings.isDarkMode.value).toBe(false);
        environmentStore.framework7DarkMode = true;
        const dark = setup('/transaction/add');
        expect(dark.bindings.isDarkMode.value).toBe(true);
    });

    test.each([
        ['/transaction/edit', pageType.Transaction, pageMode.Edit],
        ['/transaction/detail', pageType.Transaction, pageMode.View],
        ['/template/add', pageType.Template, pageMode.Add],
        ['/template/edit', pageType.Template, pageMode.Edit]
    ] as const)('maps %s to its page contract', async (path, type, mode) => {
        const { bindings } = setup(path);
        await flush();
        expect(bindings.getPageTypeNameMode()).toStrictEqual({ type, mode });
    });

    test('invalid routes fail closed and valid preset categories open the selector', async () => {
        setup('/invalid');
        expect(showToast).toHaveBeenCalledWith('Parameter Invalid');

        const { bindings } = setup('/transaction/add');
        await flush();
        await bindings.addDefaultCategoriesAndOpenSheet(transactionType.Expense);
        expect(categoryStore.addPresetCategories).toHaveBeenCalled();
        expect(categoryStore.loadAllCategories).toHaveBeenCalledWith({ force: true });
        expect(bindings.showCategorySheet.value).toBe(true);

        bindings.showCategorySheet.value = false;
        bindings.handleCategoryItemClick();
        expect(bindings.showCategorySheet.value).toBe(true);
    });

    test('guards, empty presets, confirmation, and errors keep category creation deterministic', async () => {
        const { bindings } = setup('/transaction/add');
        await flush();
        bindings.addingDefaultCategories.value = true;
        await bindings.addDefaultCategoriesAndOpenSheet(transactionType.Expense);
        expect(categoryStore.addPresetCategories).not.toHaveBeenCalled();

        bindings.addingDefaultCategories.value = false;
        presetCategories = [];
        await bindings.addDefaultCategoriesAndOpenSheet(transactionType.Expense);
        expect(showToast).toHaveBeenCalledWith('No available category');

        presetCategories = [{ id: 'preset' }];
        categoryStore.addPresetCategories.mockRejectedValueOnce({ processed: false, message: 'preset failed' });
        await bindings.addDefaultCategoriesAndOpenSheet(transactionType.Expense);
        expect(showToast).toHaveBeenCalledWith('preset failed');

        categoryStore.addPresetCategories.mockRejectedValueOnce({ processed: true });
        await bindings.addDefaultCategoriesAndOpenSheet(transactionType.Expense);

        lastBase.hasAvailableExpenseCategories.value = false;
        bindings.handleCategoryItemClick();
        expect(showConfirm).toHaveBeenCalledWith(
            'No available category. Add Default Categories?',
            expect.any(Function)
        );

        lastBase.loading.value = true;
        showConfirm.mockClear();
        bindings.handleCategoryItemClick();
        expect(showConfirm).not.toHaveBeenCalled();
    });

    test('formats every scheduled template frequency and date boundary', async () => {
        const { bindings } = setup('/template/add', { templateType: '2' });
        await flush();
        const template = bindings.transaction.value as MockTransactionTemplate;
        expect(bindings.transactionDisplayScheduledFrequency.value).toBe('Disabled');
        expect(bindings.transactionDisplayScheduledStartDate.value).toBe('No limit');
        expect(bindings.transactionDisplayScheduledEndDate.value).toBe('No limit');

        template.scheduledFrequencyType = 1;
        template.scheduledFrequency = '1,3';
        expect(bindings.transactionDisplayScheduledFrequency.value).toContain('1/3');
        template.scheduledFrequency = '';
        expect(bindings.transactionDisplayScheduledFrequency.value).toBe('Weekly');
        template.scheduledFrequencyType = 2;
        template.scheduledFrequency = '2,15';
        expect(bindings.transactionDisplayScheduledFrequency.value).toContain('2/15');
        template.scheduledFrequency = '';
        expect(bindings.transactionDisplayScheduledFrequency.value).toBe('Monthly');
        template.scheduledFrequencyType = 99;
        expect(bindings.transactionDisplayScheduledFrequency.value).toBe('');

        template.scheduledStartDate = '2026-07-01';
        template.scheduledEndDate = '2026-07-31';
        expect(bindings.transactionDisplayScheduledStartDate.value).toBe('day:2026-07-01');
        expect(bindings.transactionDisplayScheduledEndDate.value).toBe('day:2026-07-31');
    });

    test('hydrates edit, duplicate, template, template-derived, and saved-draft sources', async () => {
        const loaded = Object.assign(new MockTransaction(), {
            id: 'loaded',
            geoLocation: { latitude: 30, longitude: 120 }
        });
        transactionStore.getTransaction.mockResolvedValueOnce(loaded);
        const edit = setup('/transaction/edit', { id: 'loaded', withTime: 'true', withGeoLocation: 'true' });
        await flush();
        expect(edit.bindings.editId.value).toBe('loaded');
        expect(edit.bindings.transaction.value.geoLocation).toStrictEqual({ latitude: 30, longitude: 120 });

        transactionStore.getTransaction.mockResolvedValueOnce(loaded);
        const duplicate = setup('/transaction/add', { id: 'loaded' });
        await flush();
        expect(duplicate.bindings.duplicateFromId.value).toBe('loaded');

        transactionStore.getTransaction.mockResolvedValueOnce(loaded);
        const viewedById = setup('/transaction/detail', { id: 'loaded' });
        await flush();
        expect(viewedById.bindings.editId.value).toBe('');

        const sourceTemplate = Object.assign(new MockTransactionTemplate(), { id: 'template-source' });
        templateStore.allTransactionTemplatesMap = { 1: { 'template-source': sourceTemplate } };
        const byTemplate = setup('/transaction/add', { templateId: 'template-source' });
        await flush();
        expect(byTemplate.bindings.addByTemplateId.value).toBe('template-source');

        settingsStore.appSettings.autoSaveTransactionDraft = 'enabled';
        transactionStore.transactionDraft = { sourceAmountCents: -9_900 };
        const fromDraft = setup('/transaction/add');
        await flush();
        expect(fromDraft.bindings.loading.value).toBe(false);

        templateStore.getTemplate.mockResolvedValueOnce(sourceTemplate);
        const templateEdit = setup('/template/edit', { id: 'template-source', templateType: '2' });
        await flush();
        expect(templateEdit.bindings.editId.value).toBe('template-source');
        expect(templateEdit.bindings.transaction.value.id).toBe('template-source');

        templateStore.getTemplate.mockResolvedValueOnce(sourceTemplate);
        const templateAddById = setup('/template/add', { id: 'template-source' });
        await flush();
        expect(templateAddById.bindings.editId.value).toBe('');

        templateStore.allTransactionTemplatesMap = { 1: {} };
        const missingTemplateSource = setup('/transaction/add', { templateId: 'missing', time: '1800000001' });
        await flush();
        expect(missingTemplateSource.bindings.addByTemplateId.value).toBe('');

        const typed = setup('/transaction/add', { type: String(transactionType.Investment) });
        await flush();
        expect(typed.bindings.transaction.value.type).toBe(transactionType.Investment);
        const balance = setup('/transaction/detail', { type: String(transactionType.ModifyBalance) });
        await flush();
        expect(balance.bindings.transaction.value.type).toBe(transactionType.ModifyBalance);
    });

    test('reports missing and failed init dependencies without hanging loading state', async () => {
        transactionStore.getTransaction.mockResolvedValueOnce(null);
        setup('/transaction/edit', { id: 'missing' });
        await flush();
        expect(showToast).toHaveBeenCalledWith('Unable to retrieve transaction');

        templateStore.getTemplate.mockResolvedValueOnce(null);
        setup('/template/edit', { id: 'missing' });
        await flush();
        expect(showToast).toHaveBeenCalledWith('Unable to retrieve template');

        accountStore.loadAllAccounts.mockRejectedValueOnce({ processed: true });
        const processed = setup('/transaction/add');
        await flush();
        expect(processed.bindings.loading.value).toBe(false);

        accountStore.loadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'load failed' });
        const failed = setup('/transaction/add');
        await flush();
        expect(failed.bindings.loadingError.value).toMatchObject({ message: 'load failed' });
        expect(showToast).toHaveBeenCalledWith('load failed');
    });

    test('saves transaction amounts in cents and clears an eligible add draft', async () => {
        const { bindings, router } = setup('/transaction/add');
        await flush();
        bindings.transaction.value.sourceAmountCents = -12_345;
        bindings.save();
        await flush();
        expect(transactionStore.saveTransaction).toHaveBeenCalledWith(expect.objectContaining({
            transaction: expect.objectContaining({ sourceAmountCents: -12_345 }),
            defaultCurrency: 'CNY',
            isEdit: false,
            clientSessionId: 'session-uuid'
        }));
        expect(transactionStore.clearTransactionDraft).toHaveBeenCalled();
        expect(router.back).toHaveBeenCalled();

        const edited = setup('/transaction/edit');
        await flush();
        edited.bindings.save();
        await flush();
        expect(showToast).toHaveBeenCalledWith('You have saved this transaction');
    });

    test('guards invalid save input and confirms zero-cent transactions', async () => {
        const { bindings } = setup('/transaction/add');
        await flush();
        lastBase.inputEmptyProblemMessage.value = 'Missing account';
        bindings.save();
        expect(showAlert).toHaveBeenCalledWith('Missing account');

        lastBase.inputEmptyProblemMessage.value = '';
        bindings.transaction.value.sourceAmountCents = 0;
        bindings.save();
        await flush();
        expect(showConfirm).toHaveBeenCalledWith(
            'Are you sure you want to save this transaction with a zero amount?',
            expect.any(Function)
        );
        expect(transactionStore.saveTransaction).toHaveBeenCalled();
    });

    test('saves template add/edit modes and reports template persistence errors', async () => {
        const added = setup('/template/add');
        await flush();
        added.bindings.save();
        await flush();
        expect(templateStore.saveTemplateContent).toHaveBeenCalledWith(expect.objectContaining({ isEdit: false }));
        expect(added.router.back).toHaveBeenCalled();

        const edited = setup('/template/edit');
        await flush();
        edited.bindings.save();
        await flush();
        expect(templateStore.saveTemplateContent).toHaveBeenLastCalledWith(expect.objectContaining({ isEdit: true }));

        templateStore.saveTemplateContent.mockRejectedValueOnce({ processed: false, message: 'template failed' });
        const failed = setup('/template/add');
        await flush();
        failed.bindings.save();
        await flush();
        expect(showToast).toHaveBeenCalledWith('template failed');
    });

    test('handles editable-range and generic transaction save failures', async () => {
        transactionStore.saveTransaction.mockRejectedValueOnce({
            error: { errorCode: 'cannot-create' },
            processed: true
        });
        const restricted = setup('/transaction/add');
        await flush();
        restricted.bindings.save();
        await flush(8);
        expect(showConfirm).toHaveBeenCalledWith(expect.stringContaining('editable transaction range'), expect.any(Function));
        expect(userStore.updateUserTransactionEditScope).toHaveBeenCalledWith({ transactionEditScope: 1 });

        transactionStore.saveTransaction.mockRejectedValueOnce({ processed: false, message: 'save failed' });
        const generic = setup('/transaction/edit');
        await flush();
        generic.bindings.save();
        await flush();
        expect(showToast).toHaveBeenCalledWith('save failed');

        transactionStore.saveTransaction.mockRejectedValueOnce({
            error: { errorCode: 'cannot-modify' },
            processed: true
        });
        userStore.updateUserTransactionEditScope.mockRejectedValueOnce({ processed: false, message: 'scope failed' });
        const scopeFailure = setup('/transaction/edit');
        await flush();
        scopeFailure.bindings.save();
        await flush(8);
        expect(showToast).toHaveBeenCalledWith('scope failed');

        transactionStore.saveTransaction.mockRejectedValueOnce({ processed: true, message: 'already handled' });
        const handled = setup('/transaction/edit');
        await flush();
        handled.bindings.save();
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('already handled');
    });

    test('updates and clears geolocation through success and unsupported paths', () => {
        const { bindings } = setup('/transaction/add');
        getCurrentPosition.mockImplementationOnce((success: (position: any) => void) => success({
            coords: { latitude: 31.2, longitude: 121.5 }
        }));
        bindings.updateGeoLocation(true);
        expect(bindings.transaction.value.geoLocation).toStrictEqual({ latitude: 31.2, longitude: 121.5 });

        bindings.clearGeoLocation();
        expect(bindings.transaction.value.geoLocation).toBeNull();
        expect(bindings.geoLocationStatus.value).toBeNull();

        supportGeoLocation = false;
        const { bindings: unsupportedBindings } = setup('/transaction/add');
        unsupportedBindings.updateGeoLocation(true);
        expect(showToast).toHaveBeenCalledWith('Unable to retrieve current position');
    });

    test('handles empty and failed geolocation callbacks and editable date sheets', () => {
        const { bindings } = setup('/transaction/edit');
        getCurrentPosition.mockImplementationOnce((success) => success(null));
        bindings.updateGeoLocation(true);
        expect(bindings.geoLocationStatus.value).toBe(geoStatus.Getting);
        expect(showToast).toHaveBeenCalledWith('Unable to retrieve current position');

        getCurrentPosition.mockImplementationOnce((_success, error) => error?.(new Error('denied')));
        bindings.updateGeoLocation(true);
        expect(showToast).toHaveBeenCalledWith('Unable to retrieve current position');

        bindings.showDateTimeDialog('date');
        expect(bindings.transactionDateTimeSheetMode.value).toBe('date');
        expect(bindings.showTransactionDateTimeSheet.value).toBe(true);
    });

    test('applies OCR candidates, handles known errors, and uploads pictures', async () => {
        buildReceiptHints.mockReturnValue([{ id: 'candidate-1', key: 'amount', field: 'sourceAmountCents' }]);
        const { bindings } = setup('/transaction/add');
        await flush();
        bindings.applyReceiptRecognitionResult({ draft: {}, confidence: 0.5 });
        expect(applyReceiptAutoFill).toHaveBeenCalled();
        expect(showToast).toHaveBeenCalledWith('Low confidence recognition, please verify');

        for (const [errorCode, message] of [
            ['provider_unconfigured', 'OCR recognition requires configuration in Rule Center'],
            ['timeout', 'Recognition timed out, please try again'],
            ['parse_error', 'Could not parse this image, please try a clearer one'],
            ['rate_limited', 'Too many requests, please wait a moment'],
            ['unknown', 'Unable to recognize image']
        ] as const) {
            bindings.showReceiptRecognitionError({ errorCode });
            expect(showToast).toHaveBeenCalledWith(message);
        }

        const pictureFile = { name: 'receipt.png' } as File;
        const input = { files: [pictureFile], value: 'receipt.png' };
        await bindings.uploadPicture({ target: input });
        expect(transactionStore.uploadTransactionPicture).toHaveBeenCalledWith({
            pictureFile,
            clientSessionId: 'session-uuid'
        });
        expect(transactionStore.recognizeReceiptImage).toHaveBeenCalledWith({ imageFile: pictureFile });
        expect(bindings.transaction.value.pictures).toStrictEqual([{ pictureId: 'picture-new' }]);
        expect(input.value).toBe('');
    });

    test('covers candidate rejection, recognition failure, upload guards, and upload errors', async () => {
        const { bindings } = setup('/transaction/add');
        await flush();
        bindings.receiptDraftCandidateHints.value = [{ id: 'keep', key: 'amount', field: 'sourceAmountCents' }];
        applyReceiptField.mockReturnValueOnce(false);
        bindings.applyReceiptDraftCandidate(bindings.receiptDraftCandidateHints.value[0]);
        expect(bindings.receiptDraftCandidateHints.value).toHaveLength(1);
        applyReceiptField.mockReturnValueOnce(true);
        bindings.applyReceiptDraftCandidate(bindings.receiptDraftCandidateHints.value[0]);
        expect(bindings.receiptDraftCandidateHints.value).toHaveLength(0);

        transactionStore.recognizeReceiptImage.mockRejectedValueOnce({ errorCode: 'timeout' });
        await bindings.recognizeUploadedPicture({ name: 'receipt.png' } as File);
        expect(showToast).toHaveBeenCalledWith('Recognition timed out, please try again');

        await bindings.uploadPicture({ target: null });
        await bindings.uploadPicture({ target: { files: [] } });
        expect(transactionStore.uploadTransactionPicture).not.toHaveBeenCalled();

        transactionStore.uploadTransactionPicture.mockRejectedValueOnce({ processed: false, message: 'upload failed' });
        await bindings.uploadPicture({ target: { files: [{ name: 'bad.png' }], value: 'bad.png' } });
        expect(showToast).toHaveBeenCalledWith('upload failed');

        const template = setup('/template/add');
        await flush();
        expect(template.bindings.shouldRecognizeUploadedPicture()).toBe(false);
        await template.bindings.recognizeUploadedPicture({ name: 'template.png' } as File);

        const click = jest.fn();
        bindings.pictureInput.value = { click };
        bindings.showOpenPictureDialog();
        expect(click).toHaveBeenCalled();
        bindings.submitting.value = true;
        click.mockClear();
        bindings.showOpenPictureDialog();
        expect(click).not.toHaveBeenCalled();
    });

    test('removes pictures, duplicates routes, and applies draft policy', async () => {
        const { bindings, router } = setup('/transaction/add', { sourceAmountCents: '100' });
        await flush();
        const picture = { pictureId: 'picture-1' };
        bindings.transaction.value.pictures = [picture];
        bindings.viewOrRemovePicture(picture);
        await flush();
        expect(transactionStore.removeUnusedTransactionPicture).toHaveBeenCalledWith({ pictureInfo: picture });
        expect(bindings.transaction.value.pictures).toStrictEqual([]);

        bindings.duplicate(true, false);
        expect(router.navigate).toHaveBeenCalledWith(expect.stringContaining('withTime=true&withGeoLocation=false'));
        bindings.duplicate();
        expect(router.navigate).toHaveBeenLastCalledWith(expect.stringContaining('withTime=false&withGeoLocation=false'));

        settingsStore.appSettings.autoSaveTransactionDraft = 'enabled';
        bindings.onPageBeforeOut();
        expect(transactionStore.saveTransactionDraft).toHaveBeenCalledWith(
            bindings.transaction.value,
            100,
            undefined,
            undefined,
            undefined,
            'wallet'
        );
    });

    test('covers view-picture, remove errors, page-enter geolocation, and confirmation draft choices', async () => {
        const view = setup('/transaction/detail');
        await flush();
        view.bindings.save();
        const picture = { pictureId: 'view-picture' };
        view.bindings.transaction.value.pictures = [picture];
        view.bindings.pictureBrowser.value = { open: jest.fn() };
        view.bindings.viewOrRemovePicture(picture);
        expect(view.bindings.pictureBrowser.value.open).toHaveBeenCalled();

        transactionStore.removeUnusedTransactionPicture.mockRejectedValueOnce({
            error: { errorCode: 'picture-not-found' },
            processed: true
        });
        const removable = setup('/transaction/edit');
        await flush();
        removable.bindings.transaction.value.pictures = [picture];
        removable.bindings.viewOrRemovePicture(picture);
        await flush();
        expect(removable.bindings.transaction.value.pictures).toStrictEqual([]);

        transactionStore.removeUnusedTransactionPicture.mockRejectedValueOnce({
            error: null,
            processed: false,
            message: 'remove failed'
        });
        const removeFailure = setup('/transaction/edit');
        await flush();
        removeFailure.bindings.transaction.value.pictures = [picture];
        removeFailure.bindings.viewOrRemovePicture(picture);
        await flush();
        expect(showToast).toHaveBeenCalledWith('remove failed');

        transactionStore.removeUnusedTransactionPicture.mockResolvedValueOnce(false);
        const retained = setup('/transaction/edit');
        await flush();
        retained.bindings.transaction.value.pictures = [picture];
        retained.bindings.viewOrRemovePicture(picture);
        await flush();
        expect(retained.bindings.transaction.value.pictures).toStrictEqual([picture]);

        settingsStore.appSettings.autoGetCurrentGeoLocation = true;
        const entering = setup('/transaction/add');
        await flush();
        entering.bindings.onPageAfterIn();
        expect(routeBackOnError).toHaveBeenCalled();
        expect(getCurrentPosition).toHaveBeenCalled();

        settingsStore.appSettings.autoSaveTransactionDraft = 'confirmation';
        transactionStore.isTransactionDraftModified.mockReturnValueOnce(true);
        const confirmation = setup('/transaction/add');
        await flush();
        confirmation.bindings.onPageBeforeOut();
        const confirmationCall = showConfirm.mock.calls.find(call => call[0] === 'Do you want to save this transaction draft?');
        expect(confirmationCall).toBeDefined();
        confirmationCall?.[2]?.();
        expect(transactionStore.clearTransactionDraft).toHaveBeenCalled();

        const skipped = setup('/transaction/detail');
        await flush();
        transactionStore.saveTransactionDraft.mockClear();
        skipped.bindings.onPageBeforeOut();
        expect(transactionStore.saveTransactionDraft).not.toHaveBeenCalled();

        transactionStore.isTransactionDraftModified.mockReturnValueOnce(false);
        const unchanged = setup('/transaction/add');
        await flush();
        unchanged.bindings.onPageBeforeOut();
        expect(transactionStore.clearTransactionDraft).toHaveBeenCalled();
    });

    test('resets category ids when the transaction type changes', async () => {
        const { nextTick } = jest.requireActual('vue') as any;
        const { bindings } = setup('/transaction/add');
        await flush();
        bindings.transaction.value.type = transactionType.Income;
        await nextTick();
        expect(bindings.transaction.value).toMatchObject({
            expenseCategoryId: '',
            incomeCategoryId: '',
            transferCategoryId: '',
            investmentCategoryId: ''
        });
        expect(logger.info).toHaveBeenCalled();
    });
});
