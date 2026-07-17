import { describe, expect, test } from '@jest/globals';

import { BudgetForecastStrategy, BudgetPeriodType, BudgetType } from '@/models/budget.ts';
import {
    buildBudgetExecutionQuery,
    buildBudgetForecastQuery,
    buildBudgetHistoryQuery,
    buildBudgetListQuery,
    mapBudgetRequestToRest,
    mapImportedBudgetToRest,
    mapRestBudgetToFrontend,
    mapRestExecutionToFrontend,
    mapRestForecastToFrontend,
    mapRestHistoryToFrontend
} from '@/lib/services/budget.ts';

describe('budget service adapter fallback behavior', () => {
    test('omits absent query fields while preserving meaningful zero and boolean values', () => {
        expect(buildBudgetExecutionQuery({
            type: 0,
            periodType: '',
            year: 0,
            month: 0,
            quarter: 0,
            startDate: '',
            endDate: ''
        })).toBe('?budget_type=0&year=0&month=0&quarter=0');
        expect(buildBudgetListQuery()).toBe('');
        expect(buildBudgetListQuery({ type: 0, periodType: '', enabled: true, category: '' }))
            .toBe('?budget_type=0&enabled=true');
        expect(buildBudgetHistoryQuery({ accountIds: [], tagIds: [] })).toBe('');
        expect(buildBudgetForecastQuery({ monthsHistory: 0 })).toBe('?months_history=0');
        expect(buildBudgetForecastQuery()).toBe('');
    });

    test('maps empty, camel-case, and malformed budget values without inventing minor units', () => {
        expect(mapRestBudgetToFrontend(null, BudgetType.Investment)).toEqual(expect.objectContaining({
            id: '',
            name: '',
            periodType: BudgetPeriodType.Monthly,
            amountCents: 0,
            type: BudgetType.Investment,
            enabled: true
        }));

        expect(mapRestBudgetToFrontend({
            id: 0,
            name: 'Camel',
            subCategory: 'Sub',
            categoryId: 'cat-camel',
            periodType: BudgetPeriodType.Yearly,
            amountCents: Number.MAX_SAFE_INTEGER,
            startDate: '2026-01-01',
            endDate: '2026-12-31',
            alertThreshold: 0,
            enabled: false,
            type: BudgetType.Investment,
            createdAt: 'created-camel',
            updatedAt: 'updated-camel',
            spentAmountCents: -1,
            remainingAmountCents: 1,
            executionRate: 0,
            categoryName: 'Category camel',
            categoryIcon: 'icon-camel',
            categoryColor: '#123456'
        })).toEqual(expect.objectContaining({
            id: '0',
            subCategory: 'Sub',
            categoryId: 'cat-camel',
            amountCents: Number.MAX_SAFE_INTEGER,
            spentAmountCents: -1,
            remainingAmountCents: 1,
            alertThreshold: 0,
            enabled: false
        }));

        expect(mapRestBudgetToFrontend({ amount_cents: '9007199254740992' }).amountCents).toBe(0);
        expect(mapRestBudgetToFrontend({ amount_cents: {} }).amountCents).toBe(0);
    });

    test('maps sparse execution and forecast responses through every supported fallback shape', () => {
        expect(mapRestExecutionToFrontend(null)).toEqual(expect.objectContaining({
            totalBudgetCents: 0,
            totalSpentCents: 0,
            totalExecutionRate: 0,
            categories: [],
            periodStart: '',
            periodEnd: ''
        }));

        const execution = mapRestExecutionToFrontend({
            summary: { totalBudgetCents: 500, totalSpentCents: 100 },
            items: [{
                id: 0,
                category_id: '',
                category_info: {},
                category: 'Food',
                budgetAmountCents: 500,
                spentAmountCents: 100,
                remainingAmountCents: 400,
                execution_rate: 20,
                alert_threshold: 80
            }],
            periodStart: 'start-camel',
            periodEnd: 'end-camel'
        });
        expect(execution.categories[0]).toEqual(expect.objectContaining({
            budgetId: '',
            categoryName: 'Food',
            isOverBudget: false,
            alertTriggered: false
        }));

        const forecast = mapRestForecastToFrontend({
            items: [
                {
                    categoryId: 'direct',
                    categoryName: 'Direct',
                    forecastAmountCents: 300,
                    averageAmountCents: 200,
                    currentSpentCents: 100,
                    budgetAmountCents: 500,
                    projectedOverBudget: false,
                    periods: null
                },
                {
                    current_spent_cents: 101,
                    projected_total_cents: 0,
                    periods: [{ period: 'p1', amountCents: 7 }]
                },
                {
                    periods: [{ period: 'p2', amount_cents: 8 }]
                },
                { totalAmountCents: 9 },
                { total_amount_cents: 10 }
            ],
            periodStart: 'camel-start',
            periodEnd: 'camel-end',
            daysRemaining: 0,
            daysElapsed: 0,
            summary: {
                forecastStrategy: BudgetForecastStrategy.MovingAverage,
                historyPeriods: 0,
                avgBacktestMape: 0
            }
        });

        expect(forecast.forecasts.map((item: { currentSpentCents: number }) => item.currentSpentCents))
            .toEqual([100, 101, 8, 9, 10]);
        expect(forecast.forecasts[0]).toEqual(expect.objectContaining({
            categoryId: 'direct',
            categoryName: 'Direct',
            projectedOverBudget: false,
            trend: 'stable',
            confidence: 'low',
            periods: []
        }));
        expect(forecast).toEqual(expect.objectContaining({
            forecastStrategy: BudgetForecastStrategy.MovingAverage,
            historyPeriods: 0,
            avgBacktestMape: 0
        }));
        expect(mapRestForecastToFrontend(null)).toEqual(expect.objectContaining({
            forecasts: [],
            forecastStrategy: BudgetForecastStrategy.HistoricalAverage
        }));
    });

    test('maps history and mutation payload defaults from both naming conventions', () => {
        expect(mapRestHistoryToFrontend(null)).toEqual({
            items: [],
            count: 0,
            periodStart: '',
            periodEnd: ''
        });

        const history = mapRestHistoryToFrontend({
            items: [{
                id: 0,
                budgetId: 0,
                budgetType: null,
                subCategory: 'sub',
                periodType: BudgetPeriodType.Quarterly,
                periodStart: 'start',
                periodEnd: 'end',
                budgetAmountCents: 1,
                spentAmountCents: 2,
                remainingAmountCents: -1,
                executionRate: 200,
                filterSummary: 'filter',
                calculatedAt: 'calculated',
                alertThreshold: 0,
                enabled: false
            }],
            summary: { count: 0, periodStart: 'summary-start', periodEnd: 'summary-end' }
        });
        expect(history.items[0]).toEqual(expect.objectContaining({
            id: '0',
            budgetId: '0',
            subCategory: 'sub',
            alertThreshold: 0,
            enabled: false
        }));

        expect(mapBudgetRequestToRest(null)).toEqual(expect.objectContaining({
            name: '',
            category: '',
            amount_cents: 0,
            period_type: BudgetPeriodType.Monthly
        }));
        expect(mapBudgetRequestToRest({
            subCategory: 'camel-sub',
            periodType: BudgetPeriodType.Yearly,
            amountCents: -5,
            alertThreshold: 0,
            enabled: false
        })).toEqual(expect.objectContaining({
            sub_category: 'camel-sub',
            period_type: BudgetPeriodType.Yearly,
            amount_cents: -5,
            alert_threshold: 0,
            enabled: false
        }));

        expect(mapImportedBudgetToRest(null)).toEqual(expect.objectContaining({
            amount_cents: 0,
            period_type: BudgetPeriodType.Monthly,
            enabled: true
        }));
        expect(mapImportedBudgetToRest({
            subCategory: 'camel-sub',
            periodType: BudgetPeriodType.Yearly,
            amountCents: 5,
            startDate: 'camel-start',
            endDate: 'camel-end',
            alertThreshold: 0,
            enabled: false
        })).toEqual(expect.objectContaining({
            sub_category: 'camel-sub',
            period_type: BudgetPeriodType.Yearly,
            amount_cents: 5,
            start_date: 'camel-start',
            end_date: 'camel-end',
            alert_threshold: 0,
            enabled: false
        }));
    });
});
