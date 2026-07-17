import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import type { ApplicationSettings, LocaleDefaultSettings } from '@/core/setting.ts';

const mockGetApplicationSettings = jest.fn<() => ApplicationSettings>();
const mockGetLocaleDefaultSettings = jest.fn<() => LocaleDefaultSettings>();
const mockUpdateValue = jest.fn<(key: string, value: unknown) => void>();
const mockUpdateSubValue = jest.fn<(key: string, subKey: string, value: unknown) => void>();
const mockClearSettings = jest.fn<() => void>();
const mockNormalizeThemePreference = jest.fn<(value: string) => string>();
const mockHasEnabledCloudSync = jest.fn<(value: Record<string, boolean>) => boolean>();
const mockUpdateCloudValue = jest.fn<(key: string, value: unknown) => void>();
const mockCreateCloudSettings = jest.fn<(keys: string[]) => unknown[]>();
const mockSetCloudSettings = jest.fn<(settings: unknown[]) => void>();
const mockUpdateCloudKeys = jest.fn<(keys?: string[]) => void>();
const mockCreateCloudActions = jest.fn<(runtime: unknown) => {
    updateUserApplicationCloudSettingValue: typeof mockUpdateCloudValue;
    createApplicationCloudSettings: typeof mockCreateCloudSettings;
    setApplicationSettingsFromCloudSettings: typeof mockSetCloudSettings;
    updateApplicationSyncSettingKeys: typeof mockUpdateCloudKeys;
}>(() => ({
    updateUserApplicationCloudSettingValue: mockUpdateCloudValue,
    createApplicationCloudSettings: mockCreateCloudSettings,
    setApplicationSettingsFromCloudSettings: mockSetCloudSettings,
    updateApplicationSyncSettingKeys: mockUpdateCloudKeys
}));

jest.mock('@/lib/settings.ts', () => ({
    __esModule: true,
    getApplicationSettings: () => mockGetApplicationSettings(),
    getLocaleDefaultSettings: () => mockGetLocaleDefaultSettings(),
    updateApplicationSettingsValue: (key: string, value: unknown) => mockUpdateValue(key, value),
    updateApplicationSettingsSubValue: (key: string, subKey: string, value: unknown) => (
        mockUpdateSubValue(key, subKey, value)
    ),
    clearSettings: () => mockClearSettings()
}));

jest.mock('@/core/theme.ts', () => ({
    __esModule: true,
    normalizeThemePreference: (value: string) => mockNormalizeThemePreference(value)
}));

jest.mock('@/stores/setting/cloudSync.ts', () => ({
    __esModule: true,
    hasEnabledApplicationCloudSync: (value: Record<string, boolean>) => mockHasEnabledCloudSync(value),
    createSettingCloudSyncActions: (runtime: unknown) => mockCreateCloudActions(runtime)
}));

import { useSettingsStore } from '@/stores/setting.ts';

function settingsFixture(): ApplicationSettings {
    return {
        theme: 'system',
        fontSize: 14,
        timeZone: 'UTC',
        autoUpdateExchangeRatesData: false,
        showAccountBalance: true,
        swipeBack: true,
        animate: true,
        applicationLock: false,
        applicationLockWebAuthn: false,
        showAddTransactionButtonInDesktopNavbar: true,
        showAmountInHomePage: true,
        timezoneUsedForStatisticsInHomePage: 0,
        overviewAccountFilterInHomePage: {},
        overviewTransactionCategoryFilterInHomePage: {},
        itemsCountInTransactionListPage: 20,
        showTotalAmountInTransactionListPage: true,
        showTagInTransactionListPage: true,
        autoSaveTransactionDraft: 'never',
        autoGetCurrentGeoLocation: false,
        alwaysShowTransactionPicturesInMobileTransactionEditPage: false,
        totalAmountExcludeAccountIds: {},
        currencySortByInExchangeRatesPage: 0,
        statistics: {
            defaultChartDataType: 0,
            defaultTimezoneType: 0,
            defaultAccountFilter: {},
            defaultTransactionCategoryFilter: {},
            defaultSortingType: 0,
            defaultCategoricalChartType: 0,
            defaultCategoricalChartDataRangeType: 0,
            defaultTrendChartType: 0,
            defaultTrendChartDataRangeType: 0,
            defaultAssetTrendsChartType: 0,
            defaultAssetTrendsChartDataRangeType: 0
        }
    } as unknown as ApplicationSettings;
}

function localeFixture(currency = 'CNY', firstDayOfWeek = 1): LocaleDefaultSettings {
    return { currency, firstDayOfWeek } as unknown as LocaleDefaultSettings;
}

describe('settings store behavior', () => {
    beforeEach(() => {
        jest.clearAllMocks();
        setActivePinia(createPinia());
        mockGetApplicationSettings.mockReturnValue(settingsFixture());
        mockGetLocaleDefaultSettings.mockReturnValue(localeFixture());
        mockNormalizeThemePreference.mockReturnValue('dark');
        mockHasEnabledCloudSync.mockImplementation(value => Object.keys(value).length > 0);
        mockCreateCloudSettings.mockReturnValue([{ settingKey: 'theme', settingValue: 'dark' }]);
    });

    test('initializes state and derives cloud sync enablement from the synced key map', () => {
        const store = useSettingsStore();

        expect(store.appSettings.theme).toBe('system');
        expect(store.localeDefaultSettings).toStrictEqual(localeFixture());
        expect(store.enableApplicationCloudSync).toBe(false);

        store.syncedAppSettings = { showAccountBalance: true };
        expect(store.enableApplicationCloudSync).toBe(true);
        expect(mockCreateCloudActions).toHaveBeenCalledTimes(1);
    });

    test('persists root settings and mirrors only cloud-supported user preferences', () => {
        const store = useSettingsStore();
        const accountFilter = { cash: true };
        const categoryFilter = { food: true };
        const excludedAccounts = { archived: true };

        store.setTheme('legacy-dark');
        store.setFontSize(18);
        store.setTimeZone('Asia/Shanghai');
        store.setAutoUpdateExchangeRatesData(true);
        store.setShowAccountBalance(false);
        store.setEnableSwipeBack(false);
        store.setEnableAnimate(false);
        store.setEnableApplicationLock(true);
        store.setEnableApplicationLockWebAuthn(true);
        store.setShowAddTransactionButtonInDesktopNavbar(false);
        store.setShowAmountInHomePage(false);
        store.setTimezoneUsedForStatisticsInHomePage(2);
        store.setOverviewAccountFilterInHomePage(accountFilter);
        store.setOverviewTransactionCategoryFilterInHomePage(categoryFilter);
        store.setItemsCountInTransactionListPage(50);
        store.setShowTotalAmountInTransactionListPage(false);
        store.setShowTagInTransactionListPage(false);
        store.setAutoSaveTransactionDraft('always');
        store.setAutoGetCurrentGeoLocation(true);
        store.setAlwaysShowTransactionPicturesInMobileTransactionEditPage(true);
        store.setTotalAmountExcludeAccountIds(excludedAccounts);
        store.setCurrencySortByInExchangeRatesPage(3);

        expect(mockNormalizeThemePreference).toHaveBeenCalledWith('legacy-dark');
        expect(store.appSettings).toMatchObject({
            theme: 'dark',
            fontSize: 18,
            timeZone: 'Asia/Shanghai',
            autoUpdateExchangeRatesData: true,
            showAccountBalance: false,
            swipeBack: false,
            animate: false,
            applicationLock: true,
            applicationLockWebAuthn: true,
            showAddTransactionButtonInDesktopNavbar: false,
            showAmountInHomePage: false,
            timezoneUsedForStatisticsInHomePage: 2,
            overviewAccountFilterInHomePage: accountFilter,
            overviewTransactionCategoryFilterInHomePage: categoryFilter,
            itemsCountInTransactionListPage: 50,
            showTotalAmountInTransactionListPage: false,
            showTagInTransactionListPage: false,
            autoSaveTransactionDraft: 'always',
            autoGetCurrentGeoLocation: true,
            alwaysShowTransactionPicturesInMobileTransactionEditPage: true,
            totalAmountExcludeAccountIds: excludedAccounts,
            currencySortByInExchangeRatesPage: 3
        });
        expect(mockUpdateValue).toHaveBeenCalledTimes(22);
        expect(mockUpdateValue).toHaveBeenCalledWith('theme', 'dark');
        expect(mockUpdateValue).toHaveBeenCalledWith('totalAmountExcludeAccountIds', excludedAccounts);
        expect(mockUpdateCloudValue.mock.calls.map(call => call[0])).toStrictEqual([
            'showAccountBalance',
            'showAmountInHomePage',
            'timezoneUsedForStatisticsInHomePage',
            'overviewAccountFilterInHomePage',
            'overviewTransactionCategoryFilterInHomePage',
            'itemsCountInTransactionListPage',
            'showTotalAmountInTransactionListPage',
            'showTagInTransactionListPage',
            'autoSaveTransactionDraft',
            'autoGetCurrentGeoLocation',
            'alwaysShowTransactionPicturesInMobileTransactionEditPage',
            'totalAmountExcludeAccountIds',
            'currencySortByInExchangeRatesPage'
        ]);
    });

    test('persists every statistics preference through its nested key contract', () => {
        const store = useSettingsStore();
        const accountFilter = { cash: true };
        const categoryFilter = { salary: true };

        store.setStatisticsDefaultChartDataType(1);
        store.setStatisticsDefaultTimezoneType(2);
        store.setStatisticsDefaultAccountFilter(accountFilter);
        store.setStatisticsDefaultTransactionCategoryFilter(categoryFilter);
        store.setStatisticsSortingType(3);
        store.setStatisticsDefaultCategoricalChartType(4);
        store.setStatisticsDefaultCategoricalChartDateRange(5);
        store.setStatisticsDefaultTrendChartType(6);
        store.setStatisticsDefaultTrendChartDateRange(7);
        store.setStatisticsDefaultAssetTrendsChartType(8);
        store.setStatisticsDefaultAssetTrendsChartDateRange(9);

        expect(store.appSettings.statistics).toStrictEqual({
            defaultChartDataType: 1,
            defaultTimezoneType: 2,
            defaultAccountFilter: accountFilter,
            defaultTransactionCategoryFilter: categoryFilter,
            defaultSortingType: 3,
            defaultCategoricalChartType: 4,
            defaultCategoricalChartDataRangeType: 5,
            defaultTrendChartType: 6,
            defaultTrendChartDataRangeType: 7,
            defaultAssetTrendsChartType: 8,
            defaultAssetTrendsChartDataRangeType: 9
        });
        expect(mockUpdateSubValue.mock.calls).toStrictEqual([
            ['statistics', 'defaultChartDataType', 1],
            ['statistics', 'defaultTimezoneType', 2],
            ['statistics', 'defaultAccountFilter', accountFilter],
            ['statistics', 'defaultTransactionCategoryFilter', categoryFilter],
            ['statistics', 'defaultSortingType', 3],
            ['statistics', 'defaultCategoricalChartType', 4],
            ['statistics', 'defaultCategoricalChartDataRangeType', 5],
            ['statistics', 'defaultTrendChartType', 6],
            ['statistics', 'defaultTrendChartDataRangeType', 7],
            ['statistics', 'defaultAssetTrendsChartType', 8],
            ['statistics', 'defaultAssetTrendsChartDataRangeType', 9]
        ]);
        expect(mockUpdateCloudValue.mock.calls.map(call => call[0])).toStrictEqual([
            'statistics.defaultChartDataType',
            'statistics.defaultTimezoneType',
            'statistics.defaultAccountFilter',
            'statistics.defaultTransactionCategoryFilter',
            'statistics.defaultSortingType',
            'statistics.defaultCategoricalChartType',
            'statistics.defaultCategoricalChartDataRangeType',
            'statistics.defaultTrendChartType',
            'statistics.defaultTrendChartDataRangeType',
            'statistics.defaultAssetTrendsChartType',
            'statistics.defaultAssetTrendsChartDataRangeType'
        ]);
    });

    test('clears persisted settings, reloads state, and ignores an empty locale update', () => {
        const store = useSettingsStore();
        const reloaded = settingsFixture();
        reloaded.theme = 'light';
        mockGetApplicationSettings.mockReturnValue(reloaded);

        store.clearAppSettings();
        store.updateLocalizedDefaultSettings(null);

        expect(mockClearSettings).toHaveBeenCalledTimes(1);
        expect(store.appSettings.theme).toBe('light');
        expect(store.localeDefaultSettings).toStrictEqual(localeFixture());

        store.updateLocalizedDefaultSettings(localeFixture('USD', 0));
        expect(store.localeDefaultSettings).toStrictEqual(localeFixture('USD', 0));
    });

    test('exposes cloud sync actions without altering their payloads', () => {
        const store = useSettingsStore();
        const cloudSettings = [{ settingKey: 'theme', settingValue: 'dark' }];

        expect(store.createApplicationCloudSettings(['theme'])).toStrictEqual(cloudSettings);
        store.setApplicationSettingsFromCloudSettings(cloudSettings);
        store.updateApplicationSyncSettingKeys(['theme']);

        expect(mockCreateCloudSettings).toHaveBeenCalledWith(['theme']);
        expect(mockSetCloudSettings).toHaveBeenCalledWith(cloudSettings);
        expect(mockUpdateCloudKeys).toHaveBeenCalledWith(['theme']);
    });
});
