import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

describe('mobile import preview parity', () => {
    test('mobile router exposes the import preview review route', () => {
        const router = readSource('src/router/mobile.ts');

        expect(router).toContain("import TransactionImportPreviewPage from '@/views/mobile/transactions/ImportPreviewPage.vue';");
        expect(router).toContain("path: '/transaction/import/preview'");
        expect(router).toContain('asyncResolve(TransactionImportPreviewPage)');
    });

    test('mobile import preview page wires signals, actions, filters, and history acknowledgement', () => {
        const source = readSource('src/views/mobile/transactions/ImportPreviewPage.vue');

        expect(source).toContain('buildImportPreviewSignalViewModel');
        expect(source).toContain('matchesImportPreviewSignalFilter');
        expect(source).toContain("'History Rewrite'");
        expect(source).toContain("'Learning Suggestion'");
        expect(source).toContain("'LLM Suggestion'");
        expect(source).toContain('services.reviewImportTransferDecision');
        expect(source).toContain('services.acceptMatchingCandidate');
        expect(source).toContain('services.rejectMatchingCandidate');
        expect(source).toContain('services.llmPreviewRecommendAccept');
        expect(source).toContain('buildImportPreviewHistoryRewriteAcknowledgement');
        expect(source).toContain('historyRewriteAcknowledgement: buildHistoryAcknowledgement()');
    });
});
