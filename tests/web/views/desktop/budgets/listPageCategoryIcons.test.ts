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

describe('desktop budget list page source contract', () => {
    const readListPageSource = () => fs.readFileSync(
        path.resolve(process.cwd(), 'src/views/desktop/budgets/ListPage.vue'),
        'utf-8'
    );

    const readBudgetPageDomainSource = () => [
        'src/views/desktop/budgets/ListPage.vue',
        'src/views/desktop/budgets/list/ListPage.template.html',
        'src/views/desktop/budgets/list/ListPage.types.ts',
        'src/views/desktop/budgets/list/budgetAmountFilters.ts',
        'src/views/desktop/budgets/list/budgetHistoryPeriods.ts',
        'src/views/desktop/budgets/list/budgetPresentation.ts'
    ].map(sourcePath => fs.readFileSync(path.resolve(process.cwd(), sourcePath), 'utf-8')).join('\n');

    test('keeps the budget page facade wired to stores, dialogs, and helper modules', () => {
        const source = readListPageSource();

        [
            "import { useBudgetStore } from '@/stores/budget.ts'",
            "import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts'",
            "import BudgetForecastPanel from './components/BudgetForecastPanel.vue'",
            "import BudgetForecastSettingsDialog from './components/BudgetForecastSettingsDialog.vue'",
            "import BudgetHistoryPanel from './components/BudgetHistoryPanel.vue'",
            "import BudgetListTable from './components/BudgetListTable.vue'",
            "import EditDialog from './list/dialogs/EditDialog.vue'",
            "import { buildBudgetDrilldownRouteQuery } from './categorySelection.ts'",
            "import { buildBudgetForecastLoadRequest } from './forecastRequest.ts'",
            'const budgetStore = useBudgetStore()',
            'const transactionCategoriesStore = useTransactionCategoriesStore()'
        ].forEach(requiredSource => {
            expect(source).toContain(requiredSource);
        });
    });

    test('keeps reload, forecast, import/export, saved callback, delete, and drilldown actions attached', () => {
        const source = readBudgetPageDomainSource();

        [
            'async function reload(force: boolean): Promise<void>',
            'async function loadForecast(): Promise<void>',
            'function navigateToTransactions(category: string, budget: Budget | null): void',
            'function getCurrentPeriodRequest(): BudgetHistoryRequest',
            'async function exportBudgets(): Promise<void>',
            'function importBudgets(): void',
            'async function onFileSelected(event: Event): Promise<void>',
            'function onBudgetSaved(): void',
            '@budget:saved="onBudgetSaved"',
            'budgetStore.loadAllBudgets',
            'budgetStore.loadBudgetExecution',
            'budgetStore.loadBudgetHistory',
            'budgetStore.loadBudgetForecast',
            'budgetStore.deleteBudget',
            'budgetStore.exportBudgets',
            'budgetStore.importBudgets',
            'buildBudgetDrilldownRouteQuery({',
            'buildBudgetForecastLoadRequest'
        ].forEach(requiredSource => {
            expect(source).toContain(requiredSource);
        });
    });
});
