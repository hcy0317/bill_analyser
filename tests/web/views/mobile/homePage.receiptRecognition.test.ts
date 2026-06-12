import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

const HOMEPAGE_PATH = 'src/views/mobile/HomePage.vue';

function extractHandler(source: string): string {
    return source.split('function onReceiptRecognitionChanged(result: RecognizedReceiptImageResponse): void {')[1]
        ?.split('function onPageAfterIn')[0] ?? '';
}

describe('mobile HomePage onReceiptRecognitionChanged field mapping', () => {
    test('imports new RecognizedReceiptImageResponse type and uses new contract fields only', () => {
        const source = readSource(HOMEPAGE_PATH);
        expect(source).toContain("import type { RecognizedReceiptImageResponse } from '@/models/large_language_model.ts';");
    });

    test('amount (yuan) is converted to cents via Math.round and pushed only when finite', () => {
        const handler = extractHandler(readSource(HOMEPAGE_PATH));
        expect(handler).toContain('Math.round(result.amount * 100)');
        expect(handler).toContain("typeof result.amount === 'number' && Number.isFinite(result.amount)");
        // Push occurs inside the guarded branch, NOT unconditionally.
        expect(handler).toMatch(/Number\.isFinite\(result\.amount\)\)\s*\{[\s\S]+?params\.push\(`sourceAmountCents=/);
    });

    test('tradeTime ISO is parsed via Date.parse and converted to unix seconds, with NaN guard', () => {
        const handler = extractHandler(readSource(HOMEPAGE_PATH));
        expect(handler).toContain('Date.parse(result.tradeTime)');
        expect(handler).toContain('!Number.isNaN(parsedMs)');
        expect(handler).toContain('Math.floor(parsedMs / 1000)');
        expect(handler).toMatch(/!Number\.isNaN\(parsedMs\)\)\s*\{[\s\S]+?params\.push\(`time=/);
    });

    test('description is URI-encoded into comment query param when present', () => {
        const handler = extractHandler(readSource(HOMEPAGE_PATH));
        expect(handler).toContain('encodeURIComponent(result.description)');
        expect(handler).toContain('comment=');
    });

    test('always appends noTransactionDraft=true', () => {
        const handler = extractHandler(readSource(HOMEPAGE_PATH));
        expect(handler).toContain('params.push(`noTransactionDraft=true`);');
    });

    test('navigates to /transaction/add joining params with &', () => {
        const handler = extractHandler(readSource(HOMEPAGE_PATH));
        expect(handler).toContain("props.f7router.navigate(`/transaction/add?${params.join('&')}`);");
    });

    test('removed ezbookkeeping-only contract fields are no longer referenced in the handler', () => {
        const handler = extractHandler(readSource(HOMEPAGE_PATH));
        expect(handler).not.toContain('result.type');
        expect(handler).not.toContain('result.categoryId');
        expect(handler).not.toContain('result.sourceAccountId');
        expect(handler).not.toContain('result.destinationAccountId');
        expect(handler).not.toContain('result.sourceAmountCents');
        expect(handler).not.toContain('result.destinationAmountCents');
        expect(handler).not.toContain('result.tagIds');
        expect(handler).not.toContain('result.comment');
        expect(handler).not.toContain('result.time');
    });
});
