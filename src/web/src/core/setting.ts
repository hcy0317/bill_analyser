import { type WeekDayValue, WeekDay } from './datetime.ts';
import { TimezoneTypeForStatistics } from './timezone.ts';
import { CurrencySortingType } from './currency.ts';
import {
    CategoricalChartType,
    TrendChartType,
    ChartDataType,
    ChartSortingType,
    DEFAULT_CATEGORICAL_CHART_DATA_RANGE,
    DEFAULT_TREND_CHART_DATA_RANGE,
    DEFAULT_ASSET_TRENDS_CHART_DATA_RANGE
} from './statistics.ts';
import { DEFAULT_CURRENCY_CODE } from '@/consts/currency.ts';

export type ApplicationSettingKey = string;
export type ApplicationSettingValue = string | number | boolean | Record<string, ApplicationSettingSubValue>;
export type ApplicationSettingSubValue = string | number | boolean | Record<string, boolean> | Record<string, number>;

export interface BaseApplicationSetting {
    [key: ApplicationSettingKey]: ApplicationSettingValue;
}

export interface ApplicationSettings extends BaseApplicationSetting {
    // 调试设置
    debug: boolean;
    // 基础设置
    theme: string;
    fontSize: number;
    timeZone: string;
    autoUpdateExchangeRatesData: boolean;
    showAccountBalance: boolean;
    swipeBack: boolean;
    animate: boolean;
    // 应用锁
    applicationLock: boolean;
    applicationLockWebAuthn: boolean;
    // 导航栏
    showAddTransactionButtonInDesktopNavbar: boolean;
    // 概览页
    showAmountInHomePage: boolean;
    timezoneUsedForStatisticsInHomePage: number;
    overviewAccountFilterInHomePage: Record<string, boolean>;
    overviewTransactionCategoryFilterInHomePage: Record<string, boolean>;
    // 交易列表页
    itemsCountInTransactionListPage: number;
    showTotalAmountInTransactionListPage: boolean;
    showTagInTransactionListPage: boolean;
    // 交易编辑页
    autoSaveTransactionDraft: string;
    autoGetCurrentGeoLocation: boolean;
    alwaysShowTransactionPicturesInMobileTransactionEditPage: boolean;
    // 账户列表页
    totalAmountExcludeAccountIds: Record<string, boolean>;
    // 汇率数据页
    currencySortByInExchangeRatesPage: number;
    // 统计设置
    statistics: {
        defaultChartDataType: number;
        defaultTimezoneType: number;
        defaultAccountFilter: Record<string, boolean>;
        defaultTransactionCategoryFilter: Record<string, boolean>;
        defaultSortingType: number;
        defaultCategoricalChartType: number;
        defaultCategoricalChartDataRangeType: number;
        defaultTrendChartType: number;
        defaultTrendChartDataRangeType: number;
        defaultAssetTrendsChartType: number;
        defaultAssetTrendsChartDataRangeType: number;
    };
}

export enum UserApplicationCloudSettingType {
    String = 'string',
    Number = 'number',
    Boolean = 'boolean',
    StringBooleanMap = 'string_boolean_map',
}

export interface ApplicationCloudSetting {
    readonly settingKey: string;
    readonly settingValue: string;
}

export interface LocaleDefaultSettings {
    currency: string;
    firstDayOfWeek: WeekDayValue;
}

export interface ApplicationLockState {
    readonly username: string;
    readonly secret: string;
}

export interface WebAuthnConfig {
    readonly credentialId: string;
}

export const ALL_ALLOWED_CLOUD_SYNC_APP_SETTING_KEY_TYPES: Record<string, UserApplicationCloudSettingType> = {
    // 基础设置
    'showAccountBalance': UserApplicationCloudSettingType.Boolean,
    // 概览页
    'showAmountInHomePage': UserApplicationCloudSettingType.Boolean,
    'timezoneUsedForStatisticsInHomePage': UserApplicationCloudSettingType.Number,
    'overviewAccountFilterInHomePage': UserApplicationCloudSettingType.StringBooleanMap,
    'overviewTransactionCategoryFilterInHomePage': UserApplicationCloudSettingType.StringBooleanMap,
    // 交易列表页
    'itemsCountInTransactionListPage': UserApplicationCloudSettingType.Number,
    'showTotalAmountInTransactionListPage': UserApplicationCloudSettingType.Boolean,
    'showTagInTransactionListPage': UserApplicationCloudSettingType.Boolean,
    // 交易编辑页
    'autoSaveTransactionDraft': UserApplicationCloudSettingType.String,
    'autoGetCurrentGeoLocation': UserApplicationCloudSettingType.Boolean,
    'alwaysShowTransactionPicturesInMobileTransactionEditPage': UserApplicationCloudSettingType.Boolean,
    // 账户列表页
    'totalAmountExcludeAccountIds': UserApplicationCloudSettingType.StringBooleanMap,
    // 汇率数据页
    'currencySortByInExchangeRatesPage': UserApplicationCloudSettingType.Number,
    // 统计设置
    'statistics.defaultChartDataType': UserApplicationCloudSettingType.Number,
    'statistics.defaultTimezoneType': UserApplicationCloudSettingType.Number,
    'statistics.defaultAccountFilter': UserApplicationCloudSettingType.StringBooleanMap,
    'statistics.defaultTransactionCategoryFilter': UserApplicationCloudSettingType.StringBooleanMap,
    'statistics.defaultSortingType': UserApplicationCloudSettingType.Number,
    'statistics.defaultCategoricalChartType': UserApplicationCloudSettingType.Number,
    'statistics.defaultCategoricalChartDataRangeType': UserApplicationCloudSettingType.Number,
    'statistics.defaultTrendChartType': UserApplicationCloudSettingType.Number,
    'statistics.defaultTrendChartDataRangeType': UserApplicationCloudSettingType.Number,
    'statistics.defaultAssetTrendsChartType': UserApplicationCloudSettingType.Number,
    'statistics.defaultAssetTrendsChartDataRangeType': UserApplicationCloudSettingType.Number,
};

export const DEFAULT_APPLICATION_SETTINGS: ApplicationSettings = {
    // 调试设置
    debug: false,
    // 基础设置
    theme: 'auto',
    fontSize: 1,
    timeZone: '',
    autoUpdateExchangeRatesData: true,
    showAccountBalance: true,
    swipeBack: true,
    animate: true,
    // 应用锁
    applicationLock: false,
    applicationLockWebAuthn: false,
    // 导航栏
    showAddTransactionButtonInDesktopNavbar: true,
    // 概览页
    showAmountInHomePage: true,
    timezoneUsedForStatisticsInHomePage: TimezoneTypeForStatistics.Default.type,
    overviewAccountFilterInHomePage: {},
    overviewTransactionCategoryFilterInHomePage: {},
    // 交易列表页
    itemsCountInTransactionListPage: 15,
    showTotalAmountInTransactionListPage: true,
    showTagInTransactionListPage: true,
    // 交易编辑页
    autoSaveTransactionDraft: 'disabled',
    autoGetCurrentGeoLocation: false,
    alwaysShowTransactionPicturesInMobileTransactionEditPage: false,
    // 账户列表页
    totalAmountExcludeAccountIds: {},
    // 汇率数据页
    currencySortByInExchangeRatesPage: CurrencySortingType.Default.type,
    // 统计设置
    statistics: {
        defaultChartDataType: ChartDataType.Default.type,
        defaultTimezoneType: TimezoneTypeForStatistics.Default.type,
        defaultAccountFilter: {},
        defaultTransactionCategoryFilter: {},
        defaultSortingType: ChartSortingType.Default.type,
        defaultCategoricalChartType: CategoricalChartType.Default.type,
        defaultCategoricalChartDataRangeType: DEFAULT_CATEGORICAL_CHART_DATA_RANGE.type,
        defaultTrendChartType: TrendChartType.Default.type,
        defaultTrendChartDataRangeType: DEFAULT_TREND_CHART_DATA_RANGE.type,
        defaultAssetTrendsChartType: TrendChartType.Default.type,
        defaultAssetTrendsChartDataRangeType: DEFAULT_ASSET_TRENDS_CHART_DATA_RANGE.type,
    }
};

export const DEFAULT_LOCALE_SETTINGS: LocaleDefaultSettings = {
    currency: DEFAULT_CURRENCY_CODE,
    firstDayOfWeek: WeekDay.DefaultFirstDay.type
};
