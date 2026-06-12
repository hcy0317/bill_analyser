import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

const EDIT_SHEET_PATH = 'src/views/mobile/budgets/EditSheet.vue';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('mobile budgets EditSheet.vue source contract (S5)', () => {
    const source = readSource(EDIT_SHEET_PATH);

    describe('shell + structure', () => {
        test('renders inside a Framework7 sheet with swipe-to-close', () => {
            expect(source).toMatch(/<f7-sheet[\s\S]*?swipe-to-close/);
            expect(source).toContain('class="budget-edit-sheet"');
        });

        test('switches title between Add Budget and Edit Budget by isNew', () => {
            // Both branches must reach a translated title — required for the
            // create vs edit affordance the navbar + button surfaces.
            expect(source).toMatch(/isNew \? tt\('Add Budget'\) : tt\('Edit Budget'\)/);
        });

        test('emits update:show, save, and delete:request', () => {
            expect(source).toMatch(/\(e:\s*'update:show'/);
            expect(source).toMatch(/\(e:\s*'save'/);
            expect(source).toMatch(/\(e:\s*'delete:request'/);
        });
    });

    describe('form fields', () => {
        test('exposes a BudgetType segmented toggle (Expense / Investment)', () => {
            expect(source).toContain('<f7-segmented');
            expect(source).toMatch(/:active="form\.type === BudgetType\.Expense"/);
            expect(source).toMatch(/:active="form\.type === BudgetType\.Investment"/);
            expect(source).toMatch(/setType\(BudgetType\.Expense\)/);
            expect(source).toMatch(/setType\(BudgetType\.Investment\)/);
        });

        test('locks the type toggle once the budget has an id', () => {
            // After save the wire categoryId is bound to a specific
            // CategoryType — flipping the type would silently invalidate it.
            expect(source).toMatch(/:disabled="!isNew \|\| saving"/);
        });

        test('uses the existing tree-view-selection-sheet for category picking', () => {
            // S5 must NOT introduce a new category picker — it re-uses the
            // mobile tree-view sheet that S2 wired for transactions.
            expect(source).toContain('<tree-view-selection-sheet');
            expect(source).toMatch(/:items="availableCategories"/);
            expect(source).toMatch(/v-model="form\.categoryId"/);
        });

        test('binds available categories to CategoryType.Investment when type is Investment', () => {
            // Mirror the S2 helper choice: the tree picker switches between
            // expense and investment trees off CategoryType.
            expect(source).toContain('CategoryType.Investment');
            expect(source).toContain('CategoryType.Expense');
            expect(source).toMatch(/categoryStore\.allTransactionCategories\[catType\]/);
        });

        test('exposes period type options for Monthly / Quarterly / Yearly', () => {
            expect(source).toMatch(/BudgetPeriodType\.Monthly/);
            expect(source).toMatch(/BudgetPeriodType\.Quarterly/);
            expect(source).toMatch(/BudgetPeriodType\.Yearly/);
        });

        test('renders an amount input that round-trips via Budget.amountInYuan', () => {
            // Display path uses the existing Budget.amountInYuan getter — no
            // new yuan/cents helper is introduced.
            expect(source).toContain('amountInYuan');
            // Save path multiplies parsed yuan by 100, mirroring the inverse
            // of amountInYuan (= amountCents / 100).
            expect(source).toMatch(/Math\.round\(parsed \* 100\)/);
        });
    });

    describe('save / delete contract', () => {
        test('save button is gated by canSave (positive amount + chosen category)', () => {
            expect(source).toMatch(/form\.value\.amountCents <= 0/);
            expect(source).toMatch(/form\.value\.categoryId \|\| form\.value\.category/);
            expect(source).toMatch(/'disabled':\s*saving \|\| !canSave/);
        });

        test('save emits the (now category-resolved) Budget upward — no direct store call', () => {
            // Sheet stays presentation-only: the parent ListPage owns the
            // budgetStore.saveBudget invocation.
            expect(source).toMatch(/emit\('save',\s*form\.value\)/);
            expect(source).not.toMatch(/budgetStore\.saveBudget\(/);
            expect(source).not.toMatch(/budgetStore\.deleteBudget\(/);
        });

        test('delete is only reachable when editing an existing budget', () => {
            expect(source).toMatch(/v-if="!isNew"/);
            expect(source).toMatch(/emit\('delete:request',\s*form\.value\)/);
        });

        test('syncs primary/secondary category names from the chosen categoryId before emitting', () => {
            // The wire format requires `category` (and optionally `subCategory`)
            // — the picker only writes categoryId, so we must back-fill the
            // names before emitting save.
            expect(source).toMatch(/syncCategoryNamesFromId/);
            expect(source).toMatch(/form\.value\.category = primary\.name/);
        });
    });
});
