import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

function readLocale(locale: string): Record<string, unknown> {
    return JSON.parse(readSource(`src/locales/${locale}.json`));
}

function extractSimpleTranslationKeys(source: string): string[] {
    const keys = new Set<string>();
    const pattern = /\btt\(\s*['"]([^'"]+)['"]\s*[),]/g;
    let match: RegExpExecArray | null;

    while ((match = pattern.exec(source)) !== null) {
        keys.add(match[1]!);
    }

    return Array.from(keys).sort();
}

describe('i18n key contract for reported warning surfaces', () => {
    const activeLocales = ['en', 'zh_Hans', 'zh_Hant'];

    test('reported budget history and rule builder keys exist in active locales', () => {
        const sourceFiles = [
            'src/components/common/CategoryRuleBuilderFields.vue',
            'src/views/desktop/budgets/components/BudgetHistoryPanel.vue'
        ];
        const keys = sourceFiles.flatMap(file => extractSimpleTranslationKeys(readSource(file)));

        for (const locale of activeLocales) {
            const messages = readLocale(locale);

            for (const key of keys) {
                expect(messages).toHaveProperty(key);
            }
        }
    });

    test('category rule builder callers pass translation keys, not translated labels', () => {
        const callerFiles = [
            'src/views/desktop/categories/list/dialogs/EditDialog.vue',
            'src/views/desktop/pairingcenter/components/RuleCenterPanel.vue'
        ];

        for (const file of callerFiles) {
            const source = readSource(file);

            expect(source).toContain('<category-rule-builder-fields');
            expect(source).not.toMatch(/<category-rule-builder-fields[\s\S]*?:title="tt\(/);
        }
    });
});
