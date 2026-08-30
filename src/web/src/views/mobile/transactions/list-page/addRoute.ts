import { TransactionType } from '@/core/transaction.ts';

export interface MobileTransactionAddRouteQuery {
    readonly maxTime?: number;
    readonly minTime?: number;
    readonly type: number;
    readonly categoryIds?: string;
    readonly accountIds?: string;
    readonly tagIds?: string;
}

export function buildMobileTransactionAddPath(
    query: MobileTransactionAddRouteQuery,
    categoryIdCount: number,
    accountIdCount: number,
    currentUnixTime: number
): string {
    const params: string[] = [];

    if (query.maxTime && query.minTime) {
        const transactionTime = query.maxTime < currentUnixTime
            ? query.maxTime
            : currentUnixTime < query.minTime ? query.minTime : undefined;
        if (transactionTime) params.push(`time=${transactionTime}`);
    }

    if (query.type !== TransactionType.ModifyBalance) params.push(`type=${query.type}`);
    if (categoryIdCount === 1) params.push(`categoryId=${query.categoryIds}`);
    if (accountIdCount === 1) params.push(`accountId=${query.accountIds}`);
    if (query.tagIds) params.push(`tagIds=${query.tagIds}`);

    return `/transaction/add?${params.join('&')}`;
}
