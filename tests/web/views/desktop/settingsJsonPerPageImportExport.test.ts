import fs from 'node:fs';
import path from 'node:path';

describe('settings JSON per-page import/export controls', () => {
    const readSource = (relativePath: string) => fs.readFileSync(
        path.resolve(process.cwd(), relativePath),
        'utf-8'
    );

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
});
