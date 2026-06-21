import type { TransactionPictureInfoBasicResponse } from '@/models/transaction_picture_info.ts';

export const INTEGER_CENTS_PATTERN = /^[+-]?\d+$/u;

/** 严格解析 URL query 中的分单位金额，避免小数或脏字符串进入编辑表单。 */
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

/** 根据金额正负返回移动端交易金额的颜色类名。 */
export function getFontClassByAmount(amount: number): string {
    if (amount >= 100000000 || amount <= -100000000) {
        return 'ebk-small-amount';
    }

    if (amount >= 1000000 || amount <= -1000000) {
        return 'ebk-normal-amount';
    }

    return 'ebk-large-amount';
}

/** 构造移动端图片列表组件需要的图片 item，并保留不可解析图片的 undefined URL。 */
export function buildTransactionPictureItems(pictures: readonly TransactionPictureInfoBasicResponse[] | undefined, getTransactionPictureUrl: (picture: TransactionPictureInfoBasicResponse) => string | undefined): Record<string, string | undefined>[] {
    if (!pictures || !pictures.length) {
        return [];
    }

    return pictures.map(picture => ({
        url: getTransactionPictureUrl(picture)
    }));
}

/** 构造移动端图片预览缩略图数组，顺序与原图片列表保持一致。 */
export function buildTransactionThumbs(pictures: readonly TransactionPictureInfoBasicResponse[] | undefined, getTransactionPictureUrl: (picture: TransactionPictureInfoBasicResponse) => string | undefined): (string | undefined)[] {
    if (!pictures || !pictures.length) {
        return [];
    }

    return pictures.map(picture => getTransactionPictureUrl(picture));
}
