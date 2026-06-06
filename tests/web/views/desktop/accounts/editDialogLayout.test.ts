import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('desktop account edit dialog layout contract', () => {
    test('dialog width is constrained by viewport and account matching editor stays compact', () => {
        const editDialog = readSource('src/views/desktop/accounts/list/dialogs/EditDialog.vue');

        expect(editDialog).toContain('width="calc(100vw - 32px)"');
        expect(editDialog).toContain(':max-width="account.type === AccountType.MultiSubAccounts.type ? 1000 : 800"');
        expect(editDialog).toContain('class="account-edit-card');
        expect(editDialog).toContain('class="account-edit-content');
        expect(editDialog).toContain('class="account-edit-window');

        expect(editDialog).toContain('CategoryRuleBuilderFields');
        expect(editDialog).toContain('<category-rule-builder-fields');
        expect(editDialog).toContain('v-model="selectedAccountRuleBuilderModel"');
        expect(editDialog).toContain('title="Account Matching"');
        expect(editDialog).not.toContain('title="Account Recognition Rules"');
        expect(editDialog).toContain("`${accountName} - ${tt('Account Rule')}`");
        expect(editDialog.indexOf('title="Account Matching"')).toBeLessThan(
            editDialog.indexOf(':label="tt(\'Description\')"')
        );
        expect(editDialog).not.toContain('бд');
        expect(editDialog).toContain('accountRuleLoading');
        expect(editDialog).toContain('accountRuleLoadFailed');
        expect(editDialog).toContain('services.getAccountRules(accountId)');
        expect(editDialog).toContain('services.createAccountRule(payload)');
        expect(editDialog).toContain('services.updateAccountRule(primaryAccountRuleId.value, payload)');
        expect(editDialog).toContain('services.deleteAccountRule(primaryAccountRuleId.value)');
        expect(editDialog).toContain('.account-rule-section :deep(.category-rule-builder)');
        expect(editDialog).toContain('overflow-x: hidden;');

        expect(editDialog).not.toContain('AccountRulePanel');
        expect(editDialog).not.toContain('account-recognition-rule-panel');
        expect(editDialog).not.toContain('.account-rule-section :deep(.v-table__wrapper)');
    });
});
