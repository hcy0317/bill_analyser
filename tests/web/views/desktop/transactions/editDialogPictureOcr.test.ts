import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

const EDIT_DIALOG_PATH = 'src/views/desktop/transactions/list/dialogs/EditDialog.vue';

describe('EditDialog picture OCR wiring', () => {
    test('add-mode picture uploads call OCR and keep the uploaded picture in the same client session', () => {
        const source = readSource(EDIT_DIALOG_PATH);

        expect(source).toContain('mode.value === TransactionEditPageMode.Add');
        expect(source).toContain('clientSessionId: clientSessionId.value');
        expect(source).toContain('await recognizeUploadedPicture(pictureFile);');
    });

    test('recognized receipt payload fills current add form using frontend cents contract', () => {
        const source = readSource(EDIT_DIALOG_PATH);

        expect(source).toContain('Math.round(result.amount * 100)');
        expect(source).toContain('transaction.value.time = Math.floor(parsedMs / 1000);');
        expect(source).toContain('transaction.value.comment = result.description;');
        expect(source).toContain("activeTab.value = 'basicInfo';");
    });

    test('OCR errors surface typed messages instead of silently swallowing the upload result', () => {
        const source = readSource(EDIT_DIALOG_PATH);

        expect(source).toContain("if (errorCode === 'provider_unconfigured')");
        expect(source).toContain("snackbar.value?.showError('Receipt recognition is not configured');");
        expect(source).toContain("snackbar.value?.showError('Unable to recognize image');");
    });
});
