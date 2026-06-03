import { describe, expect, test } from '@jest/globals';

import {
    DEFAULT_BUDGET_ALERT_THRESHOLD,
    DEFAULT_BUDGET_ENABLED
} from '@/config/budget.ts';

import {
    Budget,
    BudgetPeriodType,
    BudgetPeriodTypeNames,
    BudgetType,
    BudgetTypeNames,
    type BudgetCategoryExecution,
    type BudgetInfoResponse
} from '@/models/budget.ts';

const SAMPLE_RESPONSE: BudgetInfoResponse = {
    id: 'b-1',
    name: '餐饮预算',
    category: '餐饮',
    subCategory: '早餐',
    categoryId: 'c-1',
    periodType: BudgetPeriodType.Monthly,
    amount: 120000,
    startDate: '2026-03-01',
    endDate: '2026-03-31',
    alertThreshold: 75,
    enabled: true,
    type: BudgetType.Expense,
    createdAt: '2026-03-01T00:00:00Z',
    updatedAt: '2026-03-15T00:00:00Z',
    spentAmount: 90000,
    remainingAmount: 30000,
    executionRate: 75,
    categoryIcon: 'las la-utensils',
    categoryColor: '#5470c6'
};

describe('Budget model', () => {
    test('Budget.of maps API fields and computes runtime flags', () => {
        const budget = Budget.of(SAMPLE_RESPONSE);

        expect(budget.id).toBe('b-1');
        expect(budget.name).toBe('餐饮预算');
        expect(budget.fullCategoryName).toBe('餐饮-早餐');
        expect(budget.executionRateText).toBe('75.0%');
        expect(budget.amountInYuan).toBe(1200);
        expect(budget.spentAmountInYuan).toBe(900);
        expect(budget.remainingAmountInYuan).toBe(300);
        expect(budget.isOverBudget).toBe(false);
        expect(budget.alertTriggered).toBe(true);
        expect(budget.categoryIcon).toBe('las la-utensils');
        expect(budget.categoryColor).toBe('#5470c6');
    });

    test('Budget.of applies defaults and marks over-budget cases', () => {
        const budget = Budget.of({
            id: 'b-2',
            name: '',
            category: '交通',
            subCategory: '',
            periodType: BudgetPeriodType.Yearly,
            amount: 100,
            startDate: '',
            endDate: '',
            alertThreshold: 0,
            enabled: false,
            type: BudgetType.Investment,
            spentAmount: 160,
            remainingAmount: -60,
            executionRate: 160
        });

        expect(budget.name).toBe('');
        expect(budget.categoryId).toBe('');
        expect(budget.fullCategoryName).toBe('交通');
        expect(budget.alertThreshold).toBe(0);
        expect(budget.enabled).toBe(false);
        expect(budget.isOverBudget).toBe(true);
        expect(budget.alertTriggered).toBe(true);
    });

    test('Budget.of falls back to safe defaults when optional response fields are missing', () => {
        const budget = Budget.of({
            id: '',
            name: undefined,
            category: undefined,
            subCategory: undefined,
            categoryId: undefined,
            periodType: undefined,
            amount: undefined,
            startDate: undefined,
            endDate: undefined,
            alertThreshold: undefined,
            enabled: undefined,
            type: undefined,
            createdAt: undefined,
            updatedAt: undefined,
            spentAmount: undefined,
            remainingAmount: undefined,
            executionRate: undefined,
            categoryIcon: undefined,
            categoryColor: undefined
        } as unknown as BudgetInfoResponse);

        expect(budget.id).toBe('');
        expect(budget.name).toBe('');
        expect(budget.category).toBe('');
        expect(budget.subCategory).toBe('');
        expect(budget.categoryId).toBe('');
        expect(budget.periodType).toBe(BudgetPeriodType.Monthly);
        expect(budget.amount).toBe(0);
        expect(budget.startDate).toBe('');
        expect(budget.endDate).toBe('');
        expect(budget.alertThreshold).toBe(DEFAULT_BUDGET_ALERT_THRESHOLD);
        expect(budget.enabled).toBe(DEFAULT_BUDGET_ENABLED);
        expect(budget.type).toBe(BudgetType.Expense);
        expect(budget.createdAt).toBe('');
        expect(budget.updatedAt).toBe('');
        expect(budget.spentAmount).toBe(0);
        expect(budget.remainingAmount).toBe(0);
        expect(budget.executionRate).toBe(0);
        expect(budget.categoryIcon).toBe('');
        expect(budget.categoryColor).toBe('');
        expect(budget.isOverBudget).toBe(false);
        expect(budget.alertTriggered).toBe(false);
    });

    test('Budget.ofMulti handles invalid input and maps arrays', () => {
        expect(Budget.ofMulti(undefined as unknown as BudgetInfoResponse[])).toStrictEqual([]);
        expect(Budget.ofMulti([SAMPLE_RESPONSE])).toHaveLength(1);
    });

    test('Budget.createNew seeds defaults for a new budget', () => {
        const expenseBudget = Budget.createNew();
        const investmentBudget = Budget.createNew(BudgetType.Investment);

        expect(expenseBudget.type).toBe(BudgetType.Expense);
        expect(expenseBudget.periodType).toBe(BudgetPeriodType.Monthly);
        expect(expenseBudget.alertThreshold).toBe(DEFAULT_BUDGET_ALERT_THRESHOLD);
        expect(expenseBudget.enabled).toBe(DEFAULT_BUDGET_ENABLED);
        expect(investmentBudget.type).toBe(BudgetType.Investment);
    });

    test('Budget request conversion omits blank optional fields', () => {
        const budget = Budget.createNew();
        budget.id = 'b-3';
        budget.name = '交通预算';
        budget.category = '交通';
        budget.subCategory = '';
        budget.categoryId = '';
        budget.amount = 5000;
        budget.startDate = '';
        budget.endDate = '';

        expect(budget.toCreateRequest()).toStrictEqual({
            name: '交通预算',
            category: '交通',
            subCategory: undefined,
            categoryId: undefined,
            periodType: BudgetPeriodType.Monthly,
            amount: 5000,
            startDate: undefined,
            endDate: undefined,
            alertThreshold: DEFAULT_BUDGET_ALERT_THRESHOLD,
            enabled: DEFAULT_BUDGET_ENABLED,
            type: BudgetType.Expense
        });

        expect(budget.toModifyRequest()).toStrictEqual({
            id: 'b-3',
            name: '交通预算',
            category: '交通',
            subCategory: undefined,
            categoryId: undefined,
            periodType: BudgetPeriodType.Monthly,
            amount: 5000,
            startDate: undefined,
            endDate: undefined,
            alertThreshold: DEFAULT_BUDGET_ALERT_THRESHOLD,
            enabled: DEFAULT_BUDGET_ENABLED,
            type: BudgetType.Expense
        });
    });

    test('Budget.updateExecution refreshes execution fields', () => {
        const budget = Budget.createNew();
        const execution: BudgetCategoryExecution = {
            budgetId: 'b-0',
            categoryId: 'c-2',
            categoryName: '交通-地铁',
            budgetAmount: 5000,
            spentAmount: 6200,
            executionRate: 124,
            remainingAmount: -1200,
            isOverBudget: true,
            alertTriggered: true
        };

        budget.updateExecution(execution);

        expect(budget.spentAmount).toBe(6200);
        expect(budget.executionRate).toBe(124);
        expect(budget.remainingAmount).toBe(-1200);
        expect(budget.isOverBudget).toBe(true);
        expect(budget.alertTriggered).toBe(true);
    });

    test('Budget enum display-name maps stay stable', () => {
        expect(BudgetPeriodTypeNames).toStrictEqual({
            [BudgetPeriodType.Daily]: '日度',
            [BudgetPeriodType.Weekly]: '周度',
            [BudgetPeriodType.Monthly]: '月度',
            [BudgetPeriodType.Quarterly]: '季度',
            [BudgetPeriodType.Yearly]: '年度'
        });
        expect(BudgetTypeNames).toStrictEqual({
            [BudgetType.Expense]: '支出',
            [BudgetType.Investment]: '投资'
        });
    });
});
