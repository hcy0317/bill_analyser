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

describe('budget service adapters', () => {
    test('builds budget query strings with REST field names and encoded values', () => {
        expect(buildBudgetExecutionQuery({
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Monthly,
            year: 2026,
            month: 5,
            quarter: 2,
            startDate: '2026-05-01',
            endDate: '2026-05-31'
        })).toBe('?budget_type=3&period_type=monthly&year=2026&month=5&quarter=2&start_date=2026-05-01&end_date=2026-05-31');

        expect(buildBudgetListQuery({
            type: BudgetType.Investment,
            periodType: BudgetPeriodType.Yearly,
            enabled: false,
            category: 'fund & ETF'
        })).toBe('?budget_type=5&period_type=yearly&enabled=false&category=fund%20%26%20ETF');

        expect(buildBudgetHistoryQuery({
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Quarterly,
            year: 2026,
            month: 4,
            quarter: 2,
            startDate: '2026-04-01',
            endDate: '2026-06-30',
            budgetId: 'budget 1',
            categoryId: 'cat/2',
            accountIds: ['acc-1', 'acc-2'],
            tagIds: ['tag a', 'tag b']
        })).toBe('?budget_type=3&period_type=quarterly&year=2026&month=4&quarter=2&start_date=2026-04-01&end_date=2026-06-30&budget_id=budget%201&category_id=cat%2F2&account_ids=acc-1%2Cacc-2&tag_ids=tag%20a%2Ctag%20b');

        expect(buildBudgetForecastQuery({
            type: BudgetType.Investment,
            periodType: BudgetPeriodType.Yearly,
            year: 2026,
            month: 12,
            quarter: 4,
            monthsHistory: 6,
            forecastStrategy: 'moving average',
            startDate: '2026-01-01',
            endDate: '2026-12-31'
        })).toBe('?budget_type=5&period_type=yearly&year=2026&month=12&quarter=4&months_history=6&forecast_strategy=moving%20average&start_date=2026-01-01&end_date=2026-12-31');

        expect(buildBudgetExecutionQuery()).toBe('');
    });

    test('maps budget list/detail rows from REST yuan fields to frontend cents', () => {
        const mapped = mapRestBudgetToFrontend({
            id: 7,
            name: 'Food',
            category: 'Daily',
            sub_category: 'Meals',
            category_id: 'cat-1',
            period_type: BudgetPeriodType.Quarterly,
            amount: 123.456,
            spent_amount: 10.4,
            remaining_amount: 113.056,
            execution_rate: 12.5,
            alert_threshold: 90,
            enabled: false,
            type: BudgetType.Investment,
            created_at: 'created',
            updated_at: 'updated',
            category_info: { icon: 'food', color: '#123456' }
        });

        expect(mapped).toEqual(expect.objectContaining({
            id: '7',
            subCategory: 'Meals',
            categoryId: 'cat-1',
            periodType: BudgetPeriodType.Quarterly,
            amount: 12346,
            spentAmount: 1040,
            remainingAmount: 11306,
            executionRate: 12.5,
            alertThreshold: 90,
            enabled: false,
            type: BudgetType.Investment,
            categoryIcon: 'food',
            categoryColor: '#123456'
        }));

        expect(mapRestBudgetToFrontend({ amount: 'invalid-number' }).amount).toBe(0);
    });

    test('maps execution, forecast, and history responses for frontend budget views', () => {
        expect(mapRestExecutionToFrontend({
            summary: { total_budget: 200, total_spent: 210, overall_execution_rate: 105 },
            items: [{
                id: 11,
                category: 'Daily',
                sub_category: 'Meals',
                category_id: 'cat-1',
                category_info: { icon: 'food', color: '#123456' },
                budget_amount: 100,
                spent_amount: 120,
                remaining_amount: -20,
                execution_rate: 120,
                alert_threshold: 80
            }],
            period_start: '2026-05-01',
            period_end: '2026-05-31'
        })).toEqual(expect.objectContaining({
            totalBudget: 20000,
            totalSpent: 21000,
            totalExecutionRate: 105,
            periodStart: '2026-05-01',
            periodEnd: '2026-05-31',
            categories: [expect.objectContaining({
                budgetId: '11',
                categoryName: 'Daily-Meals',
                budgetAmount: 10000,
                spentAmount: 12000,
                remainingAmount: -2000,
                isOverBudget: true,
                alertTriggered: true
            })]
        }));

        expect(mapRestForecastToFrontend({
            summary: {
                forecast_strategy: BudgetForecastStrategy.MovingAverage,
                history_periods: 3,
                avg_backtest_mape: 0.2
            },
            items: [{
                category_info: { id: 'cat-2' },
                category: 'Transport',
                forecast_amount: 88.88,
                average_amount: 70,
                periods: [{ period: '2026-04', amount: 50 }, { period: '2026-05', amount: 75.5 }],
                budget_amount: 80,
                trend: 'up',
                projected_over_budget: true,
                sample_periods: 3,
                strategy_explanation: 'trend',
                backtest_mape: 0.1,
                confidence: 'high'
            }],
            period_start: '2026-05-01',
            period_end: '2026-05-31',
            daysRemaining: 5,
            daysElapsed: 26
        })).toEqual(expect.objectContaining({
            forecastStrategy: BudgetForecastStrategy.MovingAverage,
            historyPeriods: 3,
            avgBacktestMape: 0.2,
            forecasts: [expect.objectContaining({
                categoryId: 'cat-2',
                historicalAverage: 7000,
                currentSpent: 7550,
                projectedTotal: 8888,
                budgetAmount: 8000,
                projectedOverBudget: true,
                confidence: 'high',
                periods: [
                    { period: '2026-04', amount: 5000 },
                    { period: '2026-05', amount: 7550 }
                ]
            })]
        }));

        const history = mapRestHistoryToFrontend({
            summary: { count: 3, period_start: '2026-01-01', period_end: '2026-03-31' },
            items: [
                { id: 1, budget_id: 10, budget_type: 'investment', category: 'Fund', period_type: 'yearly', budget_amount: 500, spent_amount: 100, remaining_amount: 400, execution_rate: 20 },
                { id: 2, budgetId: 20, type: 1, category: 'Current', budgetAmount: 1, spentAmount: 0.5, remainingAmount: 0.5, executionRate: 50 },
                { id: 3, budgetId: 30, type: 'unknown', category: 'Unknown' },
                { id: 4, budgetId: 40, budget_type: BudgetType.Expense, category: 'Direct enum' },
                { id: 5, budgetId: 50, budget_type: 'expense', category: 'Expense text' },
                { id: 6, budgetId: 60, budget_type: 5, category: 'Investment number' }
            ]
        });

        expect(history.count).toBe(3);
        expect(history.items[0]).toEqual(expect.objectContaining({ type: BudgetType.Investment, budgetAmount: 50000 }));
        expect(history.items[1]).toEqual(expect.objectContaining({ type: BudgetType.Expense, budgetAmount: 100, spentAmount: 50 }));
        expect(history.items[2]).not.toHaveProperty('type');
        expect(history.items[3]).toEqual(expect.objectContaining({ type: BudgetType.Expense }));
        expect(history.items[4]).toEqual(expect.objectContaining({ type: BudgetType.Expense }));
        expect(history.items[5]).toEqual(expect.objectContaining({ type: BudgetType.Investment }));
    });

    test('maps budget mutation and import payloads back to REST yuan fields', () => {
        expect(mapBudgetRequestToRest({
            name: 'Groceries',
            category: 'Daily',
            subCategory: 'Food',
            periodType: BudgetPeriodType.Monthly,
            amount: 12345,
            startDate: '2026-05-01',
            endDate: '2026-05-31'
        })).toEqual({
            name: 'Groceries',
            category: 'Daily',
            sub_category: 'Food',
            period_type: BudgetPeriodType.Monthly,
            amount: 123.45,
            start_date: '2026-05-01',
            end_date: '2026-05-31',
            alert_threshold: 80,
            enabled: true
        });

        expect(mapImportedBudgetToRest({
            name: 'Frontend import',
            category: 'Daily',
            subCategory: 'Food',
            periodType: BudgetPeriodType.Monthly,
            amount: 1000,
            startDate: '2026-05-01'
        })).toEqual(expect.objectContaining({
            sub_category: 'Food',
            period_type: BudgetPeriodType.Monthly,
            amount: 10,
            start_date: '2026-05-01'
        }));

        expect(mapImportedBudgetToRest({
            name: 'REST import',
            category: 'Daily',
            sub_category: 'Food',
            period_type: BudgetPeriodType.Yearly,
            amount: 10
        })).toEqual(expect.objectContaining({
            sub_category: 'Food',
            period_type: BudgetPeriodType.Yearly,
            amount: 10
        }));
    });
});
