import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('desktop transaction category add button contract', () => {
    test('header exposes separate primary and secondary category actions', () => {
        const facade = readSource('src/views/desktop/categories/ListPage.vue');
        const template = readSource('src/views/desktop/categories/list/ListPage.template.html');
        const renderedSurface = `${facade}\n${template}`;
        const primaryAction = template.match(/<v-btn[^>]*data-testid="desktop\.categories\.action\.add-primary"[\s\S]*?<\/v-btn>/)?.[0] ?? '';
        const secondaryAction = template.match(/<v-btn[^>]*data-testid="desktop\.categories\.action\.add-secondary"[\s\S]*?<\/v-btn>/)?.[0] ?? '';

        expect(primaryAction).toContain('@click="addPrimaryCategory"');
        expect(primaryAction).toContain("tt('Add Primary Category')");
        expect(secondaryAction).toContain('@click="addSecondaryCategory"');
        expect(secondaryAction).toContain("tt('Add Secondary Category')");
        expect(secondaryAction).toContain('!canAddSecondaryCategory');
        expect(renderedSurface).not.toContain('addCategoryByCurrentSelection');
    });
});
