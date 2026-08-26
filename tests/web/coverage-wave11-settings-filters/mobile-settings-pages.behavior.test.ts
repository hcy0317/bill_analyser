/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockVue = jest.requireActual('vue') as any;
const { proxyRefs, reactive, ref } = mockVue;

const mockShowToast = jest.fn();
const mockLoadAllAccounts = jest.fn<(...args: any[]) => Promise<void>>();
const mockLoadAllCategories = jest.fn<(...args: any[]) => Promise<void>>();
const mockSetAlwaysShowPictures = jest.fn();
const mockSetImportDirectoryName = jest.fn();
const mockChooseImportDirectory = jest.fn<() => Promise<{ name: string }>>();
const mockClearImportDirectory = jest.fn<() => Promise<void>>();
const mockFindNameByValue = jest.fn((items: any[], value: unknown) => (
    items.find(item => item.value === value)?.name ?? ''
));
const mockFindDisplayNameByType = jest.fn((items: any[], value: unknown) => (
    items.find(item => item.type === value)?.displayName ?? ''
));

const mockSettingsStore = reactive({
    appSettings: {
        alwaysShowTransactionPicturesInMobileTransactionEditPage: false,
        billImportDefaultDirectoryName: '',
    },
    setAlwaysShowTransactionPicturesInMobileTransactionEditPage(value: boolean) {
        mockSetAlwaysShowPictures(value);
        this.appSettings.alwaysShowTransactionPicturesInMobileTransactionEditPage = value;
    },
    setBillImportDefaultDirectoryName(value: string) {
        mockSetImportDirectoryName(value);
        this.appSettings.billImportDefaultDirectoryName = value;
    },
});
const mockAccountsStore = {
    loadAllAccounts: (options: any) => mockLoadAllAccounts(options),
};
const mockCategoriesStore = {
    loadAllCategories: (options: any) => mockLoadAllCategories(options),
};

const mockPageBase = {
    loadingAccounts: ref(false),
    loadingTransactionCategories: ref(true),
    hasAnyAccount: ref(true),
    hasAnyVisibleAccount: ref(true),
    hasAnyTransactionCategory: ref(true),
    allTimezoneTypesUsedForStatistics: ref([{ type: 1, displayName: 'Transaction Time' }]),
    allCurrencySortingTypes: ref([{ type: 2, displayName: 'Currency Code' }]),
    allAutoSaveTransactionDraftTypes: ref([{ value: 'enabled', name: 'Enabled' }]),
    showAmountInHomePage: ref(true),
    timezoneUsedForStatisticsInHomePage: ref(1),
    showTotalAmountInTransactionListPage: ref(true),
    showTagInTransactionListPage: ref(false),
    autoSaveTransactionDraft: ref('enabled'),
    isAutoGetCurrentGeoLocation: ref(false),
    currencySortByInExchangeRatesPage: ref(2),
    accountsIncludedInHomePageOverviewDisplayContent: ref('All'),
    accountsIncludedInTotalDisplayContent: ref('All'),
    transactionCategoriesIncludedInHomePageOverviewDisplayContent: ref('All'),
};

function typedOptions(name: string): Array<{ type: number; displayName: string }> {
    return [{ type: 1, displayName: name }];
}

const mockStatisticsBase = {
    allChartDataTypes: ref(typedOptions('Amount')),
    allTimezoneTypesUsedForStatistics: ref(typedOptions('Transaction Time')),
    allSortingTypes: ref(typedOptions('Descending')),
    allCategoricalChartTypes: ref(typedOptions('Pie')),
    allCategoricalChartDateRanges: ref(typedOptions('This Month')),
    allTrendChartDateRanges: ref(typedOptions('Last Year')),
    allAssetTrendsChartDateRanges: ref(typedOptions('All Time')),
    defaultChartDataType: ref(1),
    defaultTimezoneType: ref(1),
    defaultSortingType: ref(1),
    defaultCategoricalChartType: ref(1),
    defaultCategoricalChartDateRange: ref(1),
    defaultTrendChartDateRange: ref(1),
    defaultAssetTrendsChartDateRange: ref(1),
};

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showToast: mockShowToast }),
}));
jest.mock('@/views/base/settings/AppSettingsPageBase.ts', () => ({
    useAppSettingPageBase: () => mockPageBase,
}));
jest.mock('@/views/base/statistics/StatisticsSettingPageBase.ts', () => ({
    useStatisticsSettingPageBase: () => mockStatisticsBase,
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoriesStore,
}));
jest.mock('@/core/category.ts', () => ({ CategoryType: { Income: 1, Expense: 2 } }));
jest.mock('@/lib/common.ts', () => ({
    findNameByValue: (items: any[], value: unknown) => mockFindNameByValue(items, value),
    findDisplayNameByType: (items: any[], value: unknown) => mockFindDisplayNameByType(items, value),
}));
jest.mock('@/lib/importDirectoryPreference.ts', () => ({
    chooseDefaultImportDirectory: () => mockChooseImportDirectory(),
    clearDefaultImportDirectory: () => mockClearImportDirectory(),
}));

const PageSettingsPage = require('@/views/mobile/settings/PageSettingsPage.vue').default as any;
const StatisticsSettingsPage = require('@/views/mobile/statistics/SettingsPage.vue').default as any;

function setup(component: any, props: Record<string, unknown> = {}): any {
    return component.setup(reactive(props), {
        attrs: {},
        slots: {},
        emit: jest.fn(),
        expose: jest.fn(),
    });
}

async function flushPromises(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
}

function collectTemplateCallbacks(vnode: any): Array<{ name: string; callback: (value?: any) => unknown }> {
    const callbacks: Array<{ name: string; callback: (value?: any) => unknown }> = [];
    const seen = new Set<any>();
    const visit = (node: any): void => {
        if (node == null) return;
        if (Array.isArray(node)) {
            node.forEach(visit);
            return;
        }
        if (typeof node !== 'object' || seen.has(node)) return;
        seen.add(node);
        for (const [name, value] of Object.entries(node.props ?? {})) {
            if (name.startsWith('on') && typeof value === 'function') {
                callbacks.push({ name, callback: value as (value?: any) => unknown });
            }
        }
        if (Array.isArray(node.children)) {
            node.children.forEach(visit);
        } else if (node.children && typeof node.children === 'object') {
            for (const slot of Object.values(node.children)) {
                if (typeof slot === 'function') {
                    visit((slot as () => unknown)());
                }
            }
        }
    };
    visit(vnode);
    return callbacks;
}

function render(component: any, bindings: any): Array<{ name: string; callback: (value?: any) => unknown }> {
    const exposed = proxyRefs(bindings);
    const vnode = component.render({}, [], {}, exposed, {}, {});
    return collectTemplateCallbacks(vnode);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockLoadAllAccounts.mockResolvedValue(undefined);
    mockLoadAllCategories.mockResolvedValue(undefined);
    mockSettingsStore.appSettings.alwaysShowTransactionPicturesInMobileTransactionEditPage = false;
    mockSettingsStore.appSettings.billImportDefaultDirectoryName = '';
    mockChooseImportDirectory.mockResolvedValue({ name: '手机账单目录' });
    mockClearImportDirectory.mockResolvedValue(undefined);
    mockPageBase.loadingAccounts.value = false;
    mockPageBase.loadingTransactionCategories.value = true;
    mockPageBase.showAmountInHomePage.value = true;
    mockPageBase.showTotalAmountInTransactionListPage.value = true;
    mockPageBase.showTagInTransactionListPage.value = false;
    mockPageBase.isAutoGetCurrentGeoLocation.value = false;
    jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

describe('mobile PageSettingsPage production-loaded behavior', () => {
    test('loads account and category options and clears both loading states', async () => {
        const bindings = setup(PageSettingsPage);
        expect(mockPageBase.loadingAccounts.value).toBe(true);
        expect(mockLoadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockLoadAllCategories).toHaveBeenCalledWith({ force: false });
        await flushPromises();
        expect(mockPageBase.loadingAccounts.value).toBe(false);
        expect(mockPageBase.loadingTransactionCategories.value).toBe(false);
        expect(mockShowToast).not.toHaveBeenCalled();

        expect(bindings.alwaysShowTransactionPicturesInMobileTransactionEditPage.value).toBe(false);
        bindings.alwaysShowTransactionPicturesInMobileTransactionEditPage.value = true;
        expect(mockSetAlwaysShowPictures).toHaveBeenCalledWith(true);
        expect(bindings.alwaysShowTransactionPicturesInMobileTransactionEditPage.value).toBe(true);
    });

    test('reports unprocessed load failures using message and object fallbacks', async () => {
        const accountError = { processed: false, message: '' };
        mockLoadAllAccounts.mockRejectedValueOnce(accountError);
        mockLoadAllCategories.mockRejectedValueOnce({ processed: false, message: 'category failed' });
        setup(PageSettingsPage);
        await flushPromises();
        expect(mockPageBase.loadingAccounts.value).toBe(false);
        expect(mockPageBase.loadingTransactionCategories.value).toBe(false);
        expect(mockShowToast).toHaveBeenNthCalledWith(1, accountError);
        expect(mockShowToast).toHaveBeenNthCalledWith(2, 'category failed');
    });

    test('updates and clears the default import folder from mobile settings', async () => {
        const bindings = setup(PageSettingsPage);
        await bindings.chooseBillImportDirectory();
        expect(mockSetImportDirectoryName).toHaveBeenLastCalledWith('手机账单目录');
        expect(mockShowToast).toHaveBeenLastCalledWith('Default import folder updated');

        await bindings.resetBillImportDirectory();
        expect(mockClearImportDirectory).toHaveBeenCalledTimes(1);
        expect(mockSetImportDirectoryName).toHaveBeenLastCalledWith('');

        mockChooseImportDirectory.mockRejectedValueOnce(new Error('mobile picker failed'));
        await bindings.chooseBillImportDirectory();
        expect(mockShowToast).toHaveBeenLastCalledWith('mobile picker failed');
    });

    test('leaves processed failures to the caller and exercises popup and toggle bindings', async () => {
        mockLoadAllAccounts.mockRejectedValueOnce({ processed: true, message: 'handled account' });
        mockLoadAllCategories.mockRejectedValueOnce({ processed: true, message: 'handled category' });
        const bindings = setup(PageSettingsPage);
        await flushPromises();
        expect(mockShowToast).not.toHaveBeenCalled();

        const callbacks = render(PageSettingsPage, bindings);
        callbacks.filter(item => item.name === 'onClick').forEach(item => item.callback());
        callbacks.filter(item => item.name === 'onToggle:change').forEach((item, index) => (
            item.callback(index % 2 === 0)
        ));
        callbacks.filter(item => item.name === 'onUpdate:show').forEach(item => item.callback(true));

        expect(bindings.showTimezoneUsedForStatisticsInHomePagePopup.value).toBe(true);
        expect(bindings.showAutoSaveTransactionDraftPopup.value).toBe(true);
        expect(bindings.showCurrencySortByInExchangeRatesPagePopup.value).toBe(true);
        expect(mockFindNameByValue).toHaveBeenCalledWith(
            mockPageBase.allAutoSaveTransactionDraftTypes.value,
            mockPageBase.autoSaveTransactionDraft.value,
        );
        expect(mockFindDisplayNameByType).toHaveBeenCalled();
    });
});

describe('mobile statistics SettingsPage production-loaded behavior', () => {
    test('projects every default and opens all seven selection popups from template events', () => {
        const bindings = setup(StatisticsSettingsPage);
        expect(bindings.allChartDataTypes).toBe(mockStatisticsBase.allChartDataTypes);
        expect(bindings.defaultChartDataType).toBe(mockStatisticsBase.defaultChartDataType);
        expect([
            bindings.showDefaultChartDataTypePopup.value,
            bindings.showDefaultTimezoneTypePopup.value,
            bindings.showDefaultSortingTypePopup.value,
            bindings.showDefaultCategoricalChartTypePopup.value,
            bindings.showDefaultCategoricalChartDateRangePopup.value,
            bindings.showDefaultTrendChartDateRangePopup.value,
            bindings.showDefaultAssetTrendsChartDateRangePopup.value,
        ]).toEqual([false, false, false, false, false, false, false]);

        const callbacks = render(StatisticsSettingsPage, bindings);
        callbacks.filter(item => item.name === 'onClick').forEach(item => item.callback());
        callbacks.filter(item => item.name === 'onUpdate:show').forEach(item => item.callback(true));

        expect([
            bindings.showDefaultChartDataTypePopup.value,
            bindings.showDefaultTimezoneTypePopup.value,
            bindings.showDefaultSortingTypePopup.value,
            bindings.showDefaultCategoricalChartTypePopup.value,
            bindings.showDefaultCategoricalChartDateRangePopup.value,
            bindings.showDefaultTrendChartDateRangePopup.value,
            bindings.showDefaultAssetTrendsChartDateRangePopup.value,
        ]).toEqual([true, true, true, true, true, true, true]);
        expect(mockFindDisplayNameByType).toHaveBeenCalledTimes(7);
    });
});
