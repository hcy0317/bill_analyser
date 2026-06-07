import fs from 'fs';
import path from 'path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('desktop transaction edit dialog readonly affordance', () => {
    test('readonly controls use immutable field styling instead of gray shallow fill', () => {
        const source = readSource('src/views/desktop/transactions/list/dialogs/EditDialog.vue');
        const readonlyFormRule = source.match(/\.transaction-readonly-form\s*\{(?<body>[^}]*)\}/);
        const readonlyFieldRule = source.match(/\.transaction-readonly-form\s+\.v-field\s*\{(?<body>[^}]*)\}/);
        const readonlyOverlayRule = source.match(/\.transaction-readonly-form\s+\.v-field__overlay\s*\{(?<body>[^}]*)\}/);

        expect(readonlyFormRule?.groups?.['body'] ?? '').not.toMatch(/background|border|border-radius|padding|margin/);
        expect(source).not.toMatch(/\.transaction-readonly-form\s+:deep/);
        expect(source).toMatch(/\.transaction-readonly-form\s+\.v-field\s*\{/);
        expect(source).toMatch(/\.transaction-readonly-form\s+\.v-field__overlay\s*\{/);
        expect(source).toMatch(/\.transaction-readonly-form\s+\.v-input--readonly\s+\.v-field\s*\{/);
        expect(readonlyFieldRule?.groups?.['body'] ?? '').not.toMatch(/background-color:\s*rgba\(var\(--v-theme-on-surface\),\s*0\.08\)/);
        expect(readonlyFieldRule?.groups?.['body'] ?? '').toContain('background-color: transparent !important;');
        expect(readonlyFieldRule?.groups?.['body'] ?? '').toContain('border-color: rgba(var(--v-theme-on-surface), 0.24) !important;');
        expect(readonlyOverlayRule?.groups?.['body'] ?? '').toContain('opacity: 0 !important;');
        expect(source.match(/\.transaction-readonly-form\s+\.v-input--readonly\s+\.v-field\s*\{(?<body>[^}]*)\}/)?.groups?.['body'] ?? '')
            .toContain('pointer-events: none;');
    });

    test('readonly transaction type and geo controls cannot open editing selectors', () => {
        const source = readSource('src/views/desktop/transactions/list/dialogs/EditDialog.vue');
        const readonlyTabsRule = source.match(/\.transaction-type-tabs-readonly\s+\.v-tab\s*\{(?<body>[^}]*)\}/);
        const readonlySelectedTabRule = source.match(/\.transaction-type-tabs-readonly\s+\.v-tab--selected\s*\{(?<body>[^}]*)\}/);

        expect(source).toContain(':aria-readonly="mode === TransactionEditPageMode.View"');
        expect(source).toContain(':disabled="loading || submitting || mode === TransactionEditPageMode.View" v-model="transaction.type"');
        expect(readonlyTabsRule?.groups?.['body'] ?? '').toContain('pointer-events: none;');
        expect(readonlyTabsRule?.groups?.['body'] ?? '').toContain('cursor: default !important;');
        expect(readonlySelectedTabRule?.groups?.['body'] ?? '').toContain('background-color: transparent !important;');
        expect(readonlySelectedTabRule?.groups?.['body'] ?? '').toContain('border: 1px solid rgba(var(--v-theme-on-surface), 0.24);');
        expect(source).toContain('v-model:menu="editableGeoMenuState"');
        expect(source).toContain('get: () => mode.value !== TransactionEditPageMode.View && geoMenuState.value');
        expect(source).toContain('geoMenuState.value = mode.value !== TransactionEditPageMode.View && value;');
        expect(source).toContain('if (mode.value === TransactionEditPageMode.View) {\n        return;\n    }\n\n    if (isSupportGetGeoLocationByClick() && setGeoLocationByClickMap.value)');
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

    test('historical matching empty state is compact and remains visible while editing', () => {
        const editDialogSource = readSource('src/views/desktop/transactions/list/dialogs/EditDialog.vue');
        const billMatchingPanelSource = readSource('src/views/desktop/transactions/list/dialogs/BillMatchingPanel.vue');

        expect(editDialogSource).toContain('mode !== TransactionEditPageMode.Add && editId');
        expect(editDialogSource).toContain(':disabled="loading || submitting || mode !== TransactionEditPageMode.View"');
        expect(billMatchingPanelSource).toContain("return tt('None');");
        expect(billMatchingPanelSource).not.toContain("{{ tt('No Historical Matching Candidates') }}");
    });
});
