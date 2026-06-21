import type { TransactionPictureInfoBasicResponse } from '@/models/transaction_picture_info.ts';

export const INTEGER_CENTS_PATTERN = /^[+-]?\d+$/u;

export function parseStrictQueryCents(value: unknown): number | undefined {
    if (typeof value !== 'string') {
        return undefined;
    }

    const text = value.trim();
    if (!INTEGER_CENTS_PATTERN.test(text)) {
        return undefined;
    }

    const parsed = Number(text);
    return Number.isSafeInteger(parsed) ? parsed : undefined;
}

export function getFontClassByAmount(amount: number): string {
    if (amount >= 100000000 || amount <= -100000000) {
        return 'ebk-small-amount';
    }

    if (amount >= 1000000 || amount <= -1000000) {
        return 'ebk-normal-amount';
    }

    return 'ebk-large-amount';
}

export function buildTransactionPictureItems(pictures: readonly TransactionPictureInfoBasicResponse[] | undefined, getTransactionPictureUrl: (picture: TransactionPictureInfoBasicResponse) => string | undefined): Record<string, string | undefined>[] {
    if (!pictures || !pictures.length) {
        return [];
    }

    return pictures.map(picture => ({
        url: getTransactionPictureUrl(picture)
    }));
}

export function buildTransactionThumbs(pictures: readonly TransactionPictureInfoBasicResponse[] | undefined, getTransactionPictureUrl: (picture: TransactionPictureInfoBasicResponse) => string | undefined): (string | undefined)[] {
    if (!pictures || !pictures.length) {
        return [];
    }

    return pictures.map(picture => getTransactionPictureUrl(picture));
}
