import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

function extractFunction(source: string, functionName: string): string {
    const startIndex = source.indexOf(`function ${functionName}(`);
    expect(startIndex).toBeGreaterThanOrEqual(0);

    let depth = 0;
    let bodyStarted = false;

    for (let index = startIndex; index < source.length; index++) {
        const char = source[index];
        if (char === '{') {
            depth++;
            bodyStarted = true;
        } else if (char === '}') {
            depth--;
            if (bodyStarted && depth === 0) {
                return source.slice(startIndex, index + 1);
            }
        }
    }

    throw new Error(`Unable to extract function ${functionName}`);
}

describe('reconciliation statement dialog promise contract', () => {
    const sourcePath = 'src/views/desktop/accounts/list/dialogs/ReconciliationStatementDialog.vue';

    test('resolves the open promise on normal close instead of rejecting with undefined', () => {
        const source = readSource(sourcePath);
        const closeFunction = extractFunction(source, 'close');

        expect(source).toContain('let resolveFunc: (() => void) | null = null;');
        expect(source).toContain('return new Promise<void>((resolve) => {');
        expect(source).toContain('resolveFunc = resolve;');
        expect(closeFunction).toContain('settleOpenPromise();');
        expect(closeFunction).not.toContain('rejectFunc');
    });

    test('clears the stored promise resolver when load failure closes the dialog', () => {
        const source = readSource(sourcePath);

        expect(source).toContain('function settleOpenPromise(): void {');
        expect(source).toContain('resolveFunc = null;');
        expect(source).toContain("emit('error', error);");
        expect(source).toMatch(/showState\.value = false;\s+settleOpenPromise\(\);/);
        expect(source).toContain('watch(showState, (newValue, oldValue) => {');
    });
});
