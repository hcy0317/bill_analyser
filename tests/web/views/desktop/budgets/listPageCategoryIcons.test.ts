import { describe, expect, test } from '@jest/globals';
import fs from 'node:fs';
import path from 'node:path';

describe('desktop budget category icon loading', () => {
    const readListPageSource = () => fs.readFileSync(
        path.resolve(process.cwd(), 'src/views/desktop/budgets/ListPage.vue'),
        'utf-8'
    );

    const readBudgetListTableSource = () => fs.readFileSync(
        path.resolve(process.cwd(), 'src/views/desktop/budgets/components/BudgetListTable.vue'),
        'utf-8'
    );

    function getReloadSource(source: string): string {
        const startIndex = source.indexOf('async function reload(force: boolean): Promise<void>');
        const endIndex = source.indexOf('async function loadForecast(): Promise<void>', startIndex);

        expect(startIndex).toBeGreaterThanOrEqual(0);
        expect(endIndex).toBeGreaterThan(startIndex);

        return source.slice(startIndex, endIndex);
    }

    test('reload warms transaction category metadata with the budget list request', () => {
        const reloadSource = getReloadSource(readListPageSource());

        expect(reloadSource).toContain('transactionCategoriesStore.loadAllCategories({ force: false })');
        expect(reloadSource.indexOf('transactionCategoriesStore.loadAllCategories')).toBeLessThan(
            reloadSource.indexOf('budgetStore.loadAllBudgets')
        );
        expect(reloadSource).toContain('Promise.allSettled');
    });

    test('budget grouping falls back to icon and color metadata from the budget response', () => {
        const source = readListPageSource();

        expect(source).toContain('const categoryIcon = primaryCategory?.icon || budget.categoryIcon ||');
        expect(source).toContain('normalizeCategoryColor(primaryCategory?.color || budget.categoryColor)');
        expect(source).toContain('function getBudgetCategoryIcon(budget: Budget, group: BudgetGroup): string');
        expect(source).toContain('return budget.categoryIcon || group.categoryIcon;');
        expect(source).toContain('function getBudgetCategoryColor(budget: Budget, group: BudgetGroup): string');
        expect(source).toContain('return normalizeCategoryColor(budget.categoryColor) || group.categoryColor;');
    });

    test('budget list rows always render ItemIcon through the shared category resolver', () => {
        const budgetTableSource = readBudgetListTableSource();

        expect(budgetTableSource).toContain(':icon-id="group.categoryIcon"');
        expect(budgetTableSource).toContain(':icon-id="getBudgetCategoryIcon(budget, group)"');
        expect(budgetTableSource).not.toContain('v-if="group.categoryIcon"');
        expect(budgetTableSource).not.toContain('v-if="budget.categoryIcon"');
    });
});
