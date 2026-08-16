import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('import dialog history rewrite acknowledgement', () => {
    test('confirm service receives selected preview ids and history rewrite acknowledgement', () => {
        const source = readSource('src/views/desktop/transactions/import/ImportDialog.vue');

        expect(source).toContain('buildHistoryRewriteConfirmAcknowledgement');
        expect(source).toContain("selected_only: 'true'");
        expect(source).toContain('getSelectedHistoryRewriteOperations');
        expect(source).toContain('services.confirmImportPreview');
        expect(source).toContain('historyRewriteAcknowledgement');
        expect(source).toContain('if (!confirmed)');
    });
});
