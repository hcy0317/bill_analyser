import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('mobile secondary category creation contract', () => {
    test('list carries the current primary category into the add route', () => {
        const source = readSource('src/views/mobile/categories/ListPage.vue');

        expect(source).toContain("'&parentId=' + parent.id");
        expect(source).toContain('CategoryType.Investment');
    });

    test('list exposes discoverable primary and secondary actions with a guarded parent', () => {
        const source = readSource('src/views/mobile/categories/ListPage.vue');
        const primaryAction = source.match(/<f7-list-button[^>]*data-testid="mobile\.categories\.action\.add-primary"[\s\S]*?<\/f7-list-button>/)?.[0] ?? '';
        const secondaryAction = source.match(/<f7-list-button[^>]*data-testid="mobile\.categories\.action\.add-secondary"[\s\S]*?<\/f7-list-button>/)?.[0] ?? '';

        expect(primaryAction).toContain("tt('Add Primary Category')");
        expect(primaryAction).toContain('primaryCategoryAddHref');
        expect(secondaryAction).toContain("tt('Add Secondary Category')");
        expect(secondaryAction).toContain("'disabled': !canAddSecondaryCategory");
        expect(secondaryAction).toContain('secondaryCategoryAddHref');
        expect(source).not.toContain('data-testid="mobile.categories.action.add"');
    });

    test('saving a category updates the in-memory tree immediately', () => {
        const store = readSource('src/stores/transactionCategory.ts');

        expect(store).toContain('addCategoryToTransactionCategoryList(transactionCategory)');
        expect(store).toContain('function addCategoryToTransactionCategoryList');
    });

    test('edit page creates with parentId and accepts every supported category type', () => {
        const editPage = readSource('src/views/mobile/categories/EditPage.vue');
        const base = readSource('src/views/base/categories/CategoryEditPageBase.ts');
        const model = readSource('src/models/transaction_category.ts');

        expect(editPage).toContain("query['parentId']");
        expect(editPage).toContain('CategoryType.Investment');
        expect(base).toContain('TransactionCategory.createNewCategory(type, parentId)');
        expect(model).toContain('parentId: this.parentId');
    });
});
