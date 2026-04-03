import { describe, expect, test } from '@jest/globals';

import {
    BudgetForecastStrategy,
    BudgetPeriodType,
    BudgetType
} from '@/models/budget.ts';
import { buildBudgetForecastLoadRequest } from '@/views/desktop/budgets/forecastRequest.ts';

describe('forecastRequest helpers', () => {
    test('forwards year/month selectors into forecast requests', () => {
        const request = buildBudgetForecastLoadRequest({
            budgetType: BudgetType.Expense,
            periodRequest: {
                periodType: BudgetPeriodType.Monthly,
                year: 2026,
                month: 2
            },
            monthsHistory: 6,
            forecastStrategy: BudgetForecastStrategy.HistoricalAverage
        });

        expect(request).toStrictEqual({
            type: BudgetType.Expense,
            periodType: BudgetPeriodType.Monthly,
            year: 2026,
            month: 2,
            quarter: undefined,
            monthsHistory: 6,
            forecastStrategy: BudgetForecastStrategy.HistoricalAverage
        });
    });

    test('does not forward arbitrary explicit range into forecast shortcut requests', () => {
        const request = buildBudgetForecastLoadRequest({
            budgetType: BudgetType.Investment,
            periodRequest: {
                periodType: BudgetPeriodType.Quarterly,
                startDate: '2026-04-01',
                endDate: '2026-06-30'
            },
            monthsHistory: 3,
            forecastStrategy: BudgetForecastStrategy.MovingAverage
        });

        expect(request).toStrictEqual({
            type: BudgetType.Investment,
            periodType: BudgetPeriodType.Quarterly,
            year: undefined,
            month: undefined,
            quarter: undefined,
            monthsHistory: 3,
            forecastStrategy: BudgetForecastStrategy.MovingAverage
        });

        expect(request).not.toHaveProperty('startDate');
        expect(request).not.toHaveProperty('endDate');
    });
});