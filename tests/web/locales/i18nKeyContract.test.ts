import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

import {
    ALL_LANGUAGES,
    DEFAULT_LANGUAGE,
    getCompleteLanguageMessages
} from '@/locales/index.ts';
import { buildImportPreviewSignalViewModel } from '@/views/desktop/transactions/import/checkDataMatching.ts';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

function readJoinedSource(...relativePaths: string[]): string {
    return relativePaths.map(readSource).join('\n');
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

function listSourceFiles(relativeDirectory: string): string[] {
    const absoluteDirectory = path.resolve(process.cwd(), relativeDirectory);
    const sourceFiles: string[] = [];

    for (const entry of fs.readdirSync(absoluteDirectory, { withFileTypes: true })) {
        const relativePath = path.join(relativeDirectory, entry.name);

        if (entry.isDirectory()) {
            sourceFiles.push(...listSourceFiles(relativePath));
        } else if (/\.(vue|ts|tsx|js|jsx)$/.test(entry.name)) {
            sourceFiles.push(relativePath);
        }
    }

    return sourceFiles.sort();
}

function parseStringLiteral(rawLiteral: string): string | null {
    try {
        if (rawLiteral.startsWith('"')) {
            return JSON.parse(rawLiteral) as string;
        }

        const literalBody = rawLiteral.slice(1, -1);

        if (rawLiteral.startsWith('`') && literalBody.includes('${')) {
            return null;
        }

        const jsonStringBody = literalBody
            .replace(/"/g, '\\"')
            .replace(/\\'/g, "'")
            .replace(/\\`/g, '`');

        return JSON.parse(`"${jsonStringBody}"`) as string;
    } catch {
        return null;
    }
}

function extractStaticTranslationKeys(source: string): string[] {
    const keys = new Set<string>();
    const pattern = /(?:\btt|\bt|\$t)\(\s*((?:'[^'\\]*(?:\\.[^'\\]*)*')|(?:"[^"\\]*(?:\\.[^"\\]*)*")|(?:`[^`\\]*(?:\\.[^`\\]*)*`))/g;
    let match: RegExpExecArray | null;

    while ((match = pattern.exec(source)) !== null) {
        const nextToken = source.slice(pattern.lastIndex).match(/^\s*(.)/);

        if (nextToken?.[1] === '+') {
            continue;
        }

        const key = parseStringLiteral(match[1]!);

        if (key) {
            keys.add(key);
        }
    }

    return Array.from(keys).sort();
}

function extractStaticTranslationKeysFromSource(relativePath: string): string[] {
    return extractStaticTranslationKeys(readSource(relativePath));
}

function hasLocaleKey(messages: Record<string, unknown>, key: string): boolean {
    if (Object.prototype.hasOwnProperty.call(messages, key)) {
        return true;
    }

    let current: unknown = messages;

    for (const part of key.split('.')) {
        if (!current || typeof current !== 'object' || !Object.prototype.hasOwnProperty.call(current, part)) {
            return false;
        }

        current = (current as Record<string, unknown>)[part];
    }

    return true;
}

describe('i18n key contract for reported warning surfaces', () => {
    const activeLocales = ['en', 'zh_Hans', 'zh_Hant'];
    const localeFiles = [
        'de', 'en', 'es', 'fr', 'it', 'ja', 'ko', 'nl', 'pt_BR', 'ru', 'th', 'uk', 'vi', 'zh_Hans', 'zh_Hant'
    ];
    const staticTranslationKeys = Array.from(new Set(
        listSourceFiles('src').flatMap(file => extractStaticTranslationKeysFromSource(file))
    )).sort();

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

    test('investment amount validation is translated in every shipped locale', () => {
        for (const locale of localeFiles) {
            expect(readLocale(locale)).toHaveProperty('Investment amount cannot be blank');
        }

        expect(readLocale('zh_Hans')).toHaveProperty('Investment amount cannot be blank', '投资金额不能为空');
        expect(readLocale('zh_Hant')).toHaveProperty('Investment amount cannot be blank', '投資金額不能為空');
    });

    test('account matching rule builder title exists in active locales', () => {
        const accountEditDialog = readJoinedSource(
            'src/views/desktop/accounts/list/dialogs/EditDialog.vue',
            'src/views/desktop/accounts/list/dialogs/edit-dialog/EditDialog.template.html'
        );

        expect(accountEditDialog).toContain('title="Account Matching"');

        for (const locale of activeLocales) {
            const messages = readLocale(locale);

            expect(messages).toHaveProperty('Account Matching');
        }
    });

    test('default locale contains every statically referenced translation key', () => {
        const defaultMessages = readLocale(DEFAULT_LANGUAGE);
        const missingKeys = staticTranslationKeys.filter(key => !hasLocaleKey(defaultMessages, key));

        expect(missingKeys).toEqual([]);
    });

    test('runtime i18n messages complete supported locales from default locale', () => {
        const completedMessages = getCompleteLanguageMessages();

        expect(completedMessages['zh']).toBe(completedMessages['zh-Hans']);

        for (const languageKey of Object.keys(ALL_LANGUAGES)) {
            const messages = completedMessages[languageKey] as Record<string, unknown>;
            const missingKeys = staticTranslationKeys.filter(key => !hasLocaleKey(messages, key));

            expect(missingKeys).toEqual([]);
        }
    });

    test('import preview dynamic signal label keys exist in active locales', () => {
        const signalViews = [
            buildImportPreviewSignalViewModel({ transferStatus: 'pending' }),
            buildImportPreviewSignalViewModel({ transferStatus: 'accepted' }),
            buildImportPreviewSignalViewModel({ transferStatus: 'rejected' }),
            buildImportPreviewSignalViewModel({ learningStatus: 'pending' }),
            buildImportPreviewSignalViewModel({ learningStatus: 'pending', learningMode: 'blue' }),
            buildImportPreviewSignalViewModel({ learningStatus: 'accepted', learningAutoApplied: true }),
            buildImportPreviewSignalViewModel({ learningStatus: 'rejected' }),
            buildImportPreviewSignalViewModel({ llmStatus: 'pending' }),
            buildImportPreviewSignalViewModel({ llmStatus: 'accepted' }),
            buildImportPreviewSignalViewModel({ llmStatus: 'rejected' })
        ];
        const keys = Array.from(new Set(signalViews.flatMap(view => [
            view.transferSuggestion?.labelKey,
            ...(view.transferSuggestion?.actions.map(action => action.labelKey) ?? []),
            view.learning?.labelKey,
            ...(view.learning?.actions.map(action => action.labelKey) ?? []),
            view.llm?.labelKey,
            ...(view.llm?.actions.map(action => action.labelKey) ?? [])
        ].filter((key): key is string => !!key)))).sort();

        for (const locale of activeLocales) {
            const messages = readLocale(locale);
            const missingKeys = keys.filter(key => !hasLocaleKey(messages, key));

            expect(missingKeys).toEqual([]);
        }
    });

    test('static key scanner covers quoted literals without accepting dynamic keys', () => {
        const source = [
            "tt('Single Quoted Key')",
            't("Double Quoted Key")',
            '$t(`Backtick Quoted Key`)',
            'tt(`Dynamic ${key}`)',
            "tt('Dynamic Prefix' + suffix)"
        ].join('\n');

        expect(extractStaticTranslationKeys(source)).toEqual([
            'Backtick Quoted Key',
            'Double Quoted Key',
            'Single Quoted Key'
        ]);
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
