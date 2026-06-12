import type { Budget } from '@/models/budget.ts';

export type HistoricalBudgetLevel = 'primary' | 'secondary';

export interface BudgetGroup {
    category: string;
    categoryIcon: string;
    categoryColor: string;
    primaryBudgets: Budget[];
    subBudgets: Budget[];
    totalAmountCents: number;
    totalSpentCents: number;
    subTotalAmountCents: number;
    subTotalSpentCents: number;
    primaryAmountCents: number;
    primarySpentCents: number;
    isCollapsed: boolean;
}
