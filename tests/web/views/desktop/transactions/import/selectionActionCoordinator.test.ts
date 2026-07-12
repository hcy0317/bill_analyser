import { describe, expect, test } from '@jest/globals';
import { buildSelectionPatch } from '@/views/desktop/transactions/import/selectionActionCoordinator.ts';

describe('selection action coordinator', () => {
    test('flushes a newly selected current-page row before building the action hash', () => {
        expect(buildSelectionPatch([{ id: 11, baselineSelected: false, selected: true }]))
            .toEqual({ selectedIds: [11], deselectedIds: [] });
    });

    test('flushes a current-page deselection so a cross-page selection excludes it', () => {
        expect(buildSelectionPatch([
            { id: 11, baselineSelected: true, selected: false },
            { id: 22, baselineSelected: true, selected: true }
        ])).toEqual({ selectedIds: [], deselectedIds: [11] });
    });
});
