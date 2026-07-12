import { describe, expect, test } from '@jest/globals';

import {
    buildImportPreviewActionScope,
    hashImportPreviewActionFilters
} from '@/views/desktop/transactions/import/actionScope.ts';

describe('import preview action scope', () => {
    test('builds selected scope from the server selection snapshot', () => {
        expect(buildImportPreviewActionScope({
            selectedCount: 3,
            selectionHash: 'fnv1a32:selection',
            filters: { signal: 'learning' }
        })).toEqual({ kind: 'selected', selection_hash: 'fnv1a32:selection' });
    });

    test('builds a canonical all-matching scope from the actual filter object', () => {
        const scope = buildImportPreviewActionScope({
            selectedCount: 0,
            selectionHash: 'ignored',
            filters: { minDatetime: '2026-07-01 00:00:00', signal: 'learning', description: 'coffee' }
        });

        expect(scope.kind).toBe('all_matching');
        if (scope.kind !== 'all_matching') throw new Error('unexpected selected scope');
        expect(scope.filters).toEqual({
            min_datetime: '2026-07-01 00:00:00',
            max_datetime: null,
            transaction_type: null,
            category: null,
            account: null,
            tag: null,
            signal: 'learning',
            annotation: null,
            description: 'coffee',
            selected_only: false
        });
        expect(scope.filter_hash).toBe(hashImportPreviewActionFilters(scope.filters));
        expect(scope.filter_hash).toMatch(/^fnv1a32:[0-9a-f]{8}$/);
    });
});
