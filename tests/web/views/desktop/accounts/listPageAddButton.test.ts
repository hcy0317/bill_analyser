import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('desktop account list add button contract', () => {
    test('header add action dispatches by current account category without menu items or icon', () => {
        const facade = readSource('src/views/desktop/accounts/ListPage.vue');
        const template = readSource('src/views/desktop/accounts/list/ListPage.template.html');
        const source = `${facade}\n${template}`;
        const toolbarAddButton = template.match(/<v-btn class="ms-3"[\s\S]*?\{\{ tt\('Add'\) \}\}[\s\S]*?<\/v-btn>/)?.[0] ?? '';

        expect(toolbarAddButton).toContain('@click="addAccountForCurrentCategory"');
        expect(toolbarAddButton).not.toContain(':prepend-icon');
        expect(toolbarAddButton).not.toContain('<v-menu');
        expect(source).toContain('function addAccountForCurrentCategory(): void');
        expect(source).toContain('addAccountForCategory(activeAccountCategoryType.value);');
    });
});
