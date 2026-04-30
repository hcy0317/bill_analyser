import { describe, expect, test } from '@jest/globals';

import {
    BudgetPeriodType,
    type BudgetHistoryItem
} from '@/models/budget.ts';
import {
    buildHistoricalBudgetPeriodGroups
} from '@/views/desktop/budgets/historyGrouping.ts';

function createHistoryItem(overrides: Partial<BudgetHistoryItem>): BudgetHistoryItem {
    return {
        id: overrides.id || 'item-1',
        budgetId: overrides.budgetId || 'budget-1',
        name: overrides.name || '预算',
        category: overrides.category || '餐饮',
        subCategory: overrides.subCategory || '',
        periodType: overrides.periodType || BudgetPeriodType.Monthly,
        periodStart: overrides.periodStart || '2026-05-01',
        periodEnd: overrides.periodEnd || '2026-05-31',
        budgetAmount: overrides.budgetAmount ?? 10000,
        spentAmount: overrides.spentAmount ?? 5000,
        remainingAmount: overrides.remainingAmount ?? 5000,
        executionRate: overrides.executionRate ?? 50,
        status: overrides.status || 'active',
        filterSummary: overrides.filterSummary || '',
        calculatedAt: overrides.calculatedAt || '2026-05-31T00:00:00',
        alertThreshold: overrides.alertThreshold ?? 80,
        enabled: overrides.enabled ?? true
    };
}

describe('historyGrouping helpers', () => {
    test('groups historical budget items by month and sorts newest periods first', () => {
        const groups = buildHistoricalBudgetPeriodGroups({
            items: [
                createHistoryItem({
                    id: 'may-breakfast',
                    category: '餐饮',
                    subCategory: '早餐',
                    periodStart: '2026-05-01',
                    periodEnd: '2026-05-31',
                    budgetAmount: 12000,
                    spentAmount: 4000,
                    remainingAmount: 8000,
                    executionRate: 33.3
                }),
                createHistoryItem({
                    id: 'may-traffic',
                    category: '交通',
                    periodStart: '2026-05-01',
                    periodEnd: '2026-05-31',
                    budgetAmount: 6000,
                    spentAmount: 3000,
                    remainingAmount: 3000,
                    executionRate: 50
                }),
                createHistoryItem({
                    id: 'apr-lunch',
                    category: '餐饮',
                    subCategory: '午餐',
                    periodStart: '2026-04-01',
                    periodEnd: '2026-04-30',
                    budgetAmount: 10000,
                    spentAmount: 6000,
                    remainingAmount: 4000,
                    executionRate: 60
                })
            ],
            aggregationType: BudgetPeriodType.Monthly,
            fiscalYearStartMonth: 1,
            fiscalYearStartDay: 1,
            uncategorizedLabel: '未分类'
        });

        expect(groups.map(group => group.key)).toStrictEqual(['2026-05', '2026-04']);
        expect(groups[0]).toMatchObject({
            totalBudget: 18000,
            totalSpent: 7000,
            totalExecutionRate: 38.9,
            itemCount: 2
        });
        expect(groups[0]?.items.map(item => item.displayCategory)).toStrictEqual(['餐饮 / 早餐', '交通']);
    });

    test('rolls monthly snapshots into quarterly groups', () => {
        const groups = buildHistoricalBudgetPeriodGroups({
            items: [
                createHistoryItem({
                    id: 'apr',
                    periodStart: '2026-04-01',
                    periodEnd: '2026-04-30',
                    budgetAmount: 10000,
                    spentAmount: 2000,
                    remainingAmount: 8000
                }),
                createHistoryItem({
                    id: 'may',
                    periodStart: '2026-05-01',
                    periodEnd: '2026-05-31',
                    budgetAmount: 15000,
                    spentAmount: 5000,
                    remainingAmount: 10000
                })
            ],
            aggregationType: BudgetPeriodType.Quarterly,
            fiscalYearStartMonth: 1,
            fiscalYearStartDay: 1,
            uncategorizedLabel: '未分类'
        });

        expect(groups).toHaveLength(1);
        expect(groups[0]).toMatchObject({
            key: '2026-Q2',
            totalBudget: 25000,
            totalSpent: 7000,
            totalExecutionRate: 28
        });
    });
});
