import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('mobile secondary category creation contract', () => {
    test('list carries the current primary category into the add route', () => {
        const source = readSource('src/views/mobile/categories/ListPage.vue');

        expect(source).toContain("'&parentId=' + primaryCategoryId");
        expect(source).toContain('CategoryType.Investment');
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
