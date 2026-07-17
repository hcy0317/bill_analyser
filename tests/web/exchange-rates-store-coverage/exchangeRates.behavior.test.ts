import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

const mockStorageData = new Map<string, string>();
const mockLocalStorage = {
    getItem: jest.fn((key: string) => mockStorageData.get(key) ?? null),
    setItem: jest.fn((key: string, value: string) => {
        mockStorageData.set(key, value);
    }),
    removeItem: jest.fn((key: string) => {
        mockStorageData.delete(key);
    })
};

Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: mockLocalStorage,
    writable: true
});

const mockGetLatestExchangeRates = jest.fn<(...args: any[]) => Promise<any>>();
const mockUpdateUserCustomExchangeRate = jest.fn<(...args: any[]) => Promise<any>>();
const mockDeleteUserCustomExchangeRate = jest.fn<(...args: any[]) => Promise<any>>();
const mockIsEquals = jest.fn<(left: unknown, right: unknown) => boolean>();
const mockDayEquals = jest.fn<(left: number, right: number) => boolean>();
const mockHourEquals = jest.fn<(left: number, right: number) => boolean>();
const mockGetCurrentUnixTime = jest.fn<() => number>();
const mockGetExchangedAmountByRate = jest.fn(
    (amount: number, fromRate: string, toRate: string): number | null => {
        const exchangeRate = Number.parseFloat(toRate) / Number.parseFloat(fromRate);
        return Number.isFinite(exchangeRate) ? amount * exchangeRate : null;
    }
);
const mockLoggerError = jest.fn();

jest.mock('@/core/base.ts', () => ({
    itemAndIndex: function* (items: any[]): Generator<[any, number]> {
        for (let index = 0; index < items.length; index++) {
            yield [items[index], index];
        }
    }
}));
jest.mock('@/lib/common.ts', () => ({
    isEquals: (...args: [unknown, unknown]) => mockIsEquals(...args)
}));
jest.mock('@/lib/datetime.ts', () => ({
    isUnixTimeYearMonthDayEquals: (...args: [number, number]) => mockDayEquals(...args),
    isUnixTimeYearMonthDayHourEquals: (...args: [number, number]) => mockHourEquals(...args),
    getCurrentUnixTime: () => mockGetCurrentUnixTime()
}));
jest.mock('@/lib/numeral.ts', () => ({
    getExchangedAmountByRate: (...args: [number, string, string]) => mockGetExchangedAmountByRate(...args)
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { error: mockLoggerError }
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getLatestExchangeRates: (...args: any[]) => mockGetLatestExchangeRates(...args),
        updateUserCustomExchangeRate: (...args: any[]) => mockUpdateUserCustomExchangeRate(...args),
        deleteUserCustomExchangeRate: (...args: any[]) => mockDeleteUserCustomExchangeRate(...args)
    }
}));

import { useExchangeRatesStore } from '@/stores/exchangeRates.ts';

const exchangeRatesKey = 'ebk_app_exchange_rates';
const providerKey = 'ebk_app_exchange_rates_provider';
const now = 1_752_552_000;

function rate(currency: string, value: string): any {
    return { currency, rate: value };
}

function response(overrides: Record<string, unknown> = {}): any {
    return {
        requestedProvider: 'auto',
        providerKey: 'ecb',
        fallbackUsed: false,
        dataSource: 'provider',
        referenceUrl: 'https://rates.invalid/synthetic',
        updateTime: now,
        baseCurrency: 'CNY',
        exchangeRates: [rate('CNY', '1'), rate('USD', '0.14'), rate('EUR', '0.13')],
        ...overrides
    };
}

function envelope(result: any): Promise<any> {
    return Promise.resolve({ data: { success: true, result } });
}

function invalidEnvelope(data: unknown): Promise<any> {
    return Promise.resolve({ data });
}

function persistLatest(value: unknown): void {
    mockStorageData.set(exchangeRatesKey, JSON.stringify(value));
}

function freshStore(): any {
    setActivePinia(createPinia());
    return useExchangeRatesStore() as any;
}

function deferred<T>(): {
    promise: Promise<T>;
    resolve: (value: T) => void;
    reject: (reason: unknown) => void;
} {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockStorageData.clear();
    mockGetCurrentUnixTime.mockReturnValue(now);
    mockDayEquals.mockReturnValue(false);
    mockHourEquals.mockReturnValue(false);
    mockIsEquals.mockImplementation((left, right) => JSON.stringify(left) === JSON.stringify(right));
    mockGetExchangedAmountByRate.mockImplementation(
        (amount: number, fromRate: string, toRate: string): number | null => {
            const exchangeRate = Number.parseFloat(toRate) / Number.parseFloat(fromRate);
            return Number.isFinite(exchangeRate) ? amount * exchangeRate : null;
        }
    );
});

describe('exchange-rates store state, persistence, and getters', () => {
    test('loads defaults from empty storage and exposes empty getter states', () => {
        const store = freshStore();

        expect(store.latestExchangeRates).toStrictEqual({});
        expect(store.selectedExchangeRateProvider).toBe('auto');
        expect(store.effectiveRequestedProvider).toBe('auto');
        expect(store.isUserCustomExchangeRates).toBe(false);
        expect(store.exchangeRatesLastUpdateTime).toBeNull();
        expect(store.latestExchangeRateMap).toStrictEqual({});

        store.latestExchangeRates = null;
        store.selectedExchangeRateProvider = '';
        expect(store.effectiveRequestedProvider).toBe('auto');
        expect(store.isUserCustomExchangeRates).toBe(false);
        expect(store.exchangeRatesLastUpdateTime).toBeNull();
        expect(store.latestExchangeRateMap).toStrictEqual({});
    });

    test('hydrates provider and rates, maps base currency, and identifies user-custom data', () => {
        const custom = response({
            requestedProvider: 'user_custom',
            dataSource: 'user_custom',
            exchangeRates: [rate('CNY', '1'), rate('USD', '0.15')]
        });
        persistLatest({ time: now - 10, data: custom });
        mockStorageData.set(providerKey, 'custom-provider');

        const store = freshStore();
        expect(store.selectedExchangeRateProvider).toBe('custom-provider');
        expect(store.effectiveRequestedProvider).toBe('custom-provider');
        expect(store.isUserCustomExchangeRates).toBe(true);
        expect(store.exchangeRatesLastUpdateTime).toBe(now);
        expect(store.latestExchangeRateMap).toStrictEqual({
            CNY: rate('CNY', '1'),
            USD: rate('USD', '0.15')
        });

        store.latestExchangeRates.data.dataSource = 'provider';
        expect(store.isUserCustomExchangeRates).toBe(false);
        store.latestExchangeRates = { data: {} };
        expect(store.latestExchangeRateMap).toStrictEqual({});
    });

    test('persists provider normalization and resets cached rates', () => {
        const store = freshStore();
        store.setSelectedExchangeRateProvider('ecb');
        expect(store.selectedExchangeRateProvider).toBe('ecb');
        expect(mockStorageData.get(providerKey)).toBe('ecb');

        store.setSelectedExchangeRateProvider('');
        expect(store.selectedExchangeRateProvider).toBe('auto');
        expect(mockStorageData.get(providerKey)).toBe('auto');

        store.latestExchangeRates = { time: now, data: response() };
        persistLatest(store.latestExchangeRates);
        store.resetLatestExchangeRates();
        expect(store.latestExchangeRates).toStrictEqual({});
        expect(mockStorageData.has(exchangeRatesKey)).toBe(false);
        expect(mockLocalStorage.removeItem).toHaveBeenCalledWith(exchangeRatesKey);
    });
});

describe('exchange-rates store load, cache, provider, and error contracts', () => {
    test('returns same-day cache only for the same requested provider', async () => {
        const cached = response({ requestedProvider: 'ecb' });
        persistLatest({ time: now - 100, data: cached });
        mockStorageData.set(providerKey, 'ecb');
        mockDayEquals.mockReturnValue(true);
        const store = freshStore();

        await expect(store.getLatestExchangeRates({ silent: false, force: false })).resolves.toStrictEqual(cached);
        expect(mockDayEquals).toHaveBeenCalledWith(now, now);
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();

        store.selectedExchangeRateProvider = 'frankfurter';
        mockGetLatestExchangeRates.mockReturnValueOnce(envelope(response({ requestedProvider: 'frankfurter' })));
        await store.getLatestExchangeRates({ silent: false, force: false });
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({
            ignoreError: false,
            provider: 'frankfurter'
        });
    });

    test('uses same-hour cache after the day cache misses and fetches after both expire', async () => {
        const cached = response();
        persistLatest({ time: now - 30, data: cached });
        mockHourEquals.mockReturnValue(true);
        const store = freshStore();

        await expect(store.getLatestExchangeRates({ silent: true, force: false })).resolves.toStrictEqual(cached);
        expect(mockHourEquals).toHaveBeenCalledWith(now - 30, now);
        expect(mockGetLatestExchangeRates).not.toHaveBeenCalled();

        mockHourEquals.mockReturnValue(false);
        const fresh = response({ updateTime: now + 1 });
        mockGetLatestExchangeRates.mockReturnValueOnce(envelope(fresh));
        await expect(store.getLatestExchangeRates({ silent: true, force: false })).resolves.toBe(fresh);
        expect(mockGetLatestExchangeRates).toHaveBeenCalledWith({ ignoreError: true, provider: 'auto' });
    });

    test('fetches provider fallback envelopes and preserves requested, effective, and base rates', async () => {
        mockStorageData.set(providerKey, 'ecb');
        const fallback = response({
            requestedProvider: 'ecb',
            providerKey: 'builtin',
            fallbackUsed: true,
            dataSource: 'builtin_fallback',
            exchangeRates: [rate('CNY', '1'), rate('USD', '0.14')]
        });
        mockGetLatestExchangeRates.mockReturnValueOnce(envelope(fallback));
        const store = freshStore();

        await expect(store.getLatestExchangeRates({ silent: false, force: true })).resolves.toBe(fallback);
        expect(store.latestExchangeRates).toStrictEqual({ time: now, data: fallback });
        expect(store.latestExchangeRateMap.CNY.rate).toBe('1');
        expect(store.latestExchangeRates.data).toMatchObject({
            requestedProvider: 'ecb',
            providerKey: 'builtin',
            fallbackUsed: true,
            dataSource: 'builtin_fallback',
            baseCurrency: 'CNY'
        });
        expect(JSON.parse(mockStorageData.get(exchangeRatesKey) ?? '{}')).toStrictEqual({ time: now, data: fallback });
    });

    test('rejects force refresh when persisted data is equal and accepts changed forced data', async () => {
        const cached = response();
        persistLatest({ time: now - 100, data: cached });
        mockGetLatestExchangeRates.mockReturnValueOnce(envelope(cached));
        const store = freshStore();

        await expect(store.getLatestExchangeRates({ silent: false, force: true })).rejects.toStrictEqual({
            message: 'Exchange rates data is up to date',
            isUpToDate: true
        });

        const changed = response({ updateTime: now + 1 });
        mockGetLatestExchangeRates.mockReturnValueOnce(envelope(changed));
        mockIsEquals.mockReturnValueOnce(false);
        await expect(store.getLatestExchangeRates({ silent: false, force: true })).resolves.toBe(changed);
    });

    test('rejects every invalid latest-rate service envelope', async () => {
        const store = freshStore();
        for (const invalid of [undefined, { success: false }, { success: true }]) {
            mockGetLatestExchangeRates.mockReturnValueOnce(invalidEnvelope(invalid));
            await expect(store.getLatestExchangeRates({ silent: false, force: true })).rejects.toStrictEqual({
                message: 'Unable to retrieve exchange rates data'
            });
        }
    });

    test('normalizes processed, server-envelope, and generic latest-rate failures', async () => {
        const store = freshStore();
        const processed = { processed: true, message: 'already displayed' };
        mockGetLatestExchangeRates.mockRejectedValueOnce(processed);
        await expect(store.getLatestExchangeRates({ silent: false, force: true })).rejects.toBe(processed);

        const serverData = { message: 'provider unavailable', code: 'rates_down' };
        mockGetLatestExchangeRates.mockRejectedValueOnce({ response: { data: serverData } });
        await expect(store.getLatestExchangeRates({ silent: false, force: true })).rejects.toStrictEqual({
            error: serverData
        });

        mockGetLatestExchangeRates.mockRejectedValueOnce({ processed: false });
        await expect(store.getLatestExchangeRates({ silent: false, force: true })).rejects.toStrictEqual({
            message: 'Unable to retrieve exchange rates data'
        });
        expect(mockLoggerError).toHaveBeenCalledTimes(3);
    });

    test('allows independent concurrent loads without adding synthetic loading or error state', async () => {
        const first = deferred<any>();
        const second = deferred<any>();
        mockGetLatestExchangeRates.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
        const store = freshStore();
        expect('loading' in store).toBe(false);
        expect('error' in store).toBe(false);

        const firstLoad = store.getLatestExchangeRates({ silent: false, force: true });
        const secondLoad = store.getLatestExchangeRates({ silent: true, force: true });
        expect(mockGetLatestExchangeRates).toHaveBeenCalledTimes(2);

        const secondResult = response({ providerKey: 'second', updateTime: now + 2 });
        second.resolve({ data: { success: true, result: secondResult } });
        await expect(secondLoad).resolves.toBe(secondResult);

        const firstResult = response({ providerKey: 'first', updateTime: now + 1 });
        first.resolve({ data: { success: true, result: firstResult } });
        await expect(firstLoad).resolves.toBe(firstResult);
        expect(store.latestExchangeRates.data.providerKey).toBe('first');
    });
});

describe('exchange-rates store user-custom CRUD contracts', () => {
    test('updates an existing user rate and appends a new rate without scaling numeric rates', async () => {
        const store = freshStore();
        store.latestExchangeRates = {
            time: now,
            data: response({
                dataSource: 'user_custom',
                exchangeRates: [rate('CNY', '1'), rate('USD', '0.14')]
            })
        };
        const updated = { currency: 'USD', rate: '0.15', updateTime: now + 1 };
        mockUpdateUserCustomExchangeRate.mockReturnValueOnce(envelope(updated));

        await expect(store.updateUserCustomExchangeRate({ currency: 'USD', rate: 0.15 })).resolves.toBe(updated);
        expect(mockUpdateUserCustomExchangeRate).toHaveBeenCalledWith({ currency: 'USD', rate: '0.15' });
        expect(store.latestExchangeRateMap.USD.rate).toBe('0.15');
        expect(store.exchangeRatesLastUpdateTime).toBe(now + 1);

        const added = { currency: 'JPY', rate: '-20', updateTime: now + 2 };
        mockUpdateUserCustomExchangeRate.mockReturnValueOnce(envelope(added));
        await store.updateUserCustomExchangeRate({ currency: 'JPY', rate: -20 });
        expect(mockUpdateUserCustomExchangeRate).toHaveBeenLastCalledWith({ currency: 'JPY', rate: '-20' });
        expect(store.latestExchangeRateMap.JPY.rate).toBe('-20');
        expect(mockLocalStorage.setItem).toHaveBeenCalledWith(exchangeRatesKey, expect.any(String));
    });

    test('resolves valid updates even when local cache shapes require early return', async () => {
        const store = freshStore();
        const result = { currency: 'USD', rate: '0', updateTime: now };
        for (const cache of [null, {}, { data: {} }]) {
            store.latestExchangeRates = cache;
            mockUpdateUserCustomExchangeRate.mockReturnValueOnce(envelope(result));
            await expect(store.updateUserCustomExchangeRate({ currency: 'USD', rate: 0 })).resolves.toBe(result);
        }
        expect(mockUpdateUserCustomExchangeRate).toHaveBeenLastCalledWith({ currency: 'USD', rate: '0' });
    });

    test('rejects invalid update envelopes and normalizes all update failures', async () => {
        const store = freshStore();
        for (const invalid of [undefined, { success: false }, { success: true }]) {
            mockUpdateUserCustomExchangeRate.mockReturnValueOnce(invalidEnvelope(invalid));
            await expect(store.updateUserCustomExchangeRate({ currency: 'USD', rate: 1 })).rejects.toStrictEqual({
                message: 'Unable to update user custom exchange rate'
            });
        }

        const serverData = { message: 'invalid custom rate' };
        mockUpdateUserCustomExchangeRate.mockRejectedValueOnce({ response: { data: serverData } });
        await expect(store.updateUserCustomExchangeRate({ currency: 'USD', rate: 1 })).rejects.toStrictEqual({
            error: serverData
        });

        mockUpdateUserCustomExchangeRate.mockRejectedValueOnce({ processed: false });
        await expect(store.updateUserCustomExchangeRate({ currency: 'USD', rate: 1 })).rejects.toStrictEqual({
            message: 'Unable to update user custom exchange rate'
        });

        const processed = { processed: true, message: 'shown' };
        mockUpdateUserCustomExchangeRate.mockRejectedValueOnce(processed);
        await expect(store.updateUserCustomExchangeRate({ currency: 'USD', rate: 1 })).rejects.toBe(processed);
    });

    test('deletes immediately, supports deferred removal, and leaves missing currencies unchanged', async () => {
        const store = freshStore();
        store.latestExchangeRates = {
            time: now,
            data: response({ exchangeRates: [rate('CNY', '1'), rate('USD', '0.14'), rate('EUR', '0.13')] })
        };
        mockDeleteUserCustomExchangeRate.mockReturnValue(envelope(true));

        await expect(store.deleteUserCustomExchangeRate({ currency: 'USD' })).resolves.toBe(true);
        expect(store.latestExchangeRateMap.USD).toBeUndefined();

        let deferredRemoval: (() => void) | undefined;
        const beforeResolve = jest.fn((remove: () => void) => {
            deferredRemoval = remove;
        });
        await store.deleteUserCustomExchangeRate({ currency: 'EUR', beforeResolve });
        expect(beforeResolve).toHaveBeenCalledTimes(1);
        expect(store.latestExchangeRateMap.EUR).toBeDefined();
        deferredRemoval?.();
        expect(store.latestExchangeRateMap.EUR).toBeUndefined();

        mockLocalStorage.setItem.mockClear();
        await store.deleteUserCustomExchangeRate({ currency: 'MISSING' });
        expect(mockLocalStorage.setItem).not.toHaveBeenCalled();
    });

    test('resolves deletes against empty cache shapes and rejects invalid delete envelopes', async () => {
        const store = freshStore();
        mockDeleteUserCustomExchangeRate.mockReturnValue(envelope(true));
        for (const cache of [null, {}, { data: {} }]) {
            store.latestExchangeRates = cache;
            await expect(store.deleteUserCustomExchangeRate({ currency: 'USD' })).resolves.toBe(true);
        }

        for (const invalid of [undefined, { success: false }, { success: true }]) {
            mockDeleteUserCustomExchangeRate.mockReturnValueOnce(invalidEnvelope(invalid));
            await expect(store.deleteUserCustomExchangeRate({ currency: 'USD' })).rejects.toStrictEqual({
                message: 'Unable to delete this user custom exchange rate'
            });
        }
    });

    test('normalizes server, generic, and processed delete failures', async () => {
        const store = freshStore();
        const serverData = { message: 'cannot delete base rate' };
        mockDeleteUserCustomExchangeRate.mockRejectedValueOnce({ response: { data: serverData } });
        await expect(store.deleteUserCustomExchangeRate({ currency: 'CNY' })).rejects.toStrictEqual({
            error: serverData
        });

        mockDeleteUserCustomExchangeRate.mockRejectedValueOnce({ processed: false });
        await expect(store.deleteUserCustomExchangeRate({ currency: 'USD' })).rejects.toStrictEqual({
            message: 'Unable to delete this user custom exchange rate'
        });

        const processed = { processed: true, message: 'shown' };
        mockDeleteUserCustomExchangeRate.mockRejectedValueOnce(processed);
        await expect(store.deleteUserCustomExchangeRate({ currency: 'USD' })).rejects.toBe(processed);
    });
});

describe('exchange-rates store cents conversion boundaries', () => {
    test('converts integer cents through independent rate strings without yuan scaling', () => {
        const store = freshStore();
        store.latestExchangeRates = {
            data: response({ exchangeRates: [rate('CNY', '1'), rate('USD', '0.125'), rate('NEG', '-2')] })
        };

        expect(store.getExchangedAmount(12_345, 'CNY', 'USD')).toBe(1_543.125);
        expect(mockGetExchangedAmountByRate).toHaveBeenCalledWith(12_345, '1', '0.125');
        expect(mockGetExchangedAmountByRate).not.toHaveBeenCalledWith(123.45, expect.anything(), expect.anything());
        expect(mockGetExchangedAmountByRate).not.toHaveBeenCalledWith(1_234_500, expect.anything(), expect.anything());
        expect(store.getExchangedAmount(-12_345, 'CNY', 'USD')).toBe(-1_543.125);
        expect(store.getExchangedAmount(12_345, 'CNY', 'NEG')).toBe(-24_690);
        expect(store.getExchangedAmount(12_345, 'CNY', 'CNY')).toBe(12_345);
    });

    test('handles zero amounts, missing cache or currencies, and invalid, zero, or negative rates', () => {
        const store = freshStore();
        expect(store.getExchangedAmount(0, 'MISSING', 'MISSING')).toBe(0);
        expect(store.getExchangedAmount(100, 'CNY', 'USD')).toBeNull();

        store.latestExchangeRates = null;
        expect(store.getExchangedAmount(100, 'CNY', 'USD')).toBeNull();
        store.latestExchangeRates = { data: {} };
        expect(store.getExchangedAmount(100, 'CNY', 'USD')).toBeNull();
        store.latestExchangeRates = { data: response({ exchangeRates: [rate('CNY', '1')] }) };
        expect(store.getExchangedAmount(100, 'MISSING', 'CNY')).toBeNull();
        expect(store.getExchangedAmount(100, 'CNY', 'MISSING')).toBeNull();

        store.latestExchangeRates = {
            data: response({
                exchangeRates: [
                    rate('CNY', '1'),
                    rate('INVALID', 'not-a-rate'),
                    rate('ZERO', '0'),
                    rate('NEG', '-2')
                ]
            })
        };
        expect(store.getExchangedAmount(100, 'CNY', 'INVALID')).toBeNull();
        expect(store.getExchangedAmount(100, 'ZERO', 'CNY')).toBeNull();
        expect(store.getExchangedAmount(100, 'CNY', 'ZERO')).toBe(0);
        expect(store.getExchangedAmount(100, 'NEG', 'CNY')).toBe(-50);
    });
});
