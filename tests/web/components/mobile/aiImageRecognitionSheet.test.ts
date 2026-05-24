import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

const SHEET_PATH = 'src/components/mobile/AIImageRecognitionSheet.vue';

describe('AIImageRecognitionSheet OCR contract wiring', () => {
    test('imports new RecognizeReceiptImageError + low confidence threshold from large_language_model model', () => {
        const source = readSource(SHEET_PATH);
        expect(source).toContain("import type { RecognizedReceiptImageResponse, RecognizeReceiptImageError } from '@/models/large_language_model.ts';");
        expect(source).toContain("import { RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD } from '@/models/large_language_model.ts';");
    });

    test('on success branches on low confidence and surfaces a non-blocking warning before emit', () => {
        const source = readSource(SHEET_PATH);
        expect(source).toContain('if (response.confidence !== null && response.confidence < RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD)');
        expect(source).toContain("showToast('Low confidence recognition, please verify');");
        // emit('recognition:change', response) MUST still fire even with low confidence so backfill proceeds.
        expect(source).toMatch(/showToast\('Low confidence recognition, please verify'\);[\s\S]+?emit\('recognition:change', response\);/);
    });

    test('rejects path drives error UI from store-emitted errorCode (not raw text / not error.message)', () => {
        const source = readSource(SHEET_PATH);
        expect(source).toContain("const errorCode = typed && typeof typed.errorCode === 'string' ? typed.errorCode : 'unknown';");
        expect(source).toContain("if (errorCode === 'cancelled')");
        expect(source).toContain("if (errorCode === 'provider_unconfigured')");
        expect(source).toContain("showToast('OCR recognition requires configuration in Rule Center');");
        expect(source).toContain("if (errorCode === 'timeout')");
        expect(source).toContain("showToast('Recognition timed out, please try again');");
        expect(source).toContain("if (errorCode === 'parse_error')");
        expect(source).toContain("showToast('Could not parse this image, please try a clearer one');");
        expect(source).toContain("if (errorCode === 'rate_limited')");
        expect(source).toContain("showToast('Too many requests, please wait a moment');");
        expect(source).toContain("showToast('Unable to recognize image');");
    });

    test('on cancelled errorCode the sheet stays idle (no error toast, no emit update:show=false in catch)', () => {
        const source = readSource(SHEET_PATH);
        // cancelled branch must short-circuit before any showToast call inside the catch.
        expect(source).toMatch(/if \(errorCode === 'cancelled'\) \{[\s\S]+?cancelRecognizingUuid\.value = undefined;[\s\S]+?return;[\s\S]+?\}/);
    });

    test('axios canceled sentinel is handled silently before errorCode branching', () => {
        const source = readSource(SHEET_PATH);
        // The catch handler must short-circuit on axios .canceled before any toast.
        const confirmBody = source.split('function confirm(): void {')[1]?.split('function cancelRecognize')[0] ?? '';
        const catchBody = confirmBody.split('.catch(')[1] ?? '';
        expect(catchBody).toContain('canceled');
        expect(catchBody).toMatch(/canceled\)?\s*\{[\s\S]+?return;/);
    });

    test('provider_unconfigured does not close the sheet (no emit update:show=false in that branch)', () => {
        const source = readSource(SHEET_PATH);
        const confirmBody = source.split('function confirm(): void {')[1]?.split('function cancelRecognize')[0] ?? '';
        const catchBody = confirmBody.split('.catch(')[1] ?? '';
        // The catch handler must not flip update:show to false anywhere — only the success branch does.
        expect(catchBody).not.toContain("emit('update:show', false)");
    });

    test('legacy untyped error catch (.processed / "error.message || error" toast) is fully removed', () => {
        const source = readSource(SHEET_PATH);
        expect(source).not.toContain('error.processed');
        expect(source).not.toContain('error.message || error');
    });

    test('cancelRecognize still routes through transactionsStore.cancelRecognizeReceiptImage and clears uuid', () => {
        const source = readSource(SHEET_PATH);
        expect(source).toContain('transactionsStore.cancelRecognizeReceiptImage(cancelRecognizingUuid.value);');
        expect(source).toMatch(/function cancelRecognize\(\): void \{[\s\S]+?cancelRecognizingUuid\.value = undefined;[\s\S]+?\}/);
    });

    test('does not synthesize a "cancelled=true" form field — cancellation is purely store-level', () => {
        const source = readSource(SHEET_PATH);
        expect(source).not.toContain('cancelled: true');
        expect(source).not.toContain("'cancelled': true");
    });
});

describe('AIImageRecognitionSheet i18n keys exist in en + zh_Hans', () => {
    const REQUIRED_KEYS = [
        'OCR recognition requires configuration in Rule Center',
        'Recognition timed out, please try again',
        'Could not parse this image, please try a clearer one',
        'Too many requests, please wait a moment',
        'Unable to recognize image',
        'Low confidence recognition, please verify'
    ];

    test('en.json contains all 5 errorCode messages + low confidence warning', () => {
        const en = readSource('src/locales/en.json');
        for (const key of REQUIRED_KEYS) {
            expect(en).toContain(`"${key}"`);
        }
    });

    test('zh_Hans.json contains all 5 errorCode messages + low confidence warning', () => {
        const zh = readSource('src/locales/zh_Hans.json');
        for (const key of REQUIRED_KEYS) {
            expect(zh).toContain(`"${key}"`);
        }
    });
});
