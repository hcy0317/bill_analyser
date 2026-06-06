import fs from 'node:fs';
import path from 'node:path';

import {
    SETTINGS_BUNDLE_DATA_MANAGEMENT_ENTRIES,
    SETTINGS_BUNDLE_SECTION_LABEL_KEYS,
    buildSettingsBundleSectionFileName,
    sanitizeWindowsFileNameSegment,
    type SettingsBundleSectionKey,
} from '@/models/data_management.ts';

describe('settings JSON per-page import/export controls', () => {
    const readSource = (relativePath: string) => fs.readFileSync(
        path.resolve(process.cwd(), relativePath),
        'utf-8'
    );

    const expectedSettingsSections: SettingsBundleSectionKey[] = [
        'accounts',
        'transactionCategories',
        'transactionTags',
        'transactionTemplates',
        'scheduledTransactions',
        'categoryRecognitionRules',
        'accountRecognitionRules',
        'llmConfigs',
        'ocrConfig',
    ];

    test('keeps export inside the shared import button hover card', () => {
        const source = readSource('src/components/desktop/SettingsJsonImportExportButton.vue');

        expect(source).toContain('<v-btn');
        expect(source).toContain("{{ tt('Import') }}");
        expect(source).toContain('activator="parent"');
        expect(source).toContain(':open-on-hover="true"');
        expect(source).toContain(':open-delay="1500"');
        expect(source).toContain("tt('Export Settings JSON')");
        expect(source).toContain("openTextFileContent({ allowedExtensions: '.json,application/json' })");
        expect(source).toContain('previewImportSettingsBundleSection');
        expect(source).toContain('importSettingsBundleSection');
    });

    test('wires every requested desktop page to its own settings section', () => {
        const accounts = readSource('src/views/desktop/accounts/ListPage.vue');
        const categories = readSource('src/views/desktop/categories/ListPage.vue');
        const tags = readSource('src/views/desktop/tags/ListPage.vue');
        const templates = readSource('src/views/desktop/templates/ListPage.vue');
        const rules = readSource('src/views/desktop/pairingcenter/components/RuleCenterPanel.vue');
        const accountRules = readSource('src/views/desktop/pairingcenter/components/AccountRulePanel.vue');
        const learning = readSource('src/views/desktop/pairingcenter/components/LearningCenterPanel.vue');

        expect(accounts).toContain('section-key="accounts"');
        expect(categories).toContain('section-key="transactionCategories"');
        expect(tags).toContain('section-key="transactionTags"');
        expect(templates).toContain("'transactionTemplates'");
        expect(templates).toContain("'scheduledTransactions'");
        expect(rules).toContain('section-key="categoryRecognitionRules"');
        expect(accountRules).toContain('section-key="accountRecognitionRules"');
        expect(learning).toContain('section-key="llmConfigs"');
    });

    test('removes incorrect standalone settings-page entry and category dual buttons', () => {
        const dataManagement = readSource('src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue');
        const categories = readSource('src/views/desktop/categories/ListPage.vue');

        expect(dataManagement).not.toContain("tt('Settings JSON')");
        expect(dataManagement).not.toContain('importSettingsBundle');
        expect(dataManagement).not.toContain('exportSettingsBundle');
        expect(categories).not.toContain('@click="exportCategories"');
        expect(categories).not.toContain('@click="importCategories"');
        expect(categories).not.toContain('@change="onFileSelected"');
        expect(categories).not.toContain('importTransactionCategories');
        expect(categories).not.toContain('exportTransactionCategories');
    });

    test('uses section-scoped service endpoints', () => {
        const services = readSource('src/lib/services.ts');

        expect(services).toContain('settings/bundle/sections/${sectionKey}/export');
        expect(services).toContain('settings/bundle/sections/${sectionKey}/import/preview');
        expect(services).toContain('settings/bundle/sections/${sectionKey}/import');
    });

    test('keeps import preview details literal and uses svg-path icons', () => {
        const confirmDialog = readSource('src/components/desktop/ConfirmDialog.vue');
        const importButton = readSource('src/components/desktop/SettingsJsonImportExportButton.vue');

        expect(confirmDialog).toContain("import { mdiCircleSmall } from '@mdi/js';");
        expect(confirmDialog).toContain(':icon="mdiCircleSmall"');
        expect(confirmDialog).toContain('map(d => tm(d, actualOptions))');
        expect(confirmDialog).toContain('map(d => tm(d, options))');
        expect(confirmDialog).not.toContain('map(d => tt(d, actualOptions))');
        expect(confirmDialog).not.toContain('map(d => tt(d, options))');
        expect(importButton).toContain('getApiErrorMessageOrDefault(error,');
    });

    test('builds localized Windows-safe per-section export filenames', () => {
        const timestamp = new Date('2026-06-06T12:34:56.000Z');

        expect(buildSettingsBundleSectionFileName('分类识别规则', timestamp))
            .toBe('分类识别规则_20260606123456.json');
        expect(buildSettingsBundleSectionFileName('账户:识别/规则*', timestamp))
            .toBe('账户_识别_规则_20260606123456.json');
        expect(buildSettingsBundleSectionFileName('CON', timestamp))
            .toBe('_CON_20260606123456.json');
        expect(sanitizeWindowsFileNameSegment('///', 'settings'))
            .toBe('settings');
        expect(buildSettingsBundleSectionFileName('OCR 配置', timestamp))
            .not.toMatch(/[<>:"/\\|?*\u0000-\u001f]/);
    });

    test('uses localized labels for every settings bundle data-management entry', () => {
        expect(Object.keys(SETTINGS_BUNDLE_SECTION_LABEL_KEYS)).toEqual(expectedSettingsSections);
        expect(SETTINGS_BUNDLE_DATA_MANAGEMENT_ENTRIES.map(entry => entry.sectionKey))
            .toEqual(expectedSettingsSections);
        expect(new Set(SETTINGS_BUNDLE_DATA_MANAGEMENT_ENTRIES.map(entry => entry.sectionKey)).size)
            .toBe(expectedSettingsSections.length);

        for (const entry of SETTINGS_BUNDLE_DATA_MANAGEMENT_ENTRIES) {
            expect(entry.titleKey).toBe(SETTINGS_BUNDLE_SECTION_LABEL_KEYS[entry.sectionKey]);
            expect(entry.desktopRoute).toMatch(/^\//);
        }
    });

    test('surfaces integrated settings JSON entries from desktop and mobile Data Management pages', () => {
        const desktopDataManagement = readSource('src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue');
        const mobileDataManagement = readSource('src/views/mobile/users/DataManagementPage.vue');

        expect(desktopDataManagement).toContain("tt('Settings JSON Import/Export')");
        expect(desktopDataManagement).toContain('settingsBundleDataManagementEntries');
        expect(desktopDataManagement).toContain('<settings-json-import-export-button');
        expect(desktopDataManagement).toContain(':section-key="entry.sectionKey"');
        expect(desktopDataManagement).toContain(':to="entry.desktopRoute"');
        expect(desktopDataManagement).toContain("tt('Export Data')");

        expect(mobileDataManagement).toContain("tt('Settings JSON Import/Export')");
        expect(mobileDataManagement).toContain('settingsBundleDataManagementEntries');
        expect(mobileDataManagement).toContain(':link="entry.mobileRoute || null"');
        expect(mobileDataManagement).toContain("tt('Export Data')");
    });

    test('keeps localized settings JSON labels available for Chinese filenames and entries', () => {
        const en = JSON.parse(readSource('src/locales/en.json')) as Record<string, string>;
        const zhHans = JSON.parse(readSource('src/locales/zh_Hans.json')) as Record<string, string>;
        const zhHant = JSON.parse(readSource('src/locales/zh_Hant.json')) as Record<string, string>;

        expect(en['Settings JSON Import/Export']).toBe('Settings JSON Import/Export');
        expect(zhHans['Category Recognition Rules']).toBe('分类识别规则');
        expect(zhHans['Account Recognition Rules']).toBe('账户识别规则');
        expect(zhHans['Settings JSON Import/Export']).toBe('设置 JSON 导入/导出');
        expect(zhHant['Account Recognition Rules']).toBe('帳戶識別規則');
        expect(zhHant['OCR Config']).toBe('OCR 設定');
    });
});
