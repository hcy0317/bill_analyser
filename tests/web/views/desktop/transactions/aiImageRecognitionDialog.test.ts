import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

const DIALOG_PATH = 'src/views/desktop/transactions/list/dialogs/AIImageRecognitionDialog.vue';

describe('AIImageRecognitionDialog OCR contract wiring', () => {
    test('imports new RecognizeReceiptImageError + low confidence threshold from large_language_model model', () => {
        const source = readSource(DIALOG_PATH);
        expect(source).toContain("import type { RecognizedReceiptImageResponse, RecognizeReceiptImageError } from '@/models/large_language_model.ts';");
        expect(source).toContain("import { RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD } from '@/models/large_language_model.ts';");
    });

    test('on success branches on low confidence and surfaces a non-blocking warning before resolving', () => {
        const source = readSource(DIALOG_PATH);
        expect(source).toContain('if (response.confidence !== null && response.confidence < RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD)');
        expect(source).toContain("snackbar.value?.showMessage('Low confidence recognition, please verify');");
        // resolveFunc must be called so backfill still proceeds even with low confidence.
        expect(source).toMatch(/showMessage\('Low confidence recognition, please verify'\);[\s\S]+?resolveFunc\?\.\(response\);/);
    });

    test('rejects path drives error UI from store-emitted errorCode (not status / not raw text)', () => {
        const source = readSource(DIALOG_PATH);
        expect(source).toContain("const errorCode = typed && typeof typed.errorCode === 'string' ? typed.errorCode : 'unknown';");
        // 5 typed branches + unknown fallback, each with i18n key string literal.
        expect(source).toContain("if (errorCode === 'cancelled')");
        expect(source).toContain("if (errorCode === 'provider_unconfigured')");
        expect(source).toContain("snackbar.value?.showError('OCR recognition requires configuration in Rule Center');");
        expect(source).toContain("if (errorCode === 'timeout')");
        expect(source).toContain("snackbar.value?.showError('Recognition timed out, please try again');");
        expect(source).toContain("if (errorCode === 'parse_error')");
        expect(source).toContain("snackbar.value?.showError('Could not parse this image, please try a clearer one');");
        expect(source).toContain("if (errorCode === 'rate_limited')");
        expect(source).toContain("snackbar.value?.showError('Too many requests, please wait a moment');");
        expect(source).toContain("snackbar.value?.showError('Unable to recognize image');");
    });

    test('on cancelled errorCode the dialog stays idle (no error toast, no rejectFunc)', () => {
        const source = readSource(DIALOG_PATH);
        // cancelled branch must short-circuit before any showError call.
        expect(source).toMatch(/if \(errorCode === 'cancelled'\) \{[\s\S]+?cancelRecognizingUuid\.value = undefined;[\s\S]+?return;[\s\S]+?\}/);
    });

    test('provider_unconfigured does not close the dialog (no showState assignment, no rejectFunc)', () => {
        const source = readSource(DIALOG_PATH);
        // Narrow to the body of recognize().catch(...) so we do not pick up the open()/cancel() showState assignments.
        const recognizeBody = source.split('function recognize(): void {')[1]?.split('function cancelRecognize')[0] ?? '';
        const catchBody = recognizeBody.split('.catch(')[1] ?? '';
        // The catch handler must not flip showState.value = false anywhere — only the success branch sets it.
        expect(catchBody).not.toContain('showState.value = false');
        expect(catchBody).not.toContain('rejectFunc?.()');
    });

    test('does not send any cancelled form field to backend on user cancel (axios cancel only)', () => {
        const source = readSource(DIALOG_PATH);
        // Sanity check: dialog does not synthesize a "cancelled=true" form field; cancellation is purely axios cancel.
        expect(source).not.toContain('cancelled: true');
        expect(source).not.toContain("'cancelled': true");
        expect(source).not.toContain("formData.append('cancelled'");
    });

    test('untyped error catch (.canceled / .processed) is fully removed', () => {
        const source = readSource(DIALOG_PATH);
        expect(source).not.toContain('error.canceled');
        expect(source).not.toContain('error.processed');
    });
});

describe('AIImageRecognitionDialog cancel UX', () => {
    test('cancelRecognize still routes through transactionsStore.cancelRecognizeReceiptImage and clears uuid', () => {
        const source = readSource(DIALOG_PATH);
        expect(source).toContain('transactionsStore.cancelRecognizeReceiptImage(cancelRecognizingUuid.value);');
        expect(source).toMatch(/function cancelRecognize\(\): void \{[\s\S]+?cancelRecognizingUuid\.value = undefined;[\s\S]+?\}/);
    });
});

describe('AIImageRecognitionDialog model contract', () => {
    test('large_language_model model exports the new C2 contract types', () => {
        const source = readSource('src/models/large_language_model.ts');
        expect(source).toContain('export interface ReceiptImageProvenance');
        expect(source).toContain('readonly amount: number | null');
        expect(source).toContain('readonly tradeTime: string | null');
        expect(source).toContain('readonly description: string | null');
        expect(source).toContain('readonly provenance: ReceiptImageProvenance');
        expect(source).toContain('readonly confidence: number | null');
        expect(source).toContain('export type ReceiptImageErrorCode =');
        expect(source).toContain("'provider_unconfigured'");
        expect(source).toContain("'timeout'");
        expect(source).toContain("'parse_error'");
        expect(source).toContain("'cancelled'");
        expect(source).toContain("'rate_limited'");
        expect(source).toContain('export const RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD = 0.6;');
    });

    test('ListPage OCR backfill segment uses new contract fields and converts yuan -> cents', () => {
        const source = readSource('src/views/desktop/transactions/ListPage.vue');
        expect(source).toContain('aiImageRecognitionDialog.value?.open().then(result =>');
        expect(source).toContain('Math.round(result.amount * 100)');
        expect(source).toContain('Date.parse(result.tradeTime)');
        expect(source).toContain('comment: result.description ?? undefined');
        // Old ezbookkeeping fields no longer referenced in the OCR backfill block.
        const block = source.split('function addByRecognizingImage(): void {')[1]?.split('function ')[0] ?? '';
        expect(block).not.toContain('result.type');
        expect(block).not.toContain('result.categoryId');
        expect(block).not.toContain('result.sourceAccountId');
        expect(block).not.toContain('result.destinationAccountId');
        expect(block).not.toContain('result.sourceAmount');
        expect(block).not.toContain('result.tagIds');
    });
});
