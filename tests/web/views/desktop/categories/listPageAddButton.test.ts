import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('desktop transaction category add button contract', () => {
    test('header add action dispatches by current category level without menu items or icon', () => {
        const source = readSource('src/views/desktop/categories/ListPage.vue');
        const toolbarAddButton = source.match(/<v-btn class="ms-3"[\s\S]*?\{\{ tt\('Add'\) \}\}[\s\S]*?<\/v-btn>/)?.[0] ?? '';

        expect(toolbarAddButton).toContain('@click="addCategoryByCurrentSelection"');
        expect(toolbarAddButton).not.toContain(':prepend-icon');
        expect(toolbarAddButton).not.toContain('<v-menu');
        expect(source).not.toContain("tt('Add Primary Category')");
        expect(source).not.toContain("tt('Add Secondary Category')");
        expect(source).toContain('function addCategoryByCurrentSelection(): void');
        expect(source).toContain('if (canAddSecondaryCategory.value)');
        expect(source).toContain('addSecondaryCategory();');
        expect(source).toContain('addPrimaryCategory();');
    });
});
