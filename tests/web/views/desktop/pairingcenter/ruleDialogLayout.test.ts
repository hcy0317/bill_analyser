import fs from 'node:fs';
import path from 'node:path';

function readWebSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

function readRepoSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), '..', '..', relativePath), 'utf-8');
}

function extractDialogBlock(source: string, vModelMarker: string): string {
    const markerIndex = source.indexOf(vModelMarker);
    expect(markerIndex).toBeGreaterThanOrEqual(0);

    const dialogStart = source.lastIndexOf('<v-dialog', markerIndex);
    const dialogEnd = source.indexOf('</v-dialog>', markerIndex);
    expect(dialogStart).toBeGreaterThanOrEqual(0);
    expect(dialogEnd).toBeGreaterThan(dialogStart);

    return source.slice(dialogStart, dialogEnd + '</v-dialog>'.length);
}

function expectAddEditModalContract(block: string, options: {
    dialogClass: string;
    cardClass: string;
    contentClass: string;
    actionClass: string;
    maxWidth: string;
}): void {
    expect(block).toContain(`class="${options.dialogClass}"`);
    expect(block).toContain('width="calc(100vw - 32px)"');
    expect(block).toContain(`max-width="${options.maxWidth}"`);
    expect(block).toContain(`class="${options.cardClass} pa-2 pa-sm-4 pa-md-8"`);
    expect(block).toContain('<template #title>');
    expect(block).toContain('<h4 class="text-h4">');
    expect(block).toContain(`class="${options.contentClass} mt-md-4 pt-0"`);
    expect(block).toContain(`class="${options.actionClass} w-100 d-flex justify-center mt-2 mt-sm-4 mt-md-6 gap-4"`);
    expect(block).toContain('variant="tonal"');
    expect(block).toContain("tt('Cancel')");
    expect(block).toContain('color="primary"');
    expect(block).toContain("tt('Save')");

    expect(block).not.toContain('<v-toolbar');
    expect(block).not.toContain('density="compact"');
    expect(block).not.toContain('v-toolbar-title');
}

describe('desktop rule add/edit dialog layout contract', () => {
    test('category rule add/edit dialog uses account-edit modal hierarchy instead of compact toolbar chrome', () => {
        const source = readWebSource('src/views/desktop/pairingcenter/components/RuleCenterPanel.vue');
        const editDialog = extractDialogBlock(source, 'v-model="showEditDialog"');

        expectAddEditModalContract(editDialog, {
            dialogClass: 'rule-center-edit-dialog',
            cardClass: 'rule-center-edit-card',
            contentClass: 'rule-center-edit-dialog-content',
            actionClass: 'rule-center-edit-dialog-actions',
            maxWidth: '700',
        });

        expect(source).toContain('<!-- Test Dialog -->');
        expect(source).toContain('v-model="showDeleteDialog"');
        expect(source).toContain('v-model="showBulkDeleteDialog"');
        expect(source).toContain('<Teleport defer :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">');
    });

    test('account rule add/edit dialog uses account-edit modal hierarchy instead of compact toolbar chrome', () => {
        const source = readWebSource('src/views/desktop/pairingcenter/components/AccountRulePanel.vue');
        const editDialog = extractDialogBlock(source, 'v-model="showEditDialog"');

        expectAddEditModalContract(editDialog, {
            dialogClass: 'account-rule-edit-dialog',
            cardClass: 'account-rule-edit-card',
            contentClass: 'account-rule-edit-dialog-content',
            actionClass: 'account-rule-edit-dialog-actions',
            maxWidth: '720',
        });

        expect(source).toContain('v-model="showTestDialog"');
        expect(source).toContain('v-model="showDeleteDialog"');
        expect(source).toContain('<Teleport defer :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">');
    });

    test('style reference records add/edit modal hierarchy gate', () => {
        const styleReference = readRepoSource('.agents/skills/bill-analyser-ui-style-reference/SKILL.md');

        expect(styleReference).toContain('desktop add/edit forms');
        expect(styleReference).toContain('centered `text-h4` title slots');
        expect(styleReference).toContain('instead of compact toolbars');
    });
});
