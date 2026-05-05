import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

const EDIT_PAGE_PATH = 'src/views/mobile/transactions/EditPage.vue';
const PICTURES_PANEL_PATH = 'src/views/mobile/transactions/components/MobileTransactionPicturesPanel.vue';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
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
        expect(source).toContain('function shouldRecognizeUploadedPicture()');
        expect(source).toContain('mode.value === TransactionEditPageMode.Add');
        expect(source).toContain('transactionsStore.recognizeReceiptImage({ imageFile: pictureFile })');
        expect(source).toContain('transaction.value.sourceAmount = Math.round(result.amount * 100)');
    });

    test('keeps upload and OCR busy states on the picture control', () => {
        const picturesPanelSource = readSource(PICTURES_PANEL_PATH);
        expect(source).toContain('recognizingPicture');
        expect(source).toContain(':uploading-picture="uploadingPicture"');
        expect(source).toContain(':recognizing-picture="recognizingPicture"');
        expect(picturesPanelSource).toMatch(/submitting \|\| uploadingPicture \|\| recognizingPicture \|\| removingPictureId/);
        expect(picturesPanelSource).toMatch(/uploadingPicture \|\| recognizingPicture/);
    });
});
