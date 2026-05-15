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

    test('snackbar only translates known keys and leaves raw runtime messages alone', () => {
        const snackbarSource = readSource('src/components/desktop/SnackBar.vue');
        const helperSource = readSource('src/locales/helpers.ts');
        const budgetSource = readSource('src/views/desktop/budgets/ListPage.vue');

        expect(snackbarSource).toContain('const { tt, tm, te } = useI18n();');
        expect(snackbarSource).toContain('messageContent.value = tm(message, options);');
        expect(snackbarSource).not.toContain('messageContent.value = tt(message)');
        expect(helperSource).toContain('function translateMessage');
        expect(helperSource).toContain('return hasLocaleMessage(key) || hasLocaleMessage(key, DEFAULT_LANGUAGE);');
        expect(helperSource).not.toContain('return t(finalMessage, parameters);');
        expect(budgetSource).toContain("showMessage('Budgets exported successfully')");
        expect(budgetSource).not.toContain("showMessage(tt('Budgets exported successfully'))");

        for (const locale of activeLocales) {
            const messages = readLocale(locale);
            expect(messages).toHaveProperty('Budgets exported successfully');
        }
    });
});
