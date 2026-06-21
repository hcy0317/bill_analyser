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

export function isBudgetViewMode(value: string): value is BudgetViewMode {
    return value === 'budget' || value === 'forecast' || value === 'history';
}
