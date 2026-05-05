import type { Budget } from '@/models/budget.ts';

export type HistoricalBudgetLevel = 'primary' | 'secondary';

export interface BudgetGroup {
    category: string;
    categoryIcon: string;
    categoryColor: string;
    primaryBudgets: Budget[];
    subBudgets: Budget[];
    totalAmount: number;
    totalSpent: number;
    subTotalAmount: number;
    subTotalSpent: number;
    primaryAmount: number;
    primarySpent: number;
    isCollapsed: boolean;
}
