import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('mobile account recognition rule parity', () => {
    test('mobile router and settings expose account-rule management', () => {
        const router = readSource('src/router/mobile.ts');
        const settings = readSource('src/views/mobile/SettingsPage.vue');

        expect(router).toContain("import AccountRuleListPage from '@/views/mobile/accounts/RuleListPage.vue';");
        expect(router).toContain("path: '/account/rules'");
        expect(settings).toContain("link=\"/account/rules\"");
        expect(settings).toContain("tt('Account Recognition Rules')");
    });

    test('mobile account edit quick-adds account rules without aliases', () => {
        const source = readSource('src/views/mobile/accounts/EditPage.vue');

        expect(source).not.toContain("tt('Account Aliases')");
        expect(source).not.toContain('formatAliasText');
        expect(source).not.toContain('updateAccountAliases');
        expect(source).not.toContain('parseAliasText');
        expect(source).toContain('services.createAccountRule');
        expect(source).toContain("tt('Rule Matching Expression')");
        expect(source).toContain('openAccountRules(account)');
        expect(source).toContain('openAccountRules(subAccount)');
    });

    test('mobile rule page wires CRUD, test, and scope controls', () => {
        const source = readSource('src/views/mobile/accounts/RuleListPage.vue');

        expect(source).toContain('services.getAccountRules');
        expect(source).toContain('services.createAccountRule');
        expect(source).toContain('services.updateAccountRule');
        expect(source).toContain('services.deleteAccountRule');
        expect(source).toContain('services.testAccountRule');
        expect(source).toContain('ACCOUNT_RULE_ROLE_SCOPE_OPTIONS');
        expect(source).toContain('ACCOUNT_RULE_TRANSACTION_SCOPE_OPTIONS');
        expect(source).toContain('ACCOUNT_RULE_FIELD_SCOPE_OPTIONS');
        expect(source).toContain('toggleFieldScope');
    });
});
