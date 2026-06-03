import { describe, expect, test } from '@jest/globals';

import {
    BudgetType,
    BudgetPeriodType,
    type BudgetHistoryItem
} from '@/models/budget.ts';
import {
    buildHistoricalBudgetPeriodGroups,
    filterHistoricalBudgetItemsByType
} from '@/views/desktop/budgets/historyGrouping.ts';

function createHistoryItem(overrides: Partial<BudgetHistoryItem>): BudgetHistoryItem {
    return {
        id: overrides.id || 'item-1',
        budgetId: overrides.budgetId || 'budget-1',
        name: overrides.name || '预算',
        type: overrides.type,
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
    test('groups historical budget items at the secondary level and sorts newest periods first', () => {
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
            uncategorizedLabel: '未分类',
            levelMode: 'secondary',
            legendSelection: {},
            categoryMeta: {},
            fallbackPalette: ['#5470c6']
        });

        expect(groups.map(group => group.key)).toStrictEqual(['2026-05', '2026-04']);
        expect(groups[0]).toMatchObject({
            totalBudget: 18000,
            totalSpent: 7000,
            totalExecutionRate: 38.9
        });
        const may = groups[0]!;
        const primaryRows = may.rows.map(row => row.displayCategory);
        expect(primaryRows).toContain('餐饮');
        expect(primaryRows).toContain('交通');
        const dining = may.rows.find(row => row.primaryCategory === '餐饮')!;
        expect(dining.childRows.map(child => child.displayCategory)).toStrictEqual(['早餐']);
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
            uncategorizedLabel: '未分类',
            levelMode: 'primary',
            legendSelection: {},
            categoryMeta: {},
            fallbackPalette: ['#5470c6']
        });

        expect(groups).toHaveLength(1);
        expect(groups[0]).toMatchObject({
            key: '2026-Q2',
            totalBudget: 25000,
            totalSpent: 7000,
            totalExecutionRate: 28
        });
        expect(groups[0]?.rows.map(row => row.displayCategory)).toStrictEqual(['餐饮']);
    });

    test('respects legend selection by hiding deselected secondary categories', () => {
        const groups = buildHistoricalBudgetPeriodGroups({
            items: [
                createHistoryItem({
                    id: 'breakfast',
                    category: '餐饮',
                    subCategory: '早餐',
                    budgetAmount: 12000,
                    spentAmount: 4000
                }),
                createHistoryItem({
                    id: 'lunch',
                    category: '餐饮',
                    subCategory: '午餐',
                    budgetAmount: 8000,
                    spentAmount: 6000
                })
            ],
            aggregationType: BudgetPeriodType.Monthly,
            fiscalYearStartMonth: 1,
            fiscalYearStartDay: 1,
            uncategorizedLabel: '未分类',
            levelMode: 'secondary',
            legendSelection: { '餐饮::午餐': false },
            categoryMeta: {},
            fallbackPalette: ['#5470c6']
        });

        const dining = groups[0]?.rows.find(row => row.primaryCategory === '餐饮');
        expect(dining?.childRows.map(child => child.displayCategory)).toStrictEqual(['早餐']);
    });

    test('hides a primary row when every existing secondary is hidden by legend selection', () => {
        const groups = buildHistoricalBudgetPeriodGroups({
            items: [
                createHistoryItem({
                    id: 'primary',
                    category: '餐饮',
                    budgetAmount: 30000,
                    spentAmount: 10000
                }),
                createHistoryItem({
                    id: 'breakfast',
                    category: '餐饮',
                    subCategory: '早餐',
                    budgetAmount: 12000,
                    spentAmount: 4000
                }),
                createHistoryItem({
                    id: 'lunch',
                    category: '餐饮',
                    subCategory: '午餐',
                    budgetAmount: 8000,
                    spentAmount: 6000
                })
            ],
            aggregationType: BudgetPeriodType.Monthly,
            fiscalYearStartMonth: 1,
            fiscalYearStartDay: 1,
            uncategorizedLabel: '未分类',
            levelMode: 'secondary',
            legendSelection: {
                '餐饮::早餐': false,
                '餐饮::午餐': false
            },
            categoryMeta: {},
            fallbackPalette: ['#5470c6']
        });

        expect(groups).toStrictEqual([]);
    });

    test('keeps primary-only history rows visible when their fallback legend item is selected', () => {
        const groups = buildHistoricalBudgetPeriodGroups({
            items: [
                createHistoryItem({
                    id: 'primary',
                    category: '交通',
                    budgetAmount: 6000,
                    spentAmount: 0
                })
            ],
            aggregationType: BudgetPeriodType.Monthly,
            fiscalYearStartMonth: 1,
            fiscalYearStartDay: 1,
            uncategorizedLabel: '未分类',
            levelMode: 'secondary',
            legendSelection: {},
            categoryMeta: {},
            fallbackPalette: ['#91cc75']
        });

        expect(groups).toHaveLength(1);
        expect(groups[0]?.rows).toStrictEqual([
            expect.objectContaining({
                primaryCategory: '交通',
                budgetAmount: 6000,
                spentAmount: 0,
                childRows: []
            })
        ]);
    });

    test('filters historical items by budget type when the response includes type metadata', () => {
        const expense = createHistoryItem({
            id: 'expense',
            type: BudgetType.Expense,
            category: '餐饮'
        });
        const investment = createHistoryItem({
            id: 'investment',
            type: BudgetType.Investment,
            category: '基金'
        });
        const uncategorizedUnknown = createHistoryItem({
            id: 'unknown',
            type: undefined,
            category: '交通'
        });

        expect(filterHistoricalBudgetItemsByType([
            expense,
            investment,
            uncategorizedUnknown
        ], BudgetType.Expense).map(item => item.id)).toStrictEqual(['expense', 'unknown']);
    });
});
