/* eslint-disable @typescript-eslint/no-require-imports */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import { DateRangeScene } from '@/core/datetime.ts';
import { StatisticsAnalysisType } from '@/core/statistics.ts';

const mockGetAllDateRanges = jest.fn();
const mockGetAllTimezoneTypesUsedForStatistics = jest.fn();
const mockGetAllCategoricalChartTypes = jest.fn();
const mockGetAllTrendChartTypes = jest.fn();
const mockGetAllStatisticsChartDataTypes = jest.fn();
const mockGetAllStatisticsSortingTypes = jest.fn();

const settingsStore = {
    appSettings: {
        statistics: {
            defaultChartDataType: 1,
            defaultTimezoneType: 2,
            defaultSortingType: 3,
            defaultCategoricalChartType: 4,
            defaultCategoricalChartDataRangeType: 5,
            defaultTrendChartType: 6,
            defaultTrendChartDataRangeType: 7,
            defaultAssetTrendsChartType: 8,
            defaultAssetTrendsChartDataRangeType: 9,
        },
    },
    setStatisticsDefaultChartDataType: jest.fn(),
    setStatisticsDefaultTimezoneType: jest.fn(),
    setStatisticsSortingType: jest.fn(),
    setStatisticsDefaultCategoricalChartType: jest.fn(),
    setStatisticsDefaultCategoricalChartDateRange: jest.fn(),
    setStatisticsDefaultTrendChartType: jest.fn(),
    setStatisticsDefaultTrendChartDateRange: jest.fn(),
    setStatisticsDefaultAssetTrendsChartType: jest.fn(),
    setStatisticsDefaultAssetTrendsChartDateRange: jest.fn(),
};

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        getAllDateRanges: (...args: unknown[]) => mockGetAllDateRanges(...args),
        getAllTimezoneTypesUsedForStatistics: (...args: unknown[]) => mockGetAllTimezoneTypesUsedForStatistics(...args),
        getAllCategoricalChartTypes: (...args: unknown[]) => mockGetAllCategoricalChartTypes(...args),
        getAllTrendChartTypes: (...args: unknown[]) => mockGetAllTrendChartTypes(...args),
        getAllStatisticsChartDataTypes: (...args: unknown[]) => mockGetAllStatisticsChartDataTypes(...args),
        getAllStatisticsSortingTypes: (...args: unknown[]) => mockGetAllStatisticsSortingTypes(...args),
    }),
}));

jest.mock('@/stores/setting.ts', () => ({
    useSettingsStore: () => settingsStore,
}));

const { useStatisticsSettingPageBase } = require(
    '@/views/base/statistics/StatisticsSettingPageBase.ts'
) as typeof import('@/views/base/statistics/StatisticsSettingPageBase.ts');

beforeEach(() => {
    jest.clearAllMocks();
    mockGetAllStatisticsChartDataTypes.mockReturnValue([{ type: 1, displayName: 'Amount' }]);
    mockGetAllTimezoneTypesUsedForStatistics.mockReturnValue([{ type: 2, displayName: 'Local' }]);
    mockGetAllStatisticsSortingTypes.mockReturnValue([{ type: 3, displayName: 'Descending' }]);
    mockGetAllCategoricalChartTypes.mockReturnValue([{ type: 4, displayName: 'Pie' }]);
    mockGetAllTrendChartTypes.mockReturnValue([{ type: 6, displayName: 'Line' }]);
    mockGetAllDateRanges.mockImplementation((...args: unknown[]) => {
        const scene = args[0] as number;
        return [{ type: scene, displayName: `scene:${scene}` }];
    });
});

describe('StatisticsSettingPageBase behavior', () => {
    test('loads every localized option group using the correct analysis scenes', () => {
        const base = useStatisticsSettingPageBase();

        expect(base.allChartDataTypes.value).toEqual([{ type: 1, displayName: 'Amount' }]);
        expect(base.allTimezoneTypesUsedForStatistics.value).toEqual([{ type: 2, displayName: 'Local' }]);
        expect(base.allSortingTypes.value).toEqual([{ type: 3, displayName: 'Descending' }]);
        expect(base.allCategoricalChartTypes.value).toEqual([{ type: 4, displayName: 'Pie' }]);
        expect(base.allTrendChartTypes.value).toEqual([{ type: 6, displayName: 'Line' }]);
        expect(base.allCategoricalChartDateRanges.value).toEqual([
            { type: DateRangeScene.Normal, displayName: `scene:${DateRangeScene.Normal}` },
        ]);
        expect(base.allTrendChartDateRanges.value).toEqual([
            { type: DateRangeScene.TrendAnalysis, displayName: `scene:${DateRangeScene.TrendAnalysis}` },
        ]);
        expect(base.allAssetTrendsChartDateRanges.value).toEqual([
            { type: DateRangeScene.AssetTrends, displayName: `scene:${DateRangeScene.AssetTrends}` },
        ]);

        expect(mockGetAllStatisticsChartDataTypes).toHaveBeenCalledWith(StatisticsAnalysisType.CategoricalAnalysis);
        expect(mockGetAllDateRanges.mock.calls).toEqual([
            [DateRangeScene.Normal, false],
            [DateRangeScene.TrendAnalysis, false],
            [DateRangeScene.AssetTrends, false],
        ]);
    });

    test('projects all defaults and forwards each computed setter to the settings store', () => {
        const base = useStatisticsSettingPageBase();
        const bindings = [
            ['defaultChartDataType', 'setStatisticsDefaultChartDataType', 1, 11],
            ['defaultTimezoneType', 'setStatisticsDefaultTimezoneType', 2, 12],
            ['defaultSortingType', 'setStatisticsSortingType', 3, 13],
            ['defaultCategoricalChartType', 'setStatisticsDefaultCategoricalChartType', 4, 14],
            ['defaultCategoricalChartDateRange', 'setStatisticsDefaultCategoricalChartDateRange', 5, 15],
            ['defaultTrendChartType', 'setStatisticsDefaultTrendChartType', 6, 16],
            ['defaultTrendChartDateRange', 'setStatisticsDefaultTrendChartDateRange', 7, 17],
            ['defaultAssetTrendsChartType', 'setStatisticsDefaultAssetTrendsChartType', 8, 18],
            ['defaultAssetTrendsChartDateRange', 'setStatisticsDefaultAssetTrendsChartDateRange', 9, 19],
        ] as const;

        for (const [computedKey, setterKey, initialValue, nextValue] of bindings) {
            expect(base[computedKey].value).toBe(initialValue);
            base[computedKey].value = nextValue;
            expect(settingsStore[setterKey]).toHaveBeenCalledWith(nextValue);
        }
    });
});
