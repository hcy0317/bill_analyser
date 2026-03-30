import { describe, expect, test } from '@jest/globals';

import {
    TransactionTag,
    type TransactionTagInfoResponse
} from '@/models/transaction_tag.ts';

const SAMPLE_TAG: TransactionTagInfoResponse = {
    id: '10',
    name: '午饭',
    displayOrder: 2,
    hidden: false
};

describe('TransactionTag model', () => {
    test('TransactionTag.of normalizes id into string form', () => {
        const tag = TransactionTag.of({
            ...SAMPLE_TAG,
            id: 10 as unknown as string
        });

        expect(tag.id).toBe('10');
        expect(tag.name).toBe('午饭');
        expect(tag.displayOrder).toBe(2);
        expect(tag.hidden).toBe(false);
    });

    test('TransactionTag.ofMulti maps every response', () => {
        expect(TransactionTag.ofMulti([SAMPLE_TAG, { ...SAMPLE_TAG, id: '11', name: '通勤' }])).toHaveLength(2);
    });

    test('TransactionTag request converters and defaults stay stable', () => {
        const tag = TransactionTag.of(SAMPLE_TAG);
        const emptyTag = TransactionTag.createNewTag();
        const namedTag = TransactionTag.createNewTag('娱乐');

        expect(tag.toCreateRequest()).toStrictEqual({ name: '午饭' });
        expect(tag.toModifyRequest()).toStrictEqual({ id: '10', name: '午饭' });
        expect(emptyTag.name).toBe('');
        expect(emptyTag.hidden).toBe(false);
        expect(namedTag.name).toBe('娱乐');
        expect(namedTag.displayOrder).toBe(0);
    });
});
