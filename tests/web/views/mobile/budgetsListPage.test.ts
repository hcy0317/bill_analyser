import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

import { Budget, BudgetType } from '@/models/budget.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import {
    buildMobileBudgetGroups,
    formatBudgetAmount,
    getBudgetGroupExecutionRate,
    getBudgetGroupProgressPercent,
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
    b.amountCents = overrides.amountCents ?? 0;
    b.spentAmountCents = overrides.spentAmountCents ?? 0;
    if (overrides.executionRate !== undefined) {
        b.executionRate = overrides.executionRate;
    }
    if (overrides.category !== undefined) {
        b.category = overrides.category;
    }
    if (overrides.subCategory !== undefined) {
        b.subCategory = overrides.subCategory;
    }
    if (overrides.categoryIcon !== undefined) {
        b.categoryIcon = overrides.categoryIcon;
    }
    if (overrides.categoryColor !== undefined) {
        b.categoryColor = overrides.categoryColor;
    }
    return b;
}

function makePrimaryCategory(overrides: Partial<TransactionCategory>): TransactionCategory {
    return {
        id: overrides.id ?? 'cat-food',
        name: overrides.name ?? 'Food',
        icon: overrides.icon ?? 'food',
        color: overrides.color ?? 'ff9900',
        subCategories: overrides.subCategories ?? []
    } as TransactionCategory;
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
            const budget = makeBudget({ amountCents: 200000, spentAmountCents: 50000 });
            expect(getBudgetProgressPercent(budget)).toBe(25);
        });

        test('returns 0 when budget amount is zero or negative', () => {
            expect(getBudgetProgressPercent(makeBudget({ amountCents: 0, spentAmountCents: 100 }))).toBe(0);
            expect(getBudgetProgressPercent(makeBudget({ amountCents: -1, spentAmountCents: 100 }))).toBe(0);
        });

        test('caps over-budget progress at 100 (text rate still shows real %)', () => {
            const budget = makeBudget({ amountCents: 100, spentAmountCents: 250 });
            expect(getBudgetProgressPercent(budget)).toBe(100);
        });

        test('returns 0 for null/undefined safely', () => {
            // Defensive: empty list should not crash if a malformed budget appears.
            expect(getBudgetProgressPercent(null as unknown as Budget)).toBe(0);
        });

        test('handles non-finite ratios gracefully', () => {
            const budget = makeBudget({ amountCents: 100, spentAmountCents: Number.NaN });
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

    describe('buildMobileBudgetGroups', () => {
        test('groups secondary budgets below the primary category and uses category icon/color', () => {
            const primary = makeBudget({
                id: 'p-food',
                category: 'Food',
                subCategory: '',
                amountCents: 10000,
                spentAmountCents: 3000
            });
            const breakfast = makeBudget({
                id: 's-breakfast',
                category: 'Food',
                subCategory: 'Breakfast',
                amountCents: 2000,
                spentAmountCents: 500,
                categoryIcon: 'breakfast',
                categoryColor: '00aa00'
            });

            const groups = buildMobileBudgetGroups({
                budgets: [breakfast, primary],
                primaryCategories: [makePrimaryCategory({ name: 'Food', icon: 'restaurant', color: 'ff8800' })],
                collapsedCategories: new Set()
            });

            expect(groups).toHaveLength(1);
            expect(groups[0]!.category).toBe('Food');
            expect(groups[0]!.categoryIcon).toBe('restaurant');
            expect(groups[0]!.categoryColor).toBe('ff8800');
            expect(groups[0]!.primaryBudgets.map(b => b.id)).toEqual(['p-food']);
            expect(groups[0]!.subBudgets.map(b => b.subCategory)).toEqual(['Breakfast']);
            expect(groups[0]!.totalAmountCents).toBe(10000);
            expect(groups[0]!.totalSpentCents).toBe(3000);
        });

        test('falls back to secondary totals when no primary budget exists', () => {
            const coffee = makeBudget({
                id: 's-coffee',
                category: 'Food',
                subCategory: 'Coffee',
                amountCents: 2000,
                spentAmountCents: 2200
            });
            const lunch = makeBudget({
                id: 's-lunch',
                category: 'Food',
                subCategory: 'Lunch',
                amountCents: 3000,
                spentAmountCents: 900
            });

            const groups = buildMobileBudgetGroups({
                budgets: [lunch, coffee],
                primaryCategories: [],
                collapsedCategories: new Set(['Food'])
            });

            expect(groups[0]!.totalAmountCents).toBe(5000);
            expect(groups[0]!.totalSpentCents).toBe(3100);
            expect(groups[0]!.isCollapsed).toBe(true);
            expect(groups[0]!.subBudgets.map(b => b.subCategory)).toEqual(['Coffee', 'Lunch']);
            expect(getBudgetGroupExecutionRate(groups[0]!)).toBe(62);
            expect(getBudgetGroupProgressPercent(groups[0]!)).toBe(62);
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
        expect(source).toMatch(/<f7-progressbar[\s\S]*?:progress="getBudgetGroupProgressPercent\(group\)"/);
    });

    test('renders primary category groups with category icons and collapse state', () => {
        expect(source).toContain('groupedBudgets');
        expect(source).toContain('toggleBudgetGroup');
        expect(source).toContain('group.isCollapsed');
        expect(source).toMatch(/icon-type="category"[\s\S]*?:icon-id="group\.categoryIcon"/);
    });

    test('shows the empty state when filteredBudgets is empty (no crash)', () => {
        expect(source).toMatch(/filteredBudgets\.length === 0/);
        expect(source).toMatch(/tt\('No data'\)/);
    });

    test('tap handler is still a deferred no-op (drilldown owned by S6/S9)', () => {
        // Decision recorded again in S5: the desktop drilldown query depends
        // on buildBudgetDrilldownRouteQuery, which lives in
        // views/desktop/budgets/categorySelection.ts — outside the mobile
        // budgets/ owned_paths. The tap stays a logging no-op until a later
        // slice promotes the helper into a shared module.
        expect(source).toMatch(/drilldown still deferred/);
        expect(source).not.toMatch(/router\.push\(/);
        expect(source).not.toMatch(/f7router\.navigate\(/);
    });
});

describe('mobile budgets ListPage.vue write-parity affordances (S5)', () => {
    const source = readSource(LIST_PAGE_PATH);

    test('navbar exposes a + button that opens the create sheet', () => {
        expect(source).toMatch(/<f7-nav-right[\s\S]*?icon-f7="plus"[\s\S]*?@click="openCreateSheet"/);
    });

    test('mounts BudgetEditSheet bound to showEditSheet + editingBudget + activeBudgetType', () => {
        expect(source).toContain('<budget-edit-sheet');
        expect(source).toMatch(/v-model:show="showEditSheet"/);
        expect(source).toMatch(/:budget="editingBudget"/);
        expect(source).toMatch(/:default-type="activeBudgetType"/);
        expect(source).toMatch(/@save="onSheetSave"/);
        expect(source).toMatch(/@delete:request="requestDelete"/);
    });

    test('row swipeout exposes Edit + Delete actions', () => {
        expect(source).toContain('<f7-swipeout-actions');
        expect(source).toMatch(/openEditSheet\(budget\)/);
        expect(source).toMatch(/requestDelete\(budget\)/);
    });

    test('save handler delegates to budgetStore.saveBudget (no new store actions)', () => {
        expect(source).toMatch(/budgetStore\.saveBudget\(\{\s*budget:\s*updated\s*\}\)/);
    });

    test('confirmDelete handler delegates to budgetStore.deleteBudget (no new store actions)', () => {
        // Note: the in-flight task brief calls this "removeBudget" but the
        // real store action is named deleteBudget — we use the real one.
        expect(source).toMatch(/budgetStore\.deleteBudget\(\{\s*budgetId:\s*target\.id\s*\}\)/);
    });

    test('delete confirmation uses an action sheet, not a destructive auto-delete', () => {
        expect(source).toContain('showDeleteActionSheet');
        expect(source).toMatch(/<f7-actions[\s\S]*?:opened="showDeleteActionSheet"/);
    });
});
