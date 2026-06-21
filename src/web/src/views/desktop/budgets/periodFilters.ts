import { BudgetPeriodType } from '@/models/budget.ts';

export type BudgetRelativePeriodScope = 'current' | 'previous';

/**
 * 从预算周期筛选值中解析后端预算周期类型。
 */
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

/**
 * 从预算周期筛选值中识别当前周期或上一周期范围。
 */
export function getBudgetRelativeScopeFromFilter(filter: string): BudgetRelativePeriodScope {
    return ['lastMonth', 'lastQuarter', 'lastYear'].includes(filter) ? 'previous' : 'current';
}

/**
 * 将周期类型和相对范围组合为页面筛选值。
 */
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
