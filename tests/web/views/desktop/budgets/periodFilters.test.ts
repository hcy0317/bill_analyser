import { describe, expect, test } from '@jest/globals';

import { BudgetPeriodType } from '@/models/budget.ts';
import {
    getBudgetPeriodTypeFromFilter,
    getBudgetRelativeScopeFromFilter,
    toBudgetRelativePeriodFilter
} from '@/views/desktop/budgets/periodFilters.ts';

describe('periodFilters helpers', () => {
    test('derives the period type from current and previous filters', () => {
        expect(getBudgetPeriodTypeFromFilter('thisMonth')).toBe(BudgetPeriodType.Monthly);
        expect(getBudgetPeriodTypeFromFilter('lastQuarter')).toBe(BudgetPeriodType.Quarterly);
        expect(getBudgetPeriodTypeFromFilter('thisYear')).toBe(BudgetPeriodType.Yearly);
        expect(getBudgetPeriodTypeFromFilter('custom')).toBe(BudgetPeriodType.Monthly);
    });

    test('derives the relative scope from filter names', () => {
        expect(getBudgetRelativeScopeFromFilter('thisMonth')).toBe('current');
        expect(getBudgetRelativeScopeFromFilter('lastMonth')).toBe('previous');
        expect(getBudgetRelativeScopeFromFilter('lastYear')).toBe('previous');
    });

    test('maps period type and scope back to concrete filter values', () => {
        expect(toBudgetRelativePeriodFilter(BudgetPeriodType.Monthly, 'current')).toBe('thisMonth');
        expect(toBudgetRelativePeriodFilter(BudgetPeriodType.Monthly, 'previous')).toBe('lastMonth');
        expect(toBudgetRelativePeriodFilter(BudgetPeriodType.Quarterly, 'current')).toBe('thisQuarter');
        expect(toBudgetRelativePeriodFilter(BudgetPeriodType.Quarterly, 'previous')).toBe('lastQuarter');
        expect(toBudgetRelativePeriodFilter(BudgetPeriodType.Yearly, 'current')).toBe('thisYear');
        expect(toBudgetRelativePeriodFilter(BudgetPeriodType.Yearly, 'previous')).toBe('lastYear');
    });
});
