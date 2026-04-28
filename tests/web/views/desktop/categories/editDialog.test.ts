import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('transaction category edit dialog rule sync guards', () => {
    test('seeds the builder from legacy ruleExpression when canonical rows are missing', () => {
        const source = readSource('src/views/desktop/categories/list/dialogs/EditDialog.vue');

        expect(source).toContain("const legacyKeywords = (category.value.ruleExpression ?? '').trim();");
        expect(source).toContain('if (!primaryRule && legacyKeywords.length > 0)');
        expect(source).toContain('ruleExpression: legacyKeywords');
    });

    test('always syncs secondary-category builder edits back into category save requests', () => {
        const source = readSource('src/views/desktop/categories/list/dialogs/EditDialog.vue');

        expect(source).toContain('const canSyncSecondaryRule = isSecondaryCategory.value && !ruleLoadFailed.value;');
        expect(source).toContain('if (canSyncSecondaryRule) {');
        expect(source).toContain('category.value.ruleExpression = syncedRuleExpression;');
        expect(source).not.toContain('const shouldManageCanonicalRule =');
        expect(source).not.toContain('if (shouldManageCanonicalRule)');
    });

    test('skips rule sync when canonical-rule loading failed, so saves do not wipe the legacy mirror', () => {
        const source = readSource('src/views/desktop/categories/list/dialogs/EditDialog.vue');

        expect(source).toContain('const ruleLoadFailed = ref<boolean>(false);');
        expect(source).toContain('ruleLoadFailed.value = true;');
        expect(source).toContain('if (canSyncSecondaryRule && savedCategory.id) {');
    });
});
