import { describe, expect, test } from '@jest/globals';

import type { BudgetForecastItem } from '@/models/budget.ts';
import {
    compareNullableNumbers,
    filterAndSortForecasts,
    getForecastConfidenceRank,
    summarizeForecastRisks
} from '@/views/desktop/budgets/forecastDisplay.ts';

const SAMPLE_FORECASTS: BudgetForecastItem[] = [
    {
        categoryId: '1',
        categoryName: '餐饮',
        historicalAverage: 10000,
        currentSpent: 8000,
        projectedTotal: 15000,
        budgetAmount: 12000,
        projectedOverBudget: true,
        trend: 'up',
        samplePeriods: 6,
        strategyExplanation: '历史均值',
        backtestMape: 18.5,
        confidence: 'medium'
    },
    {
        categoryId: '2',
        categoryName: '交通',
        historicalAverage: 5000,
        currentSpent: 1500,
        projectedTotal: 4000,
        budgetAmount: 6000,
        projectedOverBudget: false,
        trend: 'down',
        samplePeriods: 6,
        strategyExplanation: '移动平均',
        backtestMape: 6.2,
        confidence: 'high'
    },
    {
        categoryId: '3',
        categoryName: '娱乐',
        historicalAverage: 3000,
        currentSpent: 2800,
        projectedTotal: 6200,
        budgetAmount: 5000,
        projectedOverBudget: true,
        trend: 'up',
        samplePeriods: 3,
        strategyExplanation: '移动平均',
        backtestMape: 28.4,
        confidence: 'low'
    },
    {
        categoryId: '4',
        categoryName: '医疗',
        historicalAverage: 2000,
        currentSpent: 500,
        projectedTotal: 1800,
        budgetAmount: 3000,
        projectedOverBudget: false,
        trend: 'stable',
        samplePeriods: 3,
        strategyExplanation: '历史均值',
        backtestMape: null,
        confidence: 'low'
    }
];

describe('forecastDisplay helpers', () => {
    test('getForecastConfidenceRank returns expected priority', () => {
        expect(getForecastConfidenceRank('high')).toBe(0);
        expect(getForecastConfidenceRank('medium')).toBe(1);
        expect(getForecastConfidenceRank('low')).toBe(2);
        expect(getForecastConfidenceRank(undefined)).toBe(2);
    });

    test('compareNullableNumbers sorts null values last', () => {
        expect(compareNullableNumbers(1, 2)).toBeLessThan(0);
        expect(compareNullableNumbers(2, 1)).toBeGreaterThan(0);
        expect(compareNullableNumbers(null, 1)).toBeGreaterThan(0);
        expect(compareNullableNumbers(1, null)).toBeLessThan(0);
        expect(compareNullableNumbers(null, null)).toBe(0);
    });

    test('filterAndSortForecasts sorts by backtest MAPE ascending by default', () => {
        const result = filterAndSortForecasts(SAMPLE_FORECASTS, {
            sortBy: 'backtest'
        });

        expect(result.map(item => item.categoryName)).toStrictEqual(['交通', '餐饮', '娱乐', '医疗']);
    });

    test('filterAndSortForecasts can filter only low confidence items', () => {
        const result = filterAndSortForecasts(SAMPLE_FORECASTS, {
            sortBy: 'category',
            onlyLowConfidence: true
        });

        expect(result.map(item => item.categoryName)).toStrictEqual(['医疗', '娱乐']);
    });

    test('filterAndSortForecasts can filter only over budget items', () => {
        const result = filterAndSortForecasts(SAMPLE_FORECASTS, {
            sortBy: 'projected_total',
            onlyOverBudget: true
        });

        expect(result.map(item => item.categoryName)).toStrictEqual(['餐饮', '娱乐']);
    });

    test('filterAndSortForecasts can combine filters and confidence sorting', () => {
        const result = filterAndSortForecasts(SAMPLE_FORECASTS, {
            sortBy: 'confidence',
            onlyLowConfidence: true,
            onlyOverBudget: true
        });

        expect(result).toHaveLength(1);
        expect(result[0]!.categoryName).toBe('娱乐');
    });

    test('filterAndSortForecasts uses category name as the final tie-breaker for confidence sorting', () => {
        const tiedForecasts: BudgetForecastItem[] = [
            {
                ...SAMPLE_FORECASTS[2]!,
                categoryId: '5',
                categoryName: '电影',
                backtestMape: 18,
                confidence: 'medium',
                projectedOverBudget: false
            },
            {
                ...SAMPLE_FORECASTS[2]!,
                categoryId: '6',
                categoryName: '游戏',
                backtestMape: 18,
                confidence: 'medium',
                projectedOverBudget: false
            }
        ];

        const result = filterAndSortForecasts(tiedForecasts, {
            sortBy: 'confidence'
        });

        expect(result.map(item => item.categoryName)).toStrictEqual(['电影', '游戏']);
    });

    test('filterAndSortForecasts defaults to backtest sorting for unknown runtime sort values', () => {
        const result = filterAndSortForecasts(SAMPLE_FORECASTS, {
            sortBy: 'unexpected' as never
        });

        expect(result.map(item => item.categoryName)).toStrictEqual(['交通', '餐饮', '娱乐', '医疗']);
    });

    test('filterAndSortForecasts uses confidence as the tie-breaker for backtest sorting', () => {
        const tiedForecasts: BudgetForecastItem[] = [
            {
                ...SAMPLE_FORECASTS[0]!,
                categoryId: '7',
                categoryName: '高置信',
                backtestMape: 10,
                confidence: 'high'
            },
            {
                ...SAMPLE_FORECASTS[0]!,
                categoryId: '8',
                categoryName: '低置信',
                backtestMape: 10,
                confidence: 'low'
            }
        ];

        const result = filterAndSortForecasts(tiedForecasts, {
            sortBy: 'backtest'
        });

        expect(result.map(item => item.categoryName)).toStrictEqual(['高置信', '低置信']);
    });

    test('summarizeForecastRisks returns total, risk counts and filtered count', () => {
        const filtered = filterAndSortForecasts(SAMPLE_FORECASTS, {
            sortBy: 'category',
            onlyOverBudget: true
        });

        const summary = summarizeForecastRisks(SAMPLE_FORECASTS, filtered.length);

        expect(summary).toStrictEqual({
            totalCount: 4,
            lowConfidenceCount: 2,
            overBudgetCount: 2,
            filteredCount: 2
        });
    });
});
