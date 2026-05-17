import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

// Stub browser globals required by transitive store imports (settings/userstate read localStorage on init).
const memStore = new Map<string, string>();
(globalThis as unknown as { localStorage: Storage }).localStorage = {
    getItem: (k: string) => memStore.get(k) ?? null,
    setItem: (k: string, v: string) => { memStore.set(k, v); },
    removeItem: (k: string) => { memStore.delete(k); },
    clear: () => { memStore.clear(); },
    key: (i: number) => Array.from(memStore.keys())[i] ?? null,
    get length() { return memStore.size; }
} as unknown as Storage;
(globalThis as unknown as { window: { location: { pathname: string; origin: string } } }).window = {
    location: { pathname: '/', origin: 'http://localhost' }
};

import {
    mapReceiptImageErrorCode,
    buildRecognizeReceiptImageError,
    useTransactionsStore
} from '@/stores/transaction.ts';

type Resolver<T> = (value: T | PromiseLike<T>) => void;
type Rejector = (reason?: unknown) => void;

interface ServiceResponse {
    data: {
        success: boolean;
        result?: Record<string, unknown> | null;
    };
}

const mockRecognizeReceiptImage = jest.fn<(args: { imageFile: File; cancelableUuid?: string }) => Promise<ServiceResponse>>();
const mockCancelRequest = jest.fn<(uuid: string) => void>();

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        recognizeReceiptImage: (args: { imageFile: File; cancelableUuid?: string }) => mockRecognizeReceiptImage(args),
        cancelRequest: (uuid: string) => mockCancelRequest(uuid)
    }
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: jest.fn(),
        error: jest.fn()
    }
}));

// vue-i18n's useI18n() requires a Vue instance with the plugin installed; statistics store calls
// it at setup time. Stub the helper layer so all consuming stores can construct in node tests.
jest.mock('@/locales/helpers.ts', () => ({
    __esModule: true,
    useI18n: () => ({
        t: (key: string) => key,
        tt: (key: string) => key,
        ti: (key: string) => key,
        te: () => false,
        getLocale: () => 'en',
        setLocale: () => undefined,
        getCurrentLanguageInfo: () => ({ name: 'en', alternativeLanguageTag: 'en' }),
        getCurrentLanguageDisplayName: () => 'English',
        getCurrentLanguageTag: () => 'en'
    })
}));

function fakeFile(): File {
    return new File([new Uint8Array([1, 2, 3])], 'receipt.jpg', { type: 'image/jpeg' });
}

function deferred<T>(): { promise: Promise<T>; resolve: Resolver<T>; reject: Rejector } {
    let resolve!: Resolver<T>;
    let reject!: Rejector;
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return { promise, resolve, reject };
}

describe('mapReceiptImageErrorCode', () => {
    test.each<[string | undefined, number, string]>([
        ['provider_unconfigured', 501, 'provider_unconfigured'],
        ['timeout', 504, 'timeout'],
        ['parse_error', 422, 'parse_error'],
        ['cancelled', 499, 'cancelled'],
        ['rate_limited', 429, 'rate_limited'],
        [undefined, 501, 'provider_unconfigured'],
        [undefined, 504, 'timeout'],
        [undefined, 422, 'parse_error'],
        [undefined, 429, 'rate_limited'],
        ['weird_unknown_code', 500, 'unknown'],
        [undefined, 500, 'unknown']
    ])('maps rawCode=%s status=%s -> %s', (rawCode, status, expected) => {
        expect(mapReceiptImageErrorCode(rawCode, status)).toBe(expected);
    });
});

describe('buildRecognizeReceiptImageError', () => {
    test('returns plain shape with originalError omitted when not provided', () => {
        const err = buildRecognizeReceiptImageError('timeout', 'boom', 504);
        expect(err).toStrictEqual({
            errorCode: 'timeout',
            message: 'boom',
            status: 504,
            originalError: undefined
        });
    });

    test('preserves originalError when provided', () => {
        const orig = { foo: 'bar' };
        const err = buildRecognizeReceiptImageError('parse_error', 'm', 422, orig);
        expect(err.originalError).toBe(orig);
    });
});

describe('useTransactionsStore.recognizeReceiptImage', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        mockRecognizeReceiptImage.mockReset();
        mockCancelRequest.mockReset();
    });

    test('normalizes snake_case payload to camelCase contract', async () => {
        mockRecognizeReceiptImage.mockResolvedValue({
            data: {
                success: true,
                result: {
                    amount: 12.34,
                    trade_time: '2026-04-01T08:30:00Z',
                    description: 'Coffee',
                    payment_platform: 'wechat_pay',
                    provenance: { provider: 'openai', model: 'gpt-x', request_id: 'req-1' },
                    confidence: 0.91
                }
            }
        });

        const store = useTransactionsStore();
        const result = await store.recognizeReceiptImage({ imageFile: fakeFile() });

        expect(result).toStrictEqual({
            amount: 12.34,
            tradeTime: '2026-04-01T08:30:00Z',
            description: 'Coffee',
            paymentPlatform: 'wechat_pay',
            provenance: { provider: 'openai', model: 'gpt-x', requestId: 'req-1' },
            confidence: 0.91
        });
    });

    test('coerces missing fields to nulls and unknown provenance', async () => {
        mockRecognizeReceiptImage.mockResolvedValue({
            data: {
                success: true,
                result: {
                    amount: null,
                    trade_time: null,
                    description: null,
                    provenance: {},
                    confidence: null
                }
            }
        });

        const store = useTransactionsStore();
        const result = await store.recognizeReceiptImage({ imageFile: fakeFile() });

        expect(result.amount).toBeNull();
        expect(result.tradeTime).toBeNull();
        expect(result.description).toBeNull();
        expect(result.paymentPlatform).toBeNull();
        expect(result.confidence).toBeNull();
        expect(result.provenance.provider).toBe('unknown');
        expect(result.provenance.requestId).toBe('');
        expect(result.provenance.model).toBeUndefined();
    });

    test('normalizes receipt transaction draft auto-fill and candidate fields', async () => {
        mockRecognizeReceiptImage.mockResolvedValue({
            data: {
                success: true,
                result: {
                    amount: 18.5,
                    trade_time: '2026-04-01T08:30:00Z',
                    description: 'Coffee',
                    payment_platform: 'wechat_pay',
                    provenance: { provider: 'local_json_ocr', request_id: 'req-2' },
                    confidence: 0.88,
                    draft: {
                        auto_fill: {
                            type: { value: 'expense', confidence: 0.9, reason: 'payment keyword', evidence: ['支付'] },
                            amount: { value: 18.5, confidence: 0.95, reason: 'largest amount', evidence: ['18.50'], unit: 'yuan' },
                            category_id: { value: 900, confidence: 0.92, reason: 'rule', evidence: ['coffee'], label: 'Food / Coffee' },
                            source_account_id: { value: '1001', confidence: 0.88, reason: 'alias', evidence: ['wechat'] },
                            tag_ids: { value: [77, '88'], confidence: 0.82, reason: 'tags', evidence: ['coffee'] }
                        },
                        candidates: {
                            category_id: [
                                { value: '901', confidence: 0.62, reason: 'name', evidence: ['latte'], label: 'Food / Drink' }
                            ],
                            source_account_id: [
                                { value: 1002, confidence: 0.58, reason: 'alias', evidence: ['pay'] }
                            ],
                            tag_ids: [
                                { value: ['99'], confidence: 0.6, reason: 'tag', evidence: ['receipt'] }
                            ]
                        }
                    }
                }
            }
        });

        const store = useTransactionsStore();
        const result = await store.recognizeReceiptImage({ imageFile: fakeFile() });

        expect(result.draft?.autoFill.type?.value).toBe('expense');
        expect(result.draft?.autoFill.amount).toMatchObject({ value: 18.5, unit: 'yuan' });
        expect(result.draft?.autoFill.categoryId).toMatchObject({ value: '900', label: 'Food / Coffee' });
        expect(result.draft?.autoFill.sourceAccountId?.value).toBe('1001');
        expect(result.draft?.autoFill.tagIds?.value).toStrictEqual(['77', '88']);
        expect(result.draft?.candidates.categoryId?.[0]).toMatchObject({ value: '901', label: 'Food / Drink' });
        expect(result.draft?.candidates.sourceAccountId?.[0]).toMatchObject({ value: '1002' });
        expect(result.draft?.candidates.tagIds?.[0]).toMatchObject({ value: ['99'] });
    });

    test('rejects with unknown when payload missing success flag', async () => {
        mockRecognizeReceiptImage.mockResolvedValue({ data: { success: false } });

        const store = useTransactionsStore();
        await expect(store.recognizeReceiptImage({ imageFile: fakeFile() }))
            .rejects.toMatchObject({ errorCode: 'unknown' });
    });

    test.each<[number, string, string]>([
        [501, 'provider_unconfigured', 'provider_unconfigured'],
        [504, 'timeout', 'timeout'],
        [422, 'parse_error', 'parse_error'],
        [429, 'rate_limited', 'rate_limited']
    ])('maps backend status=%s errorCode=%s -> %s', async (status, errorCode, expected) => {
        mockRecognizeReceiptImage.mockRejectedValue({
            response: {
                status,
                data: { success: false, errorCode, errorMessage: 'backend says no' }
            }
        });

        const store = useTransactionsStore();
        await expect(store.recognizeReceiptImage({ imageFile: fakeFile() })).rejects.toMatchObject({
            errorCode: expected,
            status,
            message: 'backend says no'
        });
    });

    test('maps unknown errorCode from backend to "unknown"', async () => {
        mockRecognizeReceiptImage.mockRejectedValue({
            response: {
                status: 500,
                data: { success: false, errorCode: 'gibberish', errorMessage: 'oops' }
            }
        });

        const store = useTransactionsStore();
        await expect(store.recognizeReceiptImage({ imageFile: fakeFile() })).rejects.toMatchObject({
            errorCode: 'unknown',
            message: 'oops'
        });
    });

    test('axios cancel propagates as cancelled errorCode (not business error)', async () => {
        mockRecognizeReceiptImage.mockRejectedValue({ canceled: true });

        const store = useTransactionsStore();
        await expect(store.recognizeReceiptImage({ imageFile: fakeFile() })).rejects.toMatchObject({
            errorCode: 'cancelled',
            status: 499
        });
    });

    test('cancelRecognizeReceiptImage delegates to services.cancelRequest', () => {
        const store = useTransactionsStore();
        store.cancelRecognizeReceiptImage('uuid-xyz');
        expect(mockCancelRequest).toHaveBeenCalledWith('uuid-xyz');
    });

    test('does not resolve while service request is pending', async () => {
        const d = deferred<ServiceResponse>();
        mockRecognizeReceiptImage.mockReturnValue(d.promise);

        const store = useTransactionsStore();
        let settled = false;
        const p = store.recognizeReceiptImage({ imageFile: fakeFile() }).then(() => { settled = true; }, () => { settled = true; });

        await Promise.resolve();
        expect(settled).toBe(false);

        d.resolve({
            data: {
                success: true,
                result: {
                    amount: 1,
                    trade_time: '2026-01-01T00:00:00Z',
                    description: 'x',
                    provenance: { provider: 'p', request_id: 'r' },
                    confidence: 0.9
                }
            }
        });
        await p;
        expect(settled).toBe(true);
    });
});
