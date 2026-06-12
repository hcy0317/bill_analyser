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

    test('maps budget list/detail rows from REST cents fields to frontend cents', () => {
        const mapped = mapRestBudgetToFrontend({
            id: 7,
            name: 'Food',
            category: 'Daily',
            sub_category: 'Meals',
            category_id: 'cat-1',
            period_type: BudgetPeriodType.Quarterly,
            amount_cents: 12346,
            spent_amount_cents: 1040,
            remaining_amount_cents: 11306,
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
            amountCents: 12346,
            spentAmountCents: 1040,
            remainingAmountCents: 11306,
            executionRate: 12.5,
            alertThreshold: 90,
            enabled: false,
            type: BudgetType.Investment,
            categoryIcon: 'food',
            categoryColor: '#123456'
        }));

        expect(mapRestBudgetToFrontend({ amount_cents: 'invalid-number' }).amountCents).toBe(0);
        expect(mapRestBudgetToFrontend({ amount_cents: true }).amountCents).toBe(0);
        expect(mapRestBudgetToFrontend({ amount_cents: 12.34 }).amountCents).toBe(0);
        expect(mapRestBudgetToFrontend({ amount_cents: '1234' }).amountCents).toBe(1234);
        expect(mapRestBudgetToFrontend({ amount_cents: '1234.0' }).amountCents).toBe(0);
    });

    test('maps execution, forecast, and history responses for frontend budget views', () => {
        expect(mapRestExecutionToFrontend({
            summary: { total_budget_cents: 20000, total_spent_cents: 21000, overall_execution_rate: 105 },
            items: [{
                id: 11,
                category: 'Daily',
                sub_category: 'Meals',
                category_id: 'cat-1',
                category_info: { icon: 'food', color: '#123456' },
                budget_amount_cents: 10000,
                spent_amount_cents: 12000,
                remaining_amount_cents: -2000,
                execution_rate: 120,
                alert_threshold: 80
            }],
            period_start: '2026-05-01',
            period_end: '2026-05-31'
        })).toEqual(expect.objectContaining({
            totalBudgetCents: 20000,
            totalSpentCents: 21000,
            totalExecutionRate: 105,
            periodStart: '2026-05-01',
            periodEnd: '2026-05-31',
            categories: [expect.objectContaining({
                budgetId: '11',
                categoryName: 'Daily-Meals',
                budgetAmountCents: 10000,
                spentAmountCents: 12000,
                remainingAmountCents: -2000,
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
                forecast_amount_cents: 8888,
                average_amount_cents: 7000,
                periods: [{ period: '2026-04', amount_cents: 5000 }, { period: '2026-05', amount_cents: 7550 }],
                budget_amount_cents: 8000,
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
                historicalAverageCents: 7000,
                currentSpentCents: 7550,
                projectedTotalCents: 8888,
                budgetAmountCents: 8000,
                projectedOverBudget: true,
                confidence: 'high',
                periods: [
                    { period: '2026-04', amountCents: 5000 },
                    { period: '2026-05', amountCents: 7550 }
                ]
            })]
        }));

        const history = mapRestHistoryToFrontend({
            summary: { count: 3, period_start: '2026-01-01', period_end: '2026-03-31' },
            items: [
                { id: 1, budget_id: 10, budget_type: 'investment', category: 'Fund', period_type: 'yearly', budget_amount_cents: 50000, spent_amount_cents: 10000, remaining_amount_cents: 40000, execution_rate: 20 },
                { id: 2, budgetId: 20, type: 1, category: 'Current', budgetAmountCents: 100, spentAmountCents: 50, remainingAmountCents: 50, executionRate: 50 },
                { id: 3, budgetId: 30, type: 'unknown', category: 'Unknown' },
                { id: 4, budgetId: 40, budget_type: BudgetType.Expense, category: 'Direct enum' },
                { id: 5, budgetId: 50, budget_type: 'expense', category: 'Expense text' },
                { id: 6, budgetId: 60, budget_type: 5, category: 'Investment number' }
            ]
        });

        expect(history.count).toBe(3);
        expect(history.items[0]).toEqual(expect.objectContaining({ type: BudgetType.Investment, budgetAmountCents: 50000 }));
        expect(history.items[1]).toEqual(expect.objectContaining({ type: BudgetType.Expense, budgetAmountCents: 100, spentAmountCents: 50 }));
        expect(history.items[2]).not.toHaveProperty('type');
        expect(history.items[3]).toEqual(expect.objectContaining({ type: BudgetType.Expense }));
        expect(history.items[4]).toEqual(expect.objectContaining({ type: BudgetType.Expense }));
        expect(history.items[5]).toEqual(expect.objectContaining({ type: BudgetType.Investment }));
    });

    test('maps budget mutation and import payloads back to REST cents fields', () => {
        expect(mapBudgetRequestToRest({
            name: 'Groceries',
            category: 'Daily',
            subCategory: 'Food',
            periodType: BudgetPeriodType.Monthly,
            amountCents: 12345,
            startDate: '2026-05-01',
            endDate: '2026-05-31'
        })).toEqual({
            name: 'Groceries',
            category: 'Daily',
            sub_category: 'Food',
            period_type: BudgetPeriodType.Monthly,
            amount_cents: 12345,
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
            amountCents: 1000,
            startDate: '2026-05-01'
        })).toEqual(expect.objectContaining({
            sub_category: 'Food',
            period_type: BudgetPeriodType.Monthly,
            amount_cents: 1000,
            start_date: '2026-05-01'
        }));

        expect(mapImportedBudgetToRest({
            name: 'REST import',
            category: 'Daily',
            sub_category: 'Food',
            period_type: BudgetPeriodType.Yearly,
            amount_cents: 10
        })).toEqual(expect.objectContaining({
            sub_category: 'Food',
            period_type: BudgetPeriodType.Yearly,
            amount_cents: 10
        }));

        expect(mapBudgetRequestToRest({ amountCents: true })).toEqual(expect.objectContaining({
            amount_cents: 0
        }));
        expect(mapImportedBudgetToRest({ amount_cents: 12.34 })).toEqual(expect.objectContaining({
            amount_cents: 0
        }));
    });
});
