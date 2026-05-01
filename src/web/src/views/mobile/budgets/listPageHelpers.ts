/**
 * Mobile budgets list — small pure helpers that the page template uses.
 *
 * Kept in a standalone .ts module so they can be unit-tested without mounting
 * Framework7 / Vue (the repo's jest harness is ts-jest in node and does not
 * compile .vue SFCs).
 */

import { Budget, BudgetType } from '@/models/budget.ts';

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
