import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

const EDIT_DIALOG_PATH = 'src/views/desktop/transactions/list/dialogs/EditDialog.vue';
const PICTURES_PANEL_PATH = 'src/views/desktop/transactions/list/dialogs/TransactionPicturesPanel.vue';
const RECEIPT_DRAFT_HELPER_PATH = 'src/lib/receiptDraft.ts';

describe('EditDialog picture OCR wiring', () => {
    test('add-mode picture uploads call OCR and keep the uploaded picture in the same client session', () => {
        const source = readSource(EDIT_DIALOG_PATH);

        expect(source).toContain('mode.value === TransactionEditPageMode.Add');
        expect(source).toContain('clientSessionId: clientSessionId.value');
        expect(source).toContain('await recognizeUploadedPicture(pictureFile);');
    });

    test('recognized receipt payload fills current add form using frontend cents contract', () => {
        const source = readSource(EDIT_DIALOG_PATH);
        const helperSource = readSource(RECEIPT_DRAFT_HELPER_PATH);

        expect(source).toContain('applyReceiptDraftAutoFillToTransaction(transaction.value, result);');
        expect(helperSource).toContain('receiptDraftAmountToCents(field)');
        expect(helperSource).toContain('transaction.sourceAmount = Math.round(result.amount * 100)');
        expect(helperSource).toContain('transaction.time = Math.floor(parsedMs / 1000);');
        expect(helperSource).toContain('transaction.comment = result.description;');
        expect(source).toContain("activeTab.value = 'basicInfo';");
    });

    test('keeps low-confidence OCR fields as explicit candidates', () => {
        const source = readSource(EDIT_DIALOG_PATH);
        const helperSource = readSource(RECEIPT_DRAFT_HELPER_PATH);

        expect(source).toContain('receiptDraftCandidateHints');
        expect(source).toContain('buildReceiptDraftCandidateHints(result.draft)');
        expect(source).toContain('@click="applyReceiptDraftCandidate(candidate)"');
        expect(source).toContain('applyReceiptDraftFieldToTransaction(transaction.value, candidate.key, candidate.field)');
        expect(helperSource).toContain('transaction.setCategoryId(categoryId);');
        expect(helperSource).toContain('transaction.tagIds = Array.from(new Set([...transaction.tagIds, ...tagIds]));');
    });

    test('OCR errors surface typed messages instead of silently swallowing the upload result', () => {
        const source = readSource(EDIT_DIALOG_PATH);

        expect(source).toContain("if (errorCode === 'provider_unconfigured')");
        expect(source).toContain("snackbar.value?.showError('Receipt recognition is not configured');");
        expect(source).toContain("snackbar.value?.showError('Unable to recognize image');");
    });

    test('extracted picture panel keeps the parent upload and view/remove contract', () => {
        const editDialogSource = readSource(EDIT_DIALOG_PATH);
        const panelSource = readSource(PICTURES_PANEL_PATH);

        expect(editDialogSource).toContain('<transaction-pictures-panel');
        expect(editDialogSource).toContain(':get-picture-url="getTransactionPictureUrl"');
        expect(editDialogSource).toContain('@upload="uploadPicture"');
        expect(editDialogSource).toContain('@view-or-remove="viewOrRemovePicture"');
        expect(panelSource).toContain(':accept="SUPPORTED_IMAGE_EXTENSIONS"');
        expect(panelSource).toContain('@change="emit(\'upload\', $event)"');
        expect(panelSource).toContain('@click="emit(\'view-or-remove\', pictureInfo)"');
        expect(panelSource).toContain('if (!props.canAddPicture || props.submitting || props.recognizingPicture)');
        expect(panelSource).toContain('props.mode === TransactionEditPageMode.Add || props.mode === TransactionEditPageMode.Edit');
    });
});
