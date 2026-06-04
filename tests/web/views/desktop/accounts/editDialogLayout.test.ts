import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('desktop account edit dialog layout contract', () => {
    test('dialog width is constrained by viewport and embedded rule table scrolls locally', () => {
        const editDialog = readSource('src/views/desktop/accounts/list/dialogs/EditDialog.vue');
        const accountRulePanel = readSource('src/views/desktop/pairingcenter/components/AccountRulePanel.vue');

        expect(editDialog).toContain('width="calc(100vw - 32px)"');
        expect(editDialog).toContain(':max-width="account.type === AccountType.MultiSubAccounts.type ? 1000 : 800"');
        expect(editDialog).toContain('class="account-edit-card');
        expect(editDialog).toContain('class="account-edit-content');
        expect(editDialog).toContain('class="account-edit-window');
        expect(editDialog).toContain('.account-rule-section :deep(.v-table__wrapper)');
        expect(editDialog).toContain('overflow-x: auto;');

        expect(accountRulePanel).toContain('.account-recognition-rule-panel');
        expect(accountRulePanel).toContain('.account-rule-table :deep(.v-table__wrapper)');
        expect(accountRulePanel).toContain('overflow-x: auto;');
    });
});
