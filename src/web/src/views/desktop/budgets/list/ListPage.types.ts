export interface PeriodFilter {
    name: string;
    value: string;
}

export interface RangeFilter {
    label: string;
    value: string;
    min?: number;
    max?: number;
}

export type BudgetViewMode = 'budget' | 'forecast' | 'history';

export interface FilterPreset {
    id: string;
    name: string;
    categoryFilter: string | null;
    accountFilter: string[];
    tagFilter: string[];
    executionRateFilter: RangeFilter | null;
    spentAmountFilterCents: string;
    budgetAmountFilterCents: string;
}

/**
 * 校验路由或持久化状态中的预算视图模式是否属于当前页面支持的集合。
 */
export function isBudgetViewMode(value: string): value is BudgetViewMode {
    return value === 'budget' || value === 'forecast' || value === 'history';
}
