import fs from 'fs';
import path from 'path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('desktop transaction edit dialog readonly affordance', () => {
    test('readonly shading is scoped to Vuetify field controls instead of the whole form', () => {
        const source = readSource('src/views/desktop/transactions/list/dialogs/EditDialog.vue');
        const readonlyFormRule = source.match(/\.transaction-readonly-form\s*\{(?<body>[^}]*)\}/);

        expect(readonlyFormRule?.groups?.['body'] ?? '').not.toMatch(/background|border|border-radius|padding|margin/);
        expect(source).toMatch(/\.transaction-readonly-form\s+:deep\(\.v-field\)\s*\{/);
        expect(source).toMatch(/\.transaction-readonly-form\s+:deep\(\.v-field__overlay\)\s*\{/);
    });
});
