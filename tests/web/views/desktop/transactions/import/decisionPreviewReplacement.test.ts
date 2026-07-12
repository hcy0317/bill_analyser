import { describe, expect, jest, test } from '@jest/globals';

import {
    applyServerPagedReclassification,
    resolveDecisionPreviewReplacement,
} from '@/views/desktop/transactions/import/decisionPreviewReplacement';

describe('decision preview replacement', () => {
    test('reject replaces one removed preview with two upserted rows', () => {
        const replacement = resolveDecisionPreviewReplacement({
            removedPreviewIds: [41],
            upsertedPreviewItems: [{ id: 51 }, { id: 52 }],
        });
        expect(replacement?.removedPreviewIds).toEqual([41]);
        expect(replacement?.upsertedPreviewItems.map(item => item.id)).toEqual([51, 52]);
    });

    test('accept response does not trigger identity replacement', () => {
        expect(resolveDecisionPreviewReplacement({
            removedPreviewIds: [],
            upsertedPreviewItems: [],
            previewItem: { id: 41 },
        })).toBeNull();
    });

    test('server paged parent refreshes page index and selection metadata together', () => {
        const refresh = jest.fn();
        expect(applyServerPagedReclassification(true, refresh)).toBe(true);
        expect(refresh).toHaveBeenCalledTimes(1);
        expect(applyServerPagedReclassification(false, refresh)).toBe(false);
        expect(refresh).toHaveBeenCalledTimes(1);
    });
});
