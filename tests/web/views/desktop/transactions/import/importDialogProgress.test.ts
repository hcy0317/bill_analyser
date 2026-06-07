import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

function readImportDialogSource(): string {
    return fs.readFileSync(path.resolve(process.cwd(), 'src/views/desktop/transactions/import/ImportDialog.vue'), 'utf-8');
}

describe('import dialog progress UI contract', () => {
    test('uses active import-flow progress instead of the obsolete StepsBar copy', () => {
        const source = readImportDialogSource();

        expect(source).not.toContain('<steps-bar');
        expect(source).toContain('import-flow-progress');
        expect(source).toContain("'selectSource'");
        expect(source).toContain("'parseStageRows'");
        expect(source).toContain("'reviewPreview'");
        expect(source).toContain("'confirmImport'");
        expect(source).toContain("'result'");
    });

    test('keeps the v2 parse endpoint on the Rust REST import chain', () => {
        const source = readImportDialogSource();

        expect(source).toContain("fetchImportStage('/api/bills/import/v2/parse'");
    });
});
