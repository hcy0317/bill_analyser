import { BudgetPeriodType } from '@/models/budget.ts';

export type BudgetRelativePeriodScope = 'current' | 'previous';

export function getBudgetPeriodTypeFromFilter(filter: string): BudgetPeriodType {
    switch (filter) {
        case 'thisQuarter':
        case 'lastQuarter':
            return BudgetPeriodType.Quarterly;
        case 'thisYear':
        case 'lastYear':
            return BudgetPeriodType.Yearly;
        default:
            return BudgetPeriodType.Monthly;
    }
}

export function getBudgetRelativeScopeFromFilter(filter: string): BudgetRelativePeriodScope {
    return ['lastMonth', 'lastQuarter', 'lastYear'].includes(filter) ? 'previous' : 'current';
}

export function toBudgetRelativePeriodFilter(
    periodType: BudgetPeriodType,
    scope: BudgetRelativePeriodScope
): string {
    if (periodType === BudgetPeriodType.Quarterly) {
        return scope === 'previous' ? 'lastQuarter' : 'thisQuarter';
    }

    if (periodType === BudgetPeriodType.Yearly) {
        return scope === 'previous' ? 'lastYear' : 'thisYear';
    }

    return scope === 'previous' ? 'lastMonth' : 'thisMonth';
}
