import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

function readImportDialogSource(): string {
    return fs.readFileSync(path.resolve(process.cwd(), 'src/views/desktop/transactions/import/ImportDialog.vue'), 'utf-8');
}

function readImportFlowProgressSource(): string {
    return fs.readFileSync(
        path.resolve(process.cwd(), 'src/views/desktop/transactions/import/import-dialog/useImportFlowProgress.ts'),
        'utf-8'
    );
}

describe('import dialog progress UI contract', () => {
    test('uses active import-flow progress instead of the obsolete StepsBar copy', () => {
        const dialogSource = readImportDialogSource();
        const progressSource = readImportFlowProgressSource();

        expect(dialogSource).not.toContain('<steps-bar');
        expect(dialogSource).toContain('import-flow-progress');
        expect(progressSource).toContain("'selectSource'");
        expect(progressSource).toContain("'parseStageRows'");
        expect(progressSource).toContain("'reviewPreview'");
        expect(progressSource).toContain("'confirmImport'");
        expect(progressSource).toContain("'result'");
    });

    test('keeps the parser-first v2 stages on the Rust REST import chain', () => {
        const source = readImportDialogSource();

        expect(source).toContain("fetchImportStage('/api/bills/import/v2/parse'");
        expect(source).toContain("fetchImportStage('/api/bills/import/v2/dedup'");
        expect(source).toContain('services.getImportSession');
        expect(source).toContain('services.confirmImportPreview');
    });
});
