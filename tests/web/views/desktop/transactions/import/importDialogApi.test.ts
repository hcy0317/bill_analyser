import { describe, expect, test } from '@jest/globals';

import {
    extractApiErrorMessage,
    formatTimeoutSeconds,
    isAbortError
} from '@/views/desktop/transactions/import/importDialogApi.ts';

describe('import dialog api helpers', () => {
    test('extractApiErrorMessage prefers structured api error fields', () => {
        expect(extractApiErrorMessage({ error: '错误', message: '消息' }, 'fallback')).toBe('错误');
        expect(extractApiErrorMessage({ message: '消息' }, 'fallback')).toBe('消息');
        expect(extractApiErrorMessage(null, 'fallback')).toBe('fallback');
    });

    test('recognizes DOM and plain abort errors', () => {
        expect(isAbortError(new DOMException('cancelled', 'AbortError'))).toBe(true);
        expect(isAbortError({ name: 'AbortError' })).toBe(true);
        expect(isAbortError(new Error('other'))).toBe(false);
    });

    test('formats timeout milliseconds as rounded seconds', () => {
        expect(formatTimeoutSeconds(90_000)).toBe('90秒');
        expect(formatTimeoutSeconds(1_499)).toBe('1秒');
    });
});
