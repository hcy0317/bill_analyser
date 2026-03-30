import { describe, expect, test } from '@jest/globals';

import {
    TransactionPicture,
    type TransactionPictureInfoBasicResponse
} from '@/models/transaction_picture_info.ts';

const SAMPLE_PICTURE: TransactionPictureInfoBasicResponse = {
    pictureId: 'pic-1',
    originalUrl: 'https://example.com/pictures/1.png'
};

describe('TransactionPicture model', () => {
    test('TransactionPicture.of maps the basic picture fields', () => {
        const picture = TransactionPicture.of(SAMPLE_PICTURE);

        expect(picture.pictureId).toBe('pic-1');
        expect(picture.originalUrl).toBe('https://example.com/pictures/1.png');
    });

    test('TransactionPicture.ofMulti maps arrays of picture responses', () => {
        const pictures = TransactionPicture.ofMulti([
            SAMPLE_PICTURE,
            { pictureId: 'pic-2', originalUrl: 'https://example.com/pictures/2.png' }
        ]);

        expect(pictures.map(item => item.pictureId)).toStrictEqual(['pic-1', 'pic-2']);
    });
});
