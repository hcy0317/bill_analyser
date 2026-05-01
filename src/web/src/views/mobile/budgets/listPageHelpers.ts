/**
 * Mobile budgets list — small pure helpers that the page template uses.
 *
 * Kept in a standalone .ts module so they can be unit-tested without mounting
 * Framework7 / Vue (the repo's jest harness is ts-jest in node and does not
 * compile .vue SFCs).
 */

import { Budget, BudgetType } from '@/models/budget.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';

export interface MobileBudgetGroup {
    category: string;
    categoryIcon: string;
    categoryColor: string;
    primaryBudgets: Budget[];
    subBudgets: Budget[];
    totalAmount: number;
    totalSpent: number;
    primaryAmount: number;
    primarySpent: number;
    subTotalAmount: number;
    subTotalSpent: number;
    isCollapsed: boolean;
}

/**
 * Format a yuan-denominated amount as the same '¥X.XX' string the desktop
 * budgets page uses. Intentionally identical to the desktop helper so both
 * surfaces present amounts the same way (S4 must not introduce a new
 * conversion path between yuan and cents).
 */
export function formatBudgetAmount(amountInYuan: number): string {
    return '¥' + amountInYuan.toFixed(2);
}

/**
 * Visual progress for f7-progressbar (0..100, capped). The actual ratio text
 * (which can exceed 100% when over budget) is rendered separately via
 * Budget.executionRateText.
 */
export function getBudgetProgressPercent(budget: Budget): number {
    if (!budget || !budget.amount || budget.amount <= 0) {
        return 0;
    }

    const ratio = (budget.spentAmount / budget.amount) * 100;

    if (!Number.isFinite(ratio) || ratio < 0) {
        return 0;
    }

    return ratio > 100 ? 100 : ratio;
}

export function getBudgetGroupProgressPercent(group: MobileBudgetGroup): number {
    if (!group || group.totalAmount <= 0) {
        return 0;
    }

    const ratio = (group.totalSpent / group.totalAmount) * 100;
    if (!Number.isFinite(ratio) || ratio < 0) {
        return 0;
    }

    return ratio > 100 ? 100 : ratio;
}

export function getBudgetGroupExecutionRate(group: MobileBudgetGroup): number {
    if (!group || group.totalAmount <= 0) {
        return 0;
    }

    const ratio = (group.totalSpent / group.totalAmount) * 100;
    return Number.isFinite(ratio) && ratio >= 0 ? ratio : 0;
}

export function buildMobileBudgetGroups({
    budgets,
    primaryCategories,
    collapsedCategories
}: {
    budgets: Budget[],
    primaryCategories: TransactionCategory[],
    collapsedCategories: Set<string>
}): MobileBudgetGroup[] {
    const groups = new Map<string, MobileBudgetGroup>();
    const primaryCategoryByName: Record<string, TransactionCategory> = {};

    for (const category of primaryCategories) {
        primaryCategoryByName[category.name] = category;
    }

    for (const budget of budgets) {
        const categoryKey = budget.category || budget.name || budget.fullCategoryName;
        if (!categoryKey) {
            continue;
        }

        if (!groups.has(categoryKey)) {
            const primaryCategory = primaryCategoryByName[categoryKey];
            groups.set(categoryKey, {
                category: categoryKey,
                categoryIcon: primaryCategory?.icon || budget.categoryIcon || '',
                categoryColor: primaryCategory?.color || budget.categoryColor || '',
                primaryBudgets: [],
                subBudgets: [],
                totalAmount: 0,
                totalSpent: 0,
                primaryAmount: 0,
                primarySpent: 0,
                subTotalAmount: 0,
                subTotalSpent: 0,
                isCollapsed: collapsedCategories.has(categoryKey)
            });
        }

        const group = groups.get(categoryKey)!;
        if (!budget.subCategory) {
            group.primaryBudgets.push(budget);
            group.primaryAmount += budget.amount;
            group.primarySpent += budget.spentAmount;
        } else {
            group.subBudgets.push(budget);
            group.subTotalAmount += budget.amount;
            group.subTotalSpent += budget.spentAmount;
        }

        if (!group.categoryIcon && budget.categoryIcon) {
            group.categoryIcon = budget.categoryIcon;
        }
        if (!group.categoryColor && budget.categoryColor) {
            group.categoryColor = budget.categoryColor;
        }
    }

    for (const group of groups.values()) {
        if (group.primaryBudgets.length > 1) {
            group.totalAmount = group.primaryAmount + group.subTotalAmount;
            group.totalSpent = group.primarySpent + group.subTotalSpent;
        } else if (group.primaryBudgets.length > 0) {
            group.totalAmount = group.primaryAmount;
            group.totalSpent = group.primarySpent > 0 ? group.primarySpent : group.subTotalSpent;
        } else {
            group.totalAmount = group.subTotalAmount;
            group.totalSpent = group.subTotalSpent;
        }

        group.subBudgets.sort((a, b) => (a.subCategory || a.name).localeCompare(b.subCategory || b.name));
    }

    return Array.from(groups.values()).sort((a, b) => a.category.localeCompare(b.category));
}

/**
 * Pick the in-period budgets matching the active type tab. Read-only —
 * delegates filtering to two existing store getters (expenseBudgets /
 * investmentBudgets) so S4 introduces no new store surface.
 */
export function selectBudgetsByType(
    type: BudgetType,
    expenseBudgets: Budget[],
    investmentBudgets: Budget[]
): Budget[] {
    return type === BudgetType.Expense ? expenseBudgets : investmentBudgets;
}
