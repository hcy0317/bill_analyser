import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { CategoryType } from '@/core/category.ts';
import type { TransactionCategoryInfoResponse } from '@/models/transaction_category.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';

const mockGetAllTransactionCategories = jest.fn<() => Promise<CategoriesApiResponse>>();

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getAllTransactionCategories: () => mockGetAllTransactionCategories()
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

type CategoryResponseMap = Record<number, TransactionCategoryInfoResponse[]>;
type CategoriesApiResponse = {
    data: {
        success: boolean;
        result: CategoryResponseMap;
    };
};

function createDeferred<T>(): {
    promise: Promise<T>;
    resolve: (value: T | PromiseLike<T>) => void;
    reject: (reason?: unknown) => void;
} {
    let resolve!: (value: T | PromiseLike<T>) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });

    return { promise, resolve, reject };
}

function buildCategoryResponse(): CategoryResponseMap {
    return {
        [CategoryType.Expense]: [
            {
                id: 'expense-1',
                name: '餐饮',
                parentId: '0',
                type: CategoryType.Expense,
                icon: 'mdi-food',
                color: '#ff9800',
                comment: '',
                displayOrder: 1,
                hidden: false,
                visible: true,
                subCategories: []
            } as TransactionCategoryInfoResponse
        ],
        [CategoryType.Income]: [],
        [CategoryType.Transfer]: [],
        [CategoryType.Investment]: []
    };
}

describe('transactionCategory store loadAllCategories', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        mockGetAllTransactionCategories.mockReset();
    });

    test('deduplicates concurrent loadAllCategories calls when force is false', async () => {
        const store = useTransactionCategoriesStore();
        const deferred = createDeferred<CategoriesApiResponse>();
        mockGetAllTransactionCategories.mockReturnValue(deferred.promise);

        const firstPromise = store.loadAllCategories({ force: false });
        const secondPromise = store.loadAllCategories({ force: false });

        expect(mockGetAllTransactionCategories).toHaveBeenCalledTimes(1);

        deferred.resolve({
            data: {
                success: true,
                result: buildCategoryResponse()
            }
        });

        const [firstResult, secondResult] = await Promise.all([firstPromise, secondPromise]);

        expect(mockGetAllTransactionCategories).toHaveBeenCalledTimes(1);
        expect(firstResult).toStrictEqual(secondResult);
        expect(store.transactionCategoryListStateInvalid).toBe(false);
        expect(store.allTransactionCategories[CategoryType.Expense]).toHaveLength(1);
        expect(store.allTransactionCategoriesMap['expense-1']?.name).toBe('餐饮');
    });

    test('retries after a shared in-flight request fails', async () => {
        const store = useTransactionCategoriesStore();
        const deferred = createDeferred<CategoriesApiResponse>();
        mockGetAllTransactionCategories
            .mockReturnValueOnce(deferred.promise)
            .mockResolvedValueOnce({
                data: {
                    success: true,
                    result: buildCategoryResponse()
                }
            });

        const firstPromise = store.loadAllCategories({ force: false });
        const secondPromise = store.loadAllCategories({ force: false });

        deferred.reject({ processed: false, message: 'network error' });

        await expect(firstPromise).rejects.toBeDefined();
        await expect(secondPromise).rejects.toBeDefined();

        await expect(store.loadAllCategories({ force: false })).resolves.toMatchObject({
            [CategoryType.Expense]: expect.any(Array)
        });
        expect(mockGetAllTransactionCategories).toHaveBeenCalledTimes(2);
    });

    test('returns cached categories without refetching after a successful load', async () => {
        const store = useTransactionCategoriesStore();
        mockGetAllTransactionCategories.mockResolvedValue({
            data: {
                success: true,
                result: buildCategoryResponse()
            }
        });

        await store.loadAllCategories({ force: false });
        await store.loadAllCategories({ force: false });

        expect(mockGetAllTransactionCategories).toHaveBeenCalledTimes(1);
    });

    test('does not repopulate store when reset happens during an in-flight load', async () => {
        const store = useTransactionCategoriesStore();
        const deferred = createDeferred<CategoriesApiResponse>();
        mockGetAllTransactionCategories.mockReturnValue(deferred.promise);

        const loadPromise = store.loadAllCategories({ force: false });

        store.resetTransactionCategories();
        deferred.resolve({
            data: {
                success: true,
                result: buildCategoryResponse()
            }
        });

        await expect(loadPromise).resolves.toStrictEqual({});
        expect(store.allTransactionCategories).toStrictEqual({});
        expect(store.allTransactionCategoriesMap).toStrictEqual({});
        expect(store.transactionCategoryListStateInvalid).toBe(true);
    });
});