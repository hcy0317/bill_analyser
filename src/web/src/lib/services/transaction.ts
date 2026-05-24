import type { TransactionListByMaxTimeRequest } from '@/models/transaction.ts';

function appendQueryParam(parts: string[], key: string, value: string | number | boolean): void {
    parts.push(`${key}=${encodeURIComponent(String(value))}`);
}

export function buildTransactionListQuery(req: TransactionListByMaxTimeRequest): string {
    const parts: string[] = [];

    if (req.maxTime > 0) {
        appendQueryParam(parts, 'max_time', req.maxTime);
    }

    if (req.minTime > 0) {
        appendQueryParam(parts, 'min_time', req.minTime);
    }

    appendQueryParam(parts, 'type', req.type);
    appendQueryParam(parts, 'categoryIds', req.categoryIds);
    appendQueryParam(parts, 'accountIds', req.accountIds);
    appendQueryParam(parts, 'tagIds', req.tagIds);
    appendQueryParam(parts, 'tagFilterType', req.tagFilterType);
    appendQueryParam(parts, 'amountFilter', req.amountFilter);
    appendQueryParam(parts, 'keyword', req.keyword);
    appendQueryParam(parts, 'page_size', req.count);
    appendQueryParam(parts, 'page', req.page);
    appendQueryParam(parts, 'with_count', req.withCount);

    return parts.join('&');
}
