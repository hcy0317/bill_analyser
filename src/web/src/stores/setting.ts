import { ref, computed } from 'vue';
import { defineStore } from 'pinia';

import {
    type ApplicationSettings,
    type LocaleDefaultSettings,
} from '@/core/setting.ts';
import { normalizeThemePreference } from '@/core/theme.ts';

import {
    getApplicationSettings,
    getLocaleDefaultSettings,
    updateApplicationSettingsValue,
    updateApplicationSettingsSubValue,
    clearSettings
} from '@/lib/settings.ts';

import {
    createSettingCloudSyncActions,
    hasEnabledApplicationCloudSync
} from './setting/cloudSync.ts';

export const useSettingsStore = defineStore('settings', () => {
    const appSettings = ref<ApplicationSettings>(getApplicationSettings());
    const syncedAppSettings = ref<Record<string, boolean>>({});
    const localeDefaultSettings = ref<LocaleDefaultSettings>(getLocaleDefaultSettings());

    const enableApplicationCloudSync = computed<boolean>(() => hasEnabledApplicationCloudSync(syncedAppSettings.value));
    const {
        updateUserApplicationCloudSettingValue,
        createApplicationCloudSettings,
        setApplicationSettingsFromCloudSettings,
        updateApplicationSyncSettingKeys
    } = createSettingCloudSyncActions({ appSettings, syncedAppSettings });

    // 基础设置
    function setTheme(value: string): void {
        const normalizedTheme = normalizeThemePreference(value);

        updateApplicationSettingsValue('theme', normalizedTheme);
        appSettings.value.theme = normalizedTheme;
    }

    function setFontSize(value: number): void {
        updateApplicationSettingsValue('fontSize', value);
        appSettings.value.fontSize = value;
    }

    function setTimeZone(value: string): void {
        updateApplicationSettingsValue('timeZone', value);
        appSettings.value.timeZone = value;
    }

    function setAutoUpdateExchangeRatesData(value: boolean): void {
        updateApplicationSettingsValue('autoUpdateExchangeRatesData', value);
        appSettings.value.autoUpdateExchangeRatesData = value;
    }

    function setShowAccountBalance(value: boolean): void {
        updateApplicationSettingsValue('showAccountBalance', value);
        appSettings.value.showAccountBalance = value;
        updateUserApplicationCloudSettingValue('showAccountBalance', value);
    }

    function setEnableSwipeBack(value: boolean): void {
        updateApplicationSettingsValue('swipeBack', value);
        appSettings.value.swipeBack = value;
    }

    function setEnableAnimate(value: boolean): void {
        updateApplicationSettingsValue('animate', value);
        appSettings.value.animate = value;
    }

    // 应用锁
    function setEnableApplicationLock(value: boolean): void {
        updateApplicationSettingsValue('applicationLock', value);
        appSettings.value.applicationLock = value;
    }

    function setEnableApplicationLockWebAuthn(value: boolean): void {
        updateApplicationSettingsValue('applicationLockWebAuthn', value);
        appSettings.value.applicationLockWebAuthn = value;
    }

    // 导航栏
    function setShowAddTransactionButtonInDesktopNavbar(value: boolean): void {
        updateApplicationSettingsValue('showAddTransactionButtonInDesktopNavbar', value);
        appSettings.value.showAddTransactionButtonInDesktopNavbar = value;
    }

    // 概览页
    function setShowAmountInHomePage(value: boolean): void {
        updateApplicationSettingsValue('showAmountInHomePage', value);
        appSettings.value.showAmountInHomePage = value;
        updateUserApplicationCloudSettingValue('showAmountInHomePage', value);
    }

    function setTimezoneUsedForStatisticsInHomePage(value: number): void {
        updateApplicationSettingsValue('timezoneUsedForStatisticsInHomePage', value);
        appSettings.value.timezoneUsedForStatisticsInHomePage = value;
        updateUserApplicationCloudSettingValue('timezoneUsedForStatisticsInHomePage', value);
    }

    function setOverviewAccountFilterInHomePage(value: Record<string, boolean>): void {
        updateApplicationSettingsValue('overviewAccountFilterInHomePage', value);
        appSettings.value.overviewAccountFilterInHomePage = value;
        updateUserApplicationCloudSettingValue('overviewAccountFilterInHomePage', value);
    }

    function setOverviewTransactionCategoryFilterInHomePage(value: Record<string, boolean>): void {
        updateApplicationSettingsValue('overviewTransactionCategoryFilterInHomePage', value);
        appSettings.value.overviewTransactionCategoryFilterInHomePage = value;
        updateUserApplicationCloudSettingValue('overviewTransactionCategoryFilterInHomePage', value);
    }

    // 交易列表页
    function setItemsCountInTransactionListPage(value: number): void {
        updateApplicationSettingsValue('itemsCountInTransactionListPage', value);
        appSettings.value.itemsCountInTransactionListPage = value;
        updateUserApplicationCloudSettingValue('itemsCountInTransactionListPage', value);
    }

    function setShowTotalAmountInTransactionListPage(value: boolean): void {
        updateApplicationSettingsValue('showTotalAmountInTransactionListPage', value);
        appSettings.value.showTotalAmountInTransactionListPage = value;
        updateUserApplicationCloudSettingValue('showTotalAmountInTransactionListPage', value);
    }

    function setShowTagInTransactionListPage(value: boolean): void {
        updateApplicationSettingsValue('showTagInTransactionListPage', value);
        appSettings.value.showTagInTransactionListPage = value;
        updateUserApplicationCloudSettingValue('showTagInTransactionListPage', value);
    }

    // 交易编辑页
    function setAutoSaveTransactionDraft(value: string): void {
        updateApplicationSettingsValue('autoSaveTransactionDraft', value);
        appSettings.value.autoSaveTransactionDraft = value;
        updateUserApplicationCloudSettingValue('autoSaveTransactionDraft', value);
    }

    function setAutoGetCurrentGeoLocation(value: boolean): void {
        updateApplicationSettingsValue('autoGetCurrentGeoLocation', value);
        appSettings.value.autoGetCurrentGeoLocation = value;
        updateUserApplicationCloudSettingValue('autoGetCurrentGeoLocation', value);
    }

    function setAlwaysShowTransactionPicturesInMobileTransactionEditPage(value: boolean): void {
        updateApplicationSettingsValue('alwaysShowTransactionPicturesInMobileTransactionEditPage', value);
        appSettings.value.alwaysShowTransactionPicturesInMobileTransactionEditPage = value;
        updateUserApplicationCloudSettingValue('alwaysShowTransactionPicturesInMobileTransactionEditPage', value);
    }

    function setBillImportDefaultDirectoryName(value: string): void {
        updateApplicationSettingsValue('billImportDefaultDirectoryName', value);
        appSettings.value.billImportDefaultDirectoryName = value;
    }

    // 账户列表页
    function setTotalAmountExcludeAccountIds(value: Record<string, boolean>): void {
        updateApplicationSettingsValue('totalAmountExcludeAccountIds', value);
        appSettings.value.totalAmountExcludeAccountIds = value;
        updateUserApplicationCloudSettingValue('totalAmountExcludeAccountIds', value);
    }

    // 汇率数据页
    function setCurrencySortByInExchangeRatesPage(value: number): void {
        updateApplicationSettingsValue('currencySortByInExchangeRatesPage', value);
        appSettings.value.currencySortByInExchangeRatesPage = value;
        updateUserApplicationCloudSettingValue('currencySortByInExchangeRatesPage', value);
    }

    // 统计设置
    function setStatisticsDefaultChartDataType(value: number): void {
        updateApplicationSettingsSubValue('statistics', 'defaultChartDataType', value);
        appSettings.value.statistics.defaultChartDataType = value;
        updateUserApplicationCloudSettingValue('statistics.defaultChartDataType', value);
    }

    function setStatisticsDefaultTimezoneType(value: number): void {
        updateApplicationSettingsSubValue('statistics', 'defaultTimezoneType', value);
        appSettings.value.statistics.defaultTimezoneType = value;
        updateUserApplicationCloudSettingValue('statistics.defaultTimezoneType', value);
    }

    function setStatisticsDefaultAccountFilter(value: Record<string, boolean>): void {
        updateApplicationSettingsSubValue('statistics', 'defaultAccountFilter', value);
        appSettings.value.statistics.defaultAccountFilter = value;
        updateUserApplicationCloudSettingValue('statistics.defaultAccountFilter', value);
    }

    function setStatisticsDefaultTransactionCategoryFilter(value: Record<string, boolean>): void {
        updateApplicationSettingsSubValue('statistics', 'defaultTransactionCategoryFilter', value);
        appSettings.value.statistics.defaultTransactionCategoryFilter = value;
        updateUserApplicationCloudSettingValue('statistics.defaultTransactionCategoryFilter', value);
    }

    function setStatisticsSortingType(value: number): void {
        updateApplicationSettingsSubValue('statistics', 'defaultSortingType', value);
        appSettings.value.statistics.defaultSortingType = value;
        updateUserApplicationCloudSettingValue('statistics.defaultSortingType', value);
    }

    function setStatisticsDefaultCategoricalChartType(value: number): void {
        updateApplicationSettingsSubValue('statistics', 'defaultCategoricalChartType', value);
        appSettings.value.statistics.defaultCategoricalChartType = value;
        updateUserApplicationCloudSettingValue('statistics.defaultCategoricalChartType', value);
    }

    function setStatisticsDefaultCategoricalChartDateRange(value: number): void {
        updateApplicationSettingsSubValue('statistics', 'defaultCategoricalChartDataRangeType', value);
        appSettings.value.statistics.defaultCategoricalChartDataRangeType = value;
        updateUserApplicationCloudSettingValue('statistics.defaultCategoricalChartDataRangeType', value);
    }

    function setStatisticsDefaultTrendChartType(value: number): void {
        updateApplicationSettingsSubValue('statistics', 'defaultTrendChartType', value);
        appSettings.value.statistics.defaultTrendChartType = value;
        updateUserApplicationCloudSettingValue('statistics.defaultTrendChartType', value);
    }

    function setStatisticsDefaultTrendChartDateRange(value: number): void {
        updateApplicationSettingsSubValue('statistics', 'defaultTrendChartDataRangeType', value);
        appSettings.value.statistics.defaultTrendChartDataRangeType = value;
        updateUserApplicationCloudSettingValue('statistics.defaultTrendChartDataRangeType', value);
    }

    function setStatisticsDefaultAssetTrendsChartType(value: number): void {
        updateApplicationSettingsSubValue('statistics', 'defaultAssetTrendsChartType', value);
        appSettings.value.statistics.defaultAssetTrendsChartType = value;
        updateUserApplicationCloudSettingValue('statistics.defaultAssetTrendsChartType', value);
    }

    function setStatisticsDefaultAssetTrendsChartDateRange(value: number): void {
        updateApplicationSettingsSubValue('statistics', 'defaultAssetTrendsChartDataRangeType', value);
        appSettings.value.statistics.defaultAssetTrendsChartDataRangeType = value;
        updateUserApplicationCloudSettingValue('statistics.defaultAssetTrendsChartDataRangeType', value);
    }

    function clearAppSettings(): void {
        clearSettings();
        appSettings.value = getApplicationSettings();
    }

    function updateLocalizedDefaultSettings(newLocaleDefaultSettings: LocaleDefaultSettings | null) {
        if (!newLocaleDefaultSettings) {
            return;
        }

        localeDefaultSettings.value.currency = newLocaleDefaultSettings.currency;
        localeDefaultSettings.value.firstDayOfWeek = newLocaleDefaultSettings.firstDayOfWeek;
    }

    return {
        // 状态
        appSettings,
        syncedAppSettings,
        localeDefaultSettings,
        // 计算状态
        enableApplicationCloudSync,
        // 函数
        // -- 基础设置
        setTheme,
        setFontSize,
        setTimeZone,
        setAutoUpdateExchangeRatesData,
        setShowAccountBalance,
        setEnableSwipeBack,
        setEnableAnimate,
        // -- 应用锁
        setEnableApplicationLock,
        setEnableApplicationLockWebAuthn,
        // -- 导航栏
        setShowAddTransactionButtonInDesktopNavbar,
        // -- 概览页
        setShowAmountInHomePage,
        setTimezoneUsedForStatisticsInHomePage,
        setOverviewAccountFilterInHomePage,
        setOverviewTransactionCategoryFilterInHomePage,
        // -- 交易列表页
        setItemsCountInTransactionListPage,
        setShowTotalAmountInTransactionListPage,
        setShowTagInTransactionListPage,
        // -- 交易编辑页
        setAutoSaveTransactionDraft,
        setAutoGetCurrentGeoLocation,
        setAlwaysShowTransactionPicturesInMobileTransactionEditPage,
        setBillImportDefaultDirectoryName,
        // -- 账户列表页
        setTotalAmountExcludeAccountIds,
        // -- 汇率数据页
        setCurrencySortByInExchangeRatesPage,
        // -- 统计设置
        setStatisticsDefaultChartDataType,
        setStatisticsDefaultTimezoneType,
        setStatisticsDefaultAccountFilter,
        setStatisticsDefaultTransactionCategoryFilter,
        setStatisticsSortingType,
        setStatisticsDefaultCategoricalChartType,
        setStatisticsDefaultCategoricalChartDateRange,
        setStatisticsDefaultTrendChartType,
        setStatisticsDefaultTrendChartDateRange,
        setStatisticsDefaultAssetTrendsChartType,
        setStatisticsDefaultAssetTrendsChartDateRange,
        clearAppSettings,
        createApplicationCloudSettings,
        setApplicationSettingsFromCloudSettings,
        updateApplicationSyncSettingKeys,
        updateLocalizedDefaultSettings
    };
});
