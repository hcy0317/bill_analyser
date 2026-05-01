import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

import { Budget, BudgetType } from '@/models/budget.ts';
import {
    formatBudgetAmount,
    getBudgetProgressPercent,
    selectBudgetsByType
} from '@/views/mobile/budgets/listPageHelpers.ts';

const LIST_PAGE_PATH = 'src/views/mobile/budgets/ListPage.vue';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

function makeBudget(overrides: Partial<Budget> = {}): Budget {
    const b = new Budget();
    b.id = overrides.id ?? 'b-1';
    b.name = overrides.name ?? 'Test Budget';
    b.type = overrides.type ?? BudgetType.Expense;
    b.amount = overrides.amount ?? 0;
    b.spentAmount = overrides.spentAmount ?? 0;
    if (overrides.executionRate !== undefined) {
        b.executionRate = overrides.executionRate;
    }
    return b;
}

describe('mobile budgets ListPage helpers (S4)', () => {
    describe('formatBudgetAmount', () => {
        test('formats yuan amount with ¥ and 2 decimals', () => {
            expect(formatBudgetAmount(0)).toBe('¥0.00');
            expect(formatBudgetAmount(123.4)).toBe('¥123.40');
            expect(formatBudgetAmount(2000)).toBe('¥2000.00');
        });
    });

    describe('getBudgetProgressPercent', () => {
        test('renders progress bar with correct ratio (target=200000 cents, actual=50000 cents → 25%)', () => {
            const budget = makeBudget({ amount: 200000, spentAmount: 50000 });
            expect(getBudgetProgressPercent(budget)).toBe(25);
        });

        test('returns 0 when budget amount is zero or negative', () => {
            expect(getBudgetProgressPercent(makeBudget({ amount: 0, spentAmount: 100 }))).toBe(0);
            expect(getBudgetProgressPercent(makeBudget({ amount: -1, spentAmount: 100 }))).toBe(0);
        });

        test('caps over-budget progress at 100 (text rate still shows real %)', () => {
            const budget = makeBudget({ amount: 100, spentAmount: 250 });
            expect(getBudgetProgressPercent(budget)).toBe(100);
        });

        test('returns 0 for null/undefined safely', () => {
            // Defensive: empty list should not crash if a malformed budget appears.
            expect(getBudgetProgressPercent(null as unknown as Budget)).toBe(0);
        });

        test('handles non-finite ratios gracefully', () => {
            const budget = makeBudget({ amount: 100, spentAmount: Number.NaN });
            expect(getBudgetProgressPercent(budget)).toBe(0);
        });
    });

    describe('selectBudgetsByType', () => {
        const expense = [makeBudget({ id: 'e1', type: BudgetType.Expense })];
        const investment = [makeBudget({ id: 'i1', type: BudgetType.Investment })];

        test('returns expense list when toggle is Expense', () => {
            const out = selectBudgetsByType(BudgetType.Expense, expense, investment);
            expect(out).toBe(expense);
            expect(out.map(b => b.id)).toEqual(['e1']);
        });

        test('returns investment list when toggle is Investment', () => {
            const out = selectBudgetsByType(BudgetType.Investment, expense, investment);
            expect(out).toBe(investment);
            expect(out.map(b => b.id)).toEqual(['i1']);
        });

        test('toggling switches the displayed list (parametric over both branches)', () => {
            for (const type of [BudgetType.Expense, BudgetType.Investment]) {
                const out = selectBudgetsByType(type, expense, investment);
                expect(out.length).toBe(1);
                expect(out[0]!.type).toBe(type);
            }
        });

        test('renders with empty list (no crash)', () => {
            expect(selectBudgetsByType(BudgetType.Expense, [], [])).toEqual([]);
            expect(selectBudgetsByType(BudgetType.Investment, [], [])).toEqual([]);
        });
    });
});

describe('mobile budgets ListPage.vue source contract (S4)', () => {
    const source = readSource(LIST_PAGE_PATH);

    test('uses Framework7 page wiring with PTR refresh handler', () => {
        expect(source).toMatch(/<f7-page[^>]*\bptr\b[^>]*@ptr:refresh="reload"/);
        expect(source).toContain('<f7-navbar>');
    });

    test('exposes a BudgetType segmented toggle (Expense + Investment)', () => {
        // The toggle must reference both BudgetType branches and switch via
        // switchBudgetType — toggling between the two updates filteredBudgets.
        expect(source).toContain('<f7-segmented');
        expect(source).toMatch(/:active="activeBudgetType === BudgetType\.Expense"/);
        expect(source).toMatch(/:active="activeBudgetType === BudgetType\.Investment"/);
        expect(source).toMatch(/switchBudgetType\(BudgetType\.Expense\)/);
        expect(source).toMatch(/switchBudgetType\(BudgetType\.Investment\)/);
    });

    test('reads from the budget store (no new store methods, no new conversion)', () => {
        expect(source).toMatch(/import\s*\{\s*useBudgetStore\s*\}\s*from\s*'@\/stores\/budget\.ts'/);
        expect(source).toContain('budgetStore.expenseBudgets');
        expect(source).toContain('budgetStore.investmentBudgets');
        expect(source).toContain('budgetStore.loadAllBudgets');
        expect(source).toContain('budgetStore.loadBudgetExecution');
        // S4 must not introduce a new yuan/cents helper — uses existing
        // amountInYuan / spentAmountInYuan from the Budget model.
        expect(source).toContain('budget.amountInYuan');
        expect(source).toContain('budget.spentAmountInYuan');
    });

    test('renders f7-progressbar bound to the bounded ratio helper', () => {
        expect(source).toMatch(/<f7-progressbar[\s\S]*?:progress="getBudgetProgressPercent\(budget\)"/);
    });

    test('shows the empty state when filteredBudgets is empty (no crash)', () => {
        expect(source).toMatch(/filteredBudgets\.length === 0/);
        expect(source).toMatch(/tt\('No data'\)/);
    });

    test('tap handler is a deferred no-op for S4 (drilldown wired in S5)', () => {
        // Decision recorded: the desktop drilldown query depends on
        // buildBudgetDrilldownRouteQuery (categories + fiscal-year + filters);
        // mobile does not yet have this context, so S4 only logs the tap.
        expect(source).toContain('drilldown deferred to S5');
        expect(source).not.toMatch(/router\.push\(/);
    });
});
