import {
    EMPTY_TRANSACTION_RESULT,
    Transaction,
    type TransactionPageWrapper
} from '@/models/transaction.ts';
import logger from '@/lib/logger.ts';
import services from '@/lib/services.ts';

import type { TransactionListFilter } from './types.ts';

export interface LoadTransactionsOptions {
    reload?: boolean;
    count?: number;
    page?: number;
    withCount?: boolean;
    autoExpand: boolean;
    defaultCurrency: string;
}

export interface LoadMonthlyTransactionsOptions {
    year: number;
    month: number;
    autoExpand: boolean;
    defaultCurrency: string;
}

interface TransactionListRequestCoordinatorDependencies {
    getFilter: () => TransactionListFilter;
    getNextTimeId: () => number;
    expandAccountIds: (accountIds: string[]) => string[];
    applyPage: (options: {
        transactionPageWrapper: TransactionPageWrapper;
        reload: boolean;
        autoExpand: boolean;
        defaultCurrency: string;
        nextTimeSequenceId?: number;
    }) => void;
    isListInvalid: () => boolean;
    updateListInvalidState: (invalid: boolean) => void;
}

/** 协调交易列表请求代次，确保陈旧响应不会覆盖最新列表。 */
export function createTransactionListRequestCoordinator(
    dependencies: TransactionListRequestCoordinatorDependencies
) {
    let requestGeneration = 0;
    let latestPageWrapper: TransactionPageWrapper = EMPTY_TRANSACTION_RESULT;

    function invalidate(): void {
        requestGeneration += 1;
    }

    function reset(): void {
        invalidate();
        latestPageWrapper = EMPTY_TRANSACTION_RESULT;
    }

    function beginRequest(): number {
        requestGeneration += 1;
        return requestGeneration;
    }

    function isLatest(generation: number): boolean {
        return generation === requestGeneration;
    }

    function expandFilterAccountIds(filter: TransactionListFilter, operation: string): string {
        let expandedAccountIds = filter.accountIds;
        if (expandedAccountIds) {
            const accountIdArray = expandedAccountIds.split(',').filter(id => id);
            if (accountIdArray.length > 0) {
                expandedAccountIds = dependencies.expandAccountIds(accountIdArray).join(',');
                logger.info(`[${operation}] 账户筛选展开: ${filter.accountIds} → ${expandedAccountIds}`);
            }
        }
        return expandedAccountIds;
    }

    function loadTransactions({
        reload,
        count,
        page,
        withCount,
        autoExpand,
        defaultCurrency
    }: LoadTransactionsOptions): Promise<TransactionPageWrapper> {
        const generation = beginRequest();
        const filter = dependencies.getFilter();
        let actualMaxTime = dependencies.getNextTimeId();
        if (reload && filter.maxTime > 0) {
            actualMaxTime = filter.maxTime * 1000 + 999;
        } else if (reload && filter.maxTime <= 0) {
            actualMaxTime = 0;
        }
        const expandedAccountIds = expandFilterAccountIds(filter, 'loadTransactions');

        return new Promise((resolve, reject) => {
            services.getTransactions({
                maxTime: actualMaxTime,
                minTime: filter.minTime * 1000,
                count: count || 50,
                page: page || 1,
                withCount: !!withCount,
                type: filter.type,
                categoryIds: filter.categoryIds,
                accountIds: expandedAccountIds,
                tagIds: filter.tagIds,
                tagFilterType: filter.tagFilterType,
                amountFilterCents: filter.amountFilterCents,
                keyword: filter.keyword
            }).then(response => {
                const data = response.data;
                if (!data || !data.success || !data.result) {
                    if (isLatest(generation) && reload) {
                        latestPageWrapper = EMPTY_TRANSACTION_RESULT;
                        dependencies.applyPage({
                            transactionPageWrapper: EMPTY_TRANSACTION_RESULT,
                            reload,
                            autoExpand,
                            defaultCurrency
                        });
                        if (!dependencies.isListInvalid()) {
                            dependencies.updateListInvalidState(true);
                        }
                    }
                    reject({ message: 'Unable to retrieve transaction list' });
                    return;
                }

                const transactionPageWrapper: TransactionPageWrapper = {
                    items: Transaction.ofMulti(data.result.items),
                    totalCount: data.result.totalCount
                };
                if (!isLatest(generation)) {
                    resolve(latestPageWrapper);
                    return;
                }

                latestPageWrapper = transactionPageWrapper;
                dependencies.applyPage({
                    transactionPageWrapper,
                    reload: !!reload,
                    autoExpand,
                    defaultCurrency,
                    nextTimeSequenceId: data.result.nextTimeSequenceId
                });
                if (reload && dependencies.isListInvalid()) {
                    dependencies.updateListInvalidState(false);
                }
                resolve(transactionPageWrapper);
            }).catch(error => {
                logger.error('failed to load transaction list', error);
                if (!isLatest(generation)) {
                    resolve(latestPageWrapper);
                    return;
                }

                if (reload) {
                    latestPageWrapper = EMPTY_TRANSACTION_RESULT;
                    dependencies.applyPage({
                        transactionPageWrapper: EMPTY_TRANSACTION_RESULT,
                        reload,
                        autoExpand,
                        defaultCurrency
                    });
                    if (!dependencies.isListInvalid()) {
                        dependencies.updateListInvalidState(true);
                    }
                }

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve transaction list' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function loadMonthlyAllTransactions({
        year,
        month,
        autoExpand,
        defaultCurrency
    }: LoadMonthlyTransactionsOptions): Promise<TransactionPageWrapper> {
        const generation = beginRequest();
        const filter = dependencies.getFilter();
        const expandedAccountIds = expandFilterAccountIds(filter, 'loadMonthlyAllTransactions');

        return new Promise((resolve, reject) => {
            services.getAllTransactionsByMonth({
                year,
                month,
                type: filter.type,
                categoryIds: filter.categoryIds,
                accountIds: expandedAccountIds,
                tagIds: filter.tagIds,
                tagFilterType: filter.tagFilterType,
                amountFilterCents: filter.amountFilterCents,
                keyword: filter.keyword
            }).then(response => {
                const data = response.data;
                if (!data || !data.success || !data.result) {
                    if (isLatest(generation)) {
                        latestPageWrapper = EMPTY_TRANSACTION_RESULT;
                        dependencies.applyPage({
                            transactionPageWrapper: EMPTY_TRANSACTION_RESULT,
                            reload: true,
                            autoExpand,
                            defaultCurrency
                        });
                        if (!dependencies.isListInvalid()) {
                            dependencies.updateListInvalidState(true);
                        }
                    }
                    reject({ message: 'Unable to retrieve transaction list' });
                    return;
                }

                const transactionPageWrapper: TransactionPageWrapper = {
                    items: Transaction.ofMulti(data.result.items),
                    totalCount: data.result.totalCount
                };
                if (!isLatest(generation)) {
                    resolve(latestPageWrapper);
                    return;
                }

                latestPageWrapper = transactionPageWrapper;
                dependencies.applyPage({
                    transactionPageWrapper,
                    reload: true,
                    autoExpand,
                    defaultCurrency
                });
                if (dependencies.isListInvalid()) {
                    dependencies.updateListInvalidState(false);
                }
                resolve(transactionPageWrapper);
            }).catch(error => {
                logger.error('failed to load monthly all transaction list', error);
                if (!isLatest(generation)) {
                    resolve(latestPageWrapper);
                    return;
                }

                latestPageWrapper = EMPTY_TRANSACTION_RESULT;
                dependencies.applyPage({
                    transactionPageWrapper: EMPTY_TRANSACTION_RESULT,
                    reload: true,
                    autoExpand,
                    defaultCurrency
                });
                if (!dependencies.isListInvalid()) {
                    dependencies.updateListInvalidState(true);
                }

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve transaction list' });
                } else {
                    reject(error);
                }
            });
        });
    }

    return {
        invalidate,
        reset,
        loadTransactions,
        loadMonthlyAllTransactions
    };
}
