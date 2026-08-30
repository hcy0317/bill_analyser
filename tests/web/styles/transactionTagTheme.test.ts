import fs from 'fs';
import path from 'path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('transaction tag theme and overflow contract', () => {
    test('mobile chips use semantic theme colors and preserve long names', () => {
        const source = readSource('src/styles/mobile/transaction-tag.scss');

        expect(source).toContain('.chip.transaction-tag');
        expect(source).toContain('--f7-chip-text-color: var(--ebk-transaction-tag-chip-text-color)');
        expect(source).toContain('--f7-chip-bg-color: var(--ebk-transaction-tag-chip-bg-color)');
        expect(source).toContain('border-color: var(--ebk-transaction-tag-chip-border-color)');
        expect(source).toContain('text-overflow: ellipsis');
        expect(source).toContain('white-space: nowrap');
    });

    test('desktop chips use current Vuetify theme and flex-safe truncation', () => {
        const source = readSource('src/styles/desktop/global.scss');

        expect(source).toContain('--ebk-transaction-tag-chip-bg-color: rgba(var(--v-theme-primary)');
        expect(source).toContain('.v-chip.transaction-tag');
        expect(source).toContain('max-inline-size: min(100%, 18rem)');
        expect(source).toContain('.v-chip__content');
        expect(source).toContain('text-overflow: ellipsis');
    });

    test('primary mobile and desktop transaction surfaces retain full-name metadata', () => {
        const mobileList = readSource('src/views/mobile/transactions/components/MobileTransactionMonthBlock.vue');
        const mobileEdit = readSource('src/views/mobile/transactions/edit-page/EditPage.template.html');
        const desktopList = readSource('src/views/desktop/transactions/list/ListPage.template.html');
        const desktopEdit = readSource('src/views/desktop/transactions/list/dialogs/edit-dialog/EditDialog.template.html');

        for (const source of [mobileList, mobileEdit, desktopList, desktopEdit]) {
            expect(source).toContain(':title=');
            expect(source).toContain(':aria-label=');
        }
    });
});
