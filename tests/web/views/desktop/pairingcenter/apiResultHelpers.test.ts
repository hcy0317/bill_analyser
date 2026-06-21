import { describe, expect, test } from '@jest/globals';

import {
    extractPayloadMessage,
    getRequestErrorMessage,
    requireApiSuccess,
} from '@/views/desktop/pairingcenter/components/apiResultHelpers.ts';

describe('rule center API result helpers', () => {
    test('extracts nested payload messages with a bounded depth', () => {
        expect(extractPayloadMessage({ error: { message: 'nested error' } })).toBe('nested error');
        expect(extractPayloadMessage({ error: { error: { error: { message: 'too deep' } } } })).toBeNull();
        expect(extractPayloadMessage(null)).toBeNull();
    });

    test('normalizes request errors and standard API success responses', () => {
        expect(getRequestErrorMessage(new Error('failed'), 'fallback')).toBe('failed');
        expect(getRequestErrorMessage('unknown', 'fallback')).toBe('fallback');
        expect(requireApiSuccess({ data: { success: true, result: 42 } }, 'fallback')).toBe(42);
        expect(() => requireApiSuccess({ data: { success: false, result: 0 } }, 'fallback')).toThrow('fallback');
    });
});
