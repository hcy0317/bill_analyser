import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('transaction category edit dialog rule sync guards', () => {
    test('seeds the builder only from loaded category rule rows', () => {
        const source = readSource('src/views/desktop/categories/list/dialogs/EditDialog.vue');

        expect(source).toContain('const primaryRule = rules[0] ?? null;');
        expect(source).toContain('categoryRuleDraft.value = primaryRule ? normalizeCategoryRuleDraft({');
        expect(source).not.toContain('keywords:');
    });

    test('always syncs secondary-category builder edits back into category save requests', () => {
        const source = readSource('src/views/desktop/categories/list/dialogs/EditDialog.vue');

        expect(source).toContain('const canSyncSecondaryRule = isSecondaryCategory.value && !ruleLoadFailed.value;');
        expect(source).toContain('if (canSyncSecondaryRule && savedCategory.id) {');
        expect(source).toContain('category.value.ruleExpression = currentPrimaryRuleExpression;');
        expect(source).not.toContain('const shouldManageCanonicalRule =');
        expect(source).not.toContain('if (shouldManageCanonicalRule)');
    });

    test('skips rule sync when category-rule loading failed', () => {
        const source = readSource('src/views/desktop/categories/list/dialogs/EditDialog.vue');

        expect(source).toContain('const ruleLoadFailed = ref<boolean>(false);');
        expect(source).toContain('ruleLoadFailed.value = true;');
        expect(source).toContain('if (canSyncSecondaryRule && savedCategory.id) {');
    });
});
