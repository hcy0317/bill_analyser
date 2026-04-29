import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

import { resolveImportPreviewCategoryId } from '@/views/desktop/transactions/import/importPreview.ts';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

const categoriesById = {
    '10': {
        id: '10',
        name: '餐饮',
        parentId: '0'
    },
    '11': {
        id: '11',
        name: '咖啡',
        parentId: '10'
    },
    '12': {
        id: '12',
        name: '咖啡',
        parentId: '0'
    },
    tagLikeName: {
        id: 'tagLikeName',
        name: '咖啡',
        parentId: 'missing-parent'
    }
};

describe('import preview category resolution', () => {
    test('prefers persisted preview category id over same-name fallback matches', () => {
        expect(resolveImportPreviewCategoryId({
            id: 1,
            category_id: 11,
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡'
        }, categoriesById)).toBe('11');
    });

    test('falls back by name only when the category hierarchy exists', () => {
        expect(resolveImportPreviewCategoryId({
            id: 2,
            category_id: 999,
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡'
        }, categoriesById)).toBe('11');

        expect(resolveImportPreviewCategoryId({
            id: 3,
            preview_main_category: '不存在',
            preview_sub_category: '咖啡'
        }, categoriesById)).toBe('');
    });
});

describe('import preview server-paged reset guards', () => {
    test('parent keeps the initial preview request pinned to the default sort contract', () => {
        const source = readSource('src/views/desktop/transactions/import/ImportDialog.vue');

        expect(source).toContain('const normalizedSortBy = normalizePreviewPageSortBy(sortOptions?.sortBy ?? previewPageSortBy.value);');
        expect(source).toContain('&& pendingRequest.sortBy === normalizedSortBy');
        expect(source).toContain('&& pendingRequest.sortDirection === normalizedSortDirection');

        const pendingIndex = source.indexOf('pendingInitialCheckDataPageRequest.value = {');
        const stepIndex = source.indexOf("currentStep.value = 'checkData';");
        const fetchIndex = source.indexOf('void fetchPreviewPage(1, 10, {');

        expect(pendingIndex).toBeGreaterThanOrEqual(0);
        expect(stepIndex).toBeGreaterThan(pendingIndex);
        expect(fetchIndex).toBeGreaterThan(stepIndex);
    });

    test('check-data reset clears stale table sort state before a new server-paged session starts', () => {
        const source = readSource('src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue');
        const resetIndex = source.indexOf('function reset(): void {');
        const nextFunctionIndex = source.indexOf('function setCountPerPage', resetIndex);
        const resetBlock = source.slice(resetIndex, nextFunctionIndex);

        expect(resetIndex).toBeGreaterThanOrEqual(0);
        expect(nextFunctionIndex).toBeGreaterThan(resetIndex);
        expect(resetBlock).toContain('tableSortBy.value = [];');
        expect(resetBlock).toContain("currentSortKey.value = '';");
        expect(resetBlock).toContain("currentSortDirection.value = 'asc';");
    });
});
