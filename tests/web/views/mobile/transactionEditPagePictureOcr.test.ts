import { describe, expect, test } from '@jest/globals';
import { readSource as readPlainSource, readVueSourceWithExternalBlocks } from '../../helpers/vueSource';

const EDIT_PAGE_PATH = 'src/views/mobile/transactions/EditPage.vue';
const PICTURES_PANEL_PATH = 'src/views/mobile/transactions/components/MobileTransactionPicturesPanel.vue';
const RECEIPT_DRAFT_HELPER_PATH = 'src/lib/receiptDraft.ts';

function readSource(relativePath: string): string {
    return relativePath === EDIT_PAGE_PATH
        ? readVueSourceWithExternalBlocks(relativePath)
        : readPlainSource(relativePath);
}

describe('mobile transaction EditPage picture OCR source contract', () => {
    const source = readSource(EDIT_PAGE_PATH);

    test('shows transaction pictures by default for add/edit transaction mode', () => {
        expect(source).toMatch(/const showTransactionPictures = ref<boolean>\(pageTypeAndMode\?\.type === TransactionEditPageType\.Transaction/);
        expect(source).toContain('&& isTransactionPicturesEnabled()');
        expect(source).not.toContain('alwaysShowTransactionPicturesInMobileTransactionEditPage);');
    });

    test('does not keep Add Picture behind the more action sheet', () => {
        expect(source).not.toMatch(/<f7-actions-button @click="showTransactionPictures = true">\{\{ tt\('Add Picture'\) \}\}/);
    });

    test('recognizes uploaded pictures only in add-transaction mode and converts yuan to cents', () => {
        const helperSource = readSource(RECEIPT_DRAFT_HELPER_PATH);

        expect(source).toContain('function shouldRecognizeUploadedPicture()');
        expect(source).toContain('mode.value === TransactionEditPageMode.Add');
        expect(source).toContain('transactionsStore.recognizeReceiptImage({ imageFile: pictureFile })');
        expect(source).toContain('applyReceiptDraftAutoFillToTransaction(transaction.value, result);');
        expect(helperSource).toContain('receiptDraftAmountToCents(field)');
        expect(helperSource).toContain('transaction.sourceAmountCents = Math.round(result.amount * 100)');
    });

    test('shows OCR draft candidates without auto-applying them', () => {
        const helperSource = readSource(RECEIPT_DRAFT_HELPER_PATH);

        expect(source).toContain('receiptDraftCandidateHints');
        expect(source).toContain('buildReceiptDraftCandidateHints(result.draft)');
        expect(source).toContain('@click="applyReceiptDraftCandidate(candidate)"');
        expect(source).toContain('applyReceiptDraftFieldToTransaction(transaction.value, candidate.key, candidate.field)');
        expect(helperSource).toContain('transaction.setCategoryId(categoryId);');
    });

    test('keeps upload and OCR busy states on the picture control', () => {
        const picturesPanelSource = readSource(PICTURES_PANEL_PATH);
        expect(source).toContain('recognizingPicture');
        expect(source).toContain(':uploading-picture="uploadingPicture"');
        expect(source).toContain(':recognizing-picture="recognizingPicture"');
        expect(picturesPanelSource).toMatch(/submitting \|\| uploadingPicture \|\| recognizingPicture \|\| removingPictureId/);
        expect(picturesPanelSource).toMatch(/uploadingPicture \|\| recognizingPicture/);
    });

    test('mobile picture panel uses OCR recognition copy and setup-oriented unconfigured message', () => {
        const picturesPanelSource = readSource(PICTURES_PANEL_PATH);

        expect(picturesPanelSource).toContain(":header=\"tt('OCR Recognition')\"");
        expect(source).toContain("showToast('OCR recognition requires configuration in Rule Center');");
    });
});
