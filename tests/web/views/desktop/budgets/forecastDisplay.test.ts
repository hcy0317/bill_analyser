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
        historicalAverageCents: 10000,
        currentSpentCents: 8000,
        projectedTotalCents: 15000,
        budgetAmountCents: 12000,
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
        historicalAverageCents: 5000,
        currentSpentCents: 1500,
        projectedTotalCents: 4000,
        budgetAmountCents: 6000,
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
        historicalAverageCents: 3000,
        currentSpentCents: 2800,
        projectedTotalCents: 6200,
        budgetAmountCents: 5000,
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
        historicalAverageCents: 2000,
        currentSpentCents: 500,
        projectedTotalCents: 1800,
        budgetAmountCents: 3000,
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

    test('filterAndSortForecasts reaches category tie-breakers for equal confidence and totals', () => {
        const tiedForecasts: BudgetForecastItem[] = [
            {
                ...SAMPLE_FORECASTS[0]!,
                categoryId: '9',
                categoryName: '游戏',
                projectedTotalCents: 100,
                backtestMape: null,
                confidence: 'medium'
            },
            {
                ...SAMPLE_FORECASTS[0]!,
                categoryId: '10',
                categoryName: '电影',
                projectedTotalCents: 100,
                backtestMape: null,
                confidence: 'medium'
            }
        ];

        expect(filterAndSortForecasts(tiedForecasts, { sortBy: 'confidence' })
            .map(item => item.categoryName)).toStrictEqual(['电影', '游戏']);
        expect(filterAndSortForecasts(tiedForecasts, { sortBy: 'projected_total' })
            .map(item => item.categoryName)).toStrictEqual(['电影', '游戏']);
        expect(filterAndSortForecasts(tiedForecasts, { sortBy: 'backtest' })
            .map(item => item.categoryName)).toStrictEqual(['电影', '游戏']);
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
