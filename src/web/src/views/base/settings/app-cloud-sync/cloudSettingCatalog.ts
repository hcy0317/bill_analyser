import type { ApplicationCloudSetting } from '@/core/setting.ts';

export interface CategorizedApplicationCloudSettingItems {
    readonly categoryName: string;
    readonly categorySubName?: string;
    readonly items: ApplicationCloudSettingItem[];
}

export interface ApplicationCloudSettingItem {
    readonly settingKey: string;
    readonly settingName: string;
    readonly mobile: boolean;
    readonly desktop: boolean;
}

export type UserApplicationCloudSettingsPayload = ApplicationCloudSetting[] | false;

export const ALL_APPLICATION_CLOUD_SETTINGS: CategorizedApplicationCloudSettingItems[] = [
    {
        categoryName: 'Basic Settings',
        items: [
            { settingKey: 'showAccountBalance', settingName: 'Show Account Balance', mobile: true, desktop: true }
        ]
    },
    {
        categoryName: 'Overview Page',
        items: [
            { settingKey: 'showAmountInHomePage', settingName: 'Show Amount', mobile: true, desktop: true },
            { settingKey: 'timezoneUsedForStatisticsInHomePage', settingName: 'Timezone Used for Statistics', mobile: true, desktop: true },
            { settingKey: 'overviewAccountFilterInHomePage', settingName: 'Accounts Included in Overview Statistics', mobile: true, desktop: true },
            { settingKey: 'overviewTransactionCategoryFilterInHomePage', settingName: 'Transaction Categories Included in Overview Statistics', mobile: true, desktop: true }
        ]
    },
    {
        categoryName: 'Transaction List Page',
        items: [
            { settingKey: 'itemsCountInTransactionListPage', settingName: 'Transactions Per Page', mobile: false, desktop: true },
            { settingKey: 'showTotalAmountInTransactionListPage', settingName: 'Show Monthly Total Amount', mobile: true, desktop: true },
            { settingKey: 'showTagInTransactionListPage', settingName: 'Show Transaction Tag', mobile: true, desktop: true }
        ]
    },
    {
        categoryName: 'Transaction Edit Page',
        items: [
            { settingKey: 'autoSaveTransactionDraft', settingName: 'Automatically Save Draft', mobile: true, desktop: true },
            { settingKey: 'autoGetCurrentGeoLocation', settingName: 'Automatically Add Geolocation', mobile: true, desktop: true },
            { settingKey: 'alwaysShowTransactionPicturesInMobileTransactionEditPage', settingName: 'Always Show Transaction Pictures', mobile: true, desktop: false }
        ]
    },
    {
        categoryName: 'Account List Page',
        items: [
            { settingKey: 'totalAmountExcludeAccountIds', settingName: 'Accounts Included in Total', mobile: true, desktop: true },
        ]
    },
    {
        categoryName: 'Exchange Rates Data Page',
        items: [
            { settingKey: 'currencySortByInExchangeRatesPage', settingName: 'Sort by', mobile: true, desktop: true }
        ]
    },
    {
        categoryName: 'Statistics Settings',
        categorySubName: 'Common Settings',
        items: [
            { settingKey: 'statistics.defaultChartDataType', settingName: 'Default Chart Data Type', mobile: true, desktop: true },
            { settingKey: 'statistics.defaultTimezoneType', settingName: 'Timezone Used for Date Range', mobile: true, desktop: true },
            { settingKey: 'statistics.defaultAccountFilter', settingName: 'Default Account Filter', mobile: true, desktop: true },
            { settingKey: 'statistics.defaultTransactionCategoryFilter', settingName: 'Default Transaction Category Filter', mobile: true, desktop: true },
            { settingKey: 'statistics.defaultSortingType', settingName: 'Default Sort Order', mobile: true, desktop: true }
        ]
    },
    {
        categoryName: 'Statistics Settings',
        categorySubName: 'Categorical Analysis Settings',
        items: [
            { settingKey: 'statistics.defaultCategoricalChartType', settingName: 'Default Chart Type', mobile: true, desktop: true },
            { settingKey: 'statistics.defaultCategoricalChartDataRangeType', settingName: 'Default Date Range', mobile: true, desktop: true }
        ]
    },
    {
        categoryName: 'Statistics Settings',
        categorySubName: 'Trend Analysis Settings',
        items: [
            { settingKey: 'statistics.defaultTrendChartType', settingName: 'Default Chart Type', mobile: false, desktop: true },
            { settingKey: 'statistics.defaultTrendChartDataRangeType', settingName: 'Default Date Range', mobile: true, desktop: true }
        ]
    },
    {
        categoryName: 'Statistics Settings',
        categorySubName: 'Asset Trends Settings',
        items: [
            { settingKey: 'statistics.defaultAssetTrendsChartType', settingName: 'Default Chart Type', mobile: false, desktop: true },
            { settingKey: 'statistics.defaultAssetTrendsChartDataRangeType', settingName: 'Default Date Range', mobile: true, desktop: true }
        ]
    }
];
