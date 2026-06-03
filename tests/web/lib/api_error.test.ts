import { getApiErrorMessage, getApiErrorMessageOrDefault } from '@/lib/api_error.ts';

describe('api error message extraction', () => {
    test('prefers taxonomy error response bodies over generic axios messages', () => {
        expect(getApiErrorMessage({
            message: 'Request failed with status code 400',
            response: {
                data: {
                    success: false,
                    error: 'Invalid JSON bundle',
                },
            },
        })).toBe('Invalid JSON bundle');
    });

    test('falls back to direct messages and caller defaults', () => {
        expect(getApiErrorMessage({ message: 'Network Error' })).toBe('Network Error');
        expect(getApiErrorMessageOrDefault({}, 'Unable to import settings bundle')).toBe('Unable to import settings bundle');
    });
});
