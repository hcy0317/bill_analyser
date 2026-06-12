import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { DateRange } from '@/core/datetime.ts';
import { TransactionTagFilterType } from '@/core/transaction.ts';
import { useStatisticsStore } from '@/stores/statistics.ts';

const mockGetTransactionStatistics = jest.fn();
const mockGetTransactionStatisticsTrends = jest.fn();
const mockGetTransactionStatisticsAssetTrends = jest.fn();

const mockSettingsStore = {
    appSettings: {
        statistics: {
            defaultTimezoneType: 1,
            defaultChartDataType: 0,
            defaultCategoricalChartType: 0,
            defaultCategoricalChartDataRangeType: 7,
            defaultTrendChartType: 0,
            defaultTrendChartDataRangeType: 9,
            defaultAssetTrendsChartDataRangeType: 9
        }
    }
};

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getTransactionStatistics: (...args: unknown[]) => mockGetTransactionStatistics(...args),
        getTransactionStatisticsTrends: (...args: unknown[]) => mockGetTransactionStatisticsTrends(...args),
        getTransactionStatisticsAssetTrends: (...args: unknown[]) => mockGetTransactionStatisticsAssetTrends(...args)
    }
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

jest.mock('@/locales/helpers.ts', () => ({
    __esModule: true,
    useI18n: () => ({ tt: (value: string) => value })
}));

jest.mock('@/stores/setting.ts', () => ({
    __esModule: true,
    useSettingsStore: () => mockSettingsStore
}));

jest.mock('@/stores/user.ts', () => ({
    __esModule: true,
    useUserStore: () => ({
        currentUserFirstDayOfWeek: 1,
        currentUserFiscalYearStart: 1,
        currentUserDefaultCurrency: 'CNY'
    })
}));

jest.mock('@/stores/account.ts', () => ({
    __esModule: true,
    useAccountsStore: () => ({
        allAccountsMap: {},
        allPlainAccounts: []
    })
}));

jest.mock('@/stores/transactionCategory.ts', () => ({
    __esModule: true,
    useTransactionCategoriesStore: () => ({
        allTransactionCategoriesMap: {}
    })
}));

jest.mock('@/stores/exchangeRates.ts', () => ({
    __esModule: true,
    useExchangeRatesStore: () => ({
        getExchangedAmount: (amount: number) => amount
    })
}));

function statisticsEnvelope<T>(result: T): Promise<{ data: { success: boolean; result: T } }> {
    return Promise.resolve({
        data: {
            success: true,
            result
        }
    });
}

describe('statistics store service boundary', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        mockGetTransactionStatistics.mockReset();
        mockGetTransactionStatisticsTrends.mockReset();
        mockGetTransactionStatisticsAssetTrends.mockReset();
    });

    test('loads categorical statistics through success/result and clears invalid state', async () => {
        const result = {
            startTime: 100,
            endTime: 200,
            items: [{ categoryId: 'cat-food', accountId: 'cash', amountCents: -1234 }]
        };
        mockGetTransactionStatistics.mockReturnValue(statisticsEnvelope(result));

        const store = useStatisticsStore();
        store.transactionStatisticsFilter.categoricalChartStartTime = 100;
        store.transactionStatisticsFilter.categoricalChartEndTime = 200;
        store.transactionStatisticsFilter.tagIds = 'tag-1,tag-2';
        store.transactionStatisticsFilter.tagFilterType = TransactionTagFilterType.NotHasAny.type;
        store.transactionStatisticsFilter.keyword = 'coffee';
        store.updateTransactionStatisticsInvalidState(true);

        await expect(store.loadCategoricalAnalysis({ force: false })).resolves.toBe(result);
        expect(mockGetTransactionStatistics).toHaveBeenCalledWith({
            startTime: 100,
            endTime: 200,
            tagIds: 'tag-1,tag-2',
            tagFilterType: TransactionTagFilterType.NotHasAny.type,
            keyword: 'coffee',
            useTransactionTimezone: true
        });
        expect(store.transactionStatisticsStateInvalid).toBe(false);
        expect(store.transactionCategoryStatisticsData).toStrictEqual(result);
    });

    test('rejects force refresh when category statistics are already current', async () => {
        const result = { startTime: 1, endTime: 2, items: [] };
        mockGetTransactionStatistics.mockReturnValue(statisticsEnvelope(result));

        const store = useStatisticsStore();
        await store.loadCategoricalAnalysis({ force: false });

        await expect(store.loadCategoricalAnalysis({ force: true })).rejects.toMatchObject({
            message: 'Data is up to date',
            isUpToDate: true
        });
    });

    test('loads trend statistics using the all-range 197001 sentinel', async () => {
        const result = [{ year: 2026, month: 3, items: [] }];
        mockGetTransactionStatisticsTrends.mockReturnValue(statisticsEnvelope(result));

        const store = useStatisticsStore();
        store.transactionStatisticsFilter.trendChartDateType = DateRange.All.type;
        store.transactionStatisticsFilter.trendChartStartYearMonth = '2026-01';
        store.transactionStatisticsFilter.trendChartEndYearMonth = '2026-03';
        store.transactionStatisticsFilter.keyword = '餐饮';

        await expect(store.loadTrendAnalysis({ force: false })).resolves.toBe(result);
        expect(mockGetTransactionStatisticsTrends).toHaveBeenCalledWith({
            startYearMonth: '197001',
            endYearMonth: '197001',
            tagIds: '',
            tagFilterType: TransactionTagFilterType.Default.type,
            keyword: '餐饮',
            useTransactionTimezone: true
        });
        expect(store.transactionCategoryTrendsData).toStrictEqual(result);
    });

    test('loads asset trend statistics through success/result and keeps cents intact', async () => {
        const result = [{
            year: 2026,
            month: 3,
            day: 1,
            items: [{
                accountId: 'cash',
                accountOpeningBalanceCents: 10025,
                accountClosingBalanceCents: 10941
            }]
        }];
        mockGetTransactionStatisticsAssetTrends.mockReturnValue(statisticsEnvelope(result));

        const store = useStatisticsStore();
        store.transactionStatisticsFilter.assetTrendsChartStartTime = 1_772_409_600;
        store.transactionStatisticsFilter.assetTrendsChartEndTime = 1_772_496_000;

        await expect(store.loadAssetTrends({ force: false })).resolves.toBe(result);
        expect(mockGetTransactionStatisticsAssetTrends).toHaveBeenCalledWith({
            startTime: 1_772_409_600,
            endTime: 1_772_496_000
        });
        await expect(store.loadAssetTrends({ force: true })).rejects.toMatchObject({
            message: 'Data is up to date',
            isUpToDate: true
        });
        expect(mockGetTransactionStatisticsAssetTrends).toHaveBeenCalledTimes(2);
        expect(result[0]!.items[0]!.accountOpeningBalanceCents).toBe(10025);
        expect(result[0]!.items[0]!.accountClosingBalanceCents).toBe(10941);
    });
});
