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

    test('mobile account edit links each persisted account to rules without alias fields', () => {
        const source = readSource('src/views/mobile/accounts/EditPage.vue');

        expect(source).toContain('`/account/rules?accountId=${account.id}`');
        expect(source).toContain('`/account/rules?accountId=${subAccount.id}`');
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
