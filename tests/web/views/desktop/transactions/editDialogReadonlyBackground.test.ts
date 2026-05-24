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
        expect(source).not.toMatch(/\.transaction-readonly-form\s+:deep/);
        expect(source).toMatch(/\.transaction-readonly-form\s+\.v-field\s*\{/);
        expect(source).toMatch(/\.transaction-readonly-form\s+\.v-field__overlay\s*\{/);
        expect(source).toMatch(/background-color:\s*rgba\(var\(--v-theme-on-surface\),\s*0\.08\)/);
    });

    test('scheduled and historical matching panels share the readonly surface background', () => {
        const editDialogSource = readSource('src/views/desktop/transactions/list/dialogs/EditDialog.vue');
        const billMatchingPanelSource = readSource('src/views/desktop/transactions/list/dialogs/BillMatchingPanel.vue');
        const readonlySurface = /background-color:\s*rgba\(var\(--v-theme-on-surface\),\s*0\.08\)\s*!important/;

        const recurringRule = editDialogSource.match(/\.recurring-match-card\s*\{(?<body>[^}]*)\}/);
        const historicalRule = billMatchingPanelSource.match(/\.bill-matching-card\s*\{(?<body>[^}]*)\}/);

        expect(recurringRule?.groups?.['body'] ?? '').toMatch(readonlySurface);
        expect(historicalRule?.groups?.['body'] ?? '').toMatch(readonlySurface);
        expect(recurringRule?.groups?.['body'] ?? '').toContain('border: 1px solid rgba(var(--v-theme-on-surface), 0.10)');
        expect(historicalRule?.groups?.['body'] ?? '').toContain('border: 1px solid rgba(var(--v-theme-on-surface), 0.10)');
    });

    test('historical matching candidates keep facts above remarks and actions visible', () => {
        const source = readSource('src/views/desktop/transactions/list/dialogs/BillMatchingPanel.vue');
        const titleSlot = source.match(/<template #title>(?<body>[\s\S]*?)<\/template>/)?.groups?.['body'] ?? '';
        const subtitleSlot = source.match(/<template #subtitle>(?<body>[\s\S]*?)<\/template>/)?.groups?.['body'] ?? '';
        const appendSlot = source.match(/<template #append>(?<body>[\s\S]*?)<\/template>/)?.groups?.['body'] ?? '';

        expect(titleSlot).toContain('getCandidateTopLineParts(candidate)');
        expect(titleSlot).toContain('getCandidateSecondLineParts(candidate)');
        expect(subtitleSlot).toContain('getCandidateRemark(candidate)');
        expect(titleSlot).not.toContain('getCandidateRemark(candidate)');
        expect(appendSlot).toContain('isBillMatchingCandidateReviewActionSupported(candidate)');
        expect(appendSlot).toContain("handleCandidateAction('accept', candidate)");
        expect(appendSlot).toContain("handleCandidateAction('reject', candidate)");
    });
});
