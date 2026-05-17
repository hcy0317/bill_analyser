import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

import { CategoryType } from '@/core/category.ts';
import {
    resolveImportPreviewCategoryId,
    resolveImportPreviewCategoryPath
} from '@/views/desktop/transactions/import/importPreview.ts';

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

    test('normalizes string category ids and ignores zero-like persisted values', () => {
        expect(resolveImportPreviewCategoryId({
            id: 4,
            category_id: ' 11 ',
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡'
        }, categoriesById)).toBe('11');

        expect(resolveImportPreviewCategoryId({
            id: 5,
            category_id: ' 0 ',
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡'
        }, categoriesById)).toBe('11');
    });

    test('returns empty when no preview category names are available', () => {
        expect(resolveImportPreviewCategoryId({
            id: 6,
            category_id: null,
            categoryId: undefined,
            preview_main_category: '',
            preview_sub_category: ''
        }, categoriesById)).toBe('');
    });

    test('skips empty category slots and supports top-level main-category fallback', () => {
        expect(resolveImportPreviewCategoryId({
            id: 7,
            preview_main_category: '餐饮',
            preview_sub_category: ''
        }, {
            ...categoriesById,
            emptySlot: undefined
        })).toBe('10');
    });

    test('skips undefined category entries before resolving a nested fallback match', () => {
        expect(resolveImportPreviewCategoryId({
            id: 8,
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡'
        }, {
            emptySlot: undefined,
            ...categoriesById
        })).toBe('11');
    });

    test('keeps scanning top-level categories until a later main-category match is found', () => {
        expect(resolveImportPreviewCategoryId({
            id: 9,
            preview_main_category: '咖啡',
            preview_sub_category: ''
        }, {
            emptySlot: undefined,
            otherTopLevel: {
                id: 'otherTopLevel',
                name: '交通',
                parentId: '0'
            },
            ...categoriesById
        })).toBe('12');
    });

    test('does not resolve same-name categories from another transaction type', () => {
        const typedCategoriesById = {
            expenseParent: {
                id: 'expenseParent',
                name: '餐饮',
                parentId: '0',
                type: CategoryType.Expense
            },
            expenseSub: {
                id: 'expenseSub',
                name: '咖啡',
                parentId: 'expenseParent',
                type: CategoryType.Expense
            },
            transferParent: {
                id: 'transferParent',
                name: '账户互转',
                parentId: '0',
                type: CategoryType.Transfer
            },
            transferSub: {
                id: 'transferSub',
                name: '咖啡',
                parentId: 'transferParent',
                type: CategoryType.Transfer
            }
        };

        expect(resolveImportPreviewCategoryId({
            id: 10,
            category_id: 'expenseSub',
            preview_type: '转账',
            preview_main_category: '账户互转',
            preview_sub_category: '咖啡'
        }, typedCategoriesById)).toBe('transferSub');

        expect(resolveImportPreviewCategoryId({
            id: 11,
            preview_type: '转账',
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡'
        }, typedCategoriesById)).toBe('');
    });

    test('resolves canonical category path from the taxonomy id', () => {
        expect(resolveImportPreviewCategoryPath('11', categoriesById)).toEqual({
            id: '11',
            mainCategory: '餐饮',
            subCategory: '咖啡',
            displayCategory: '咖啡',
            type: null
        });

        expect(resolveImportPreviewCategoryPath('missing', categoriesById)).toBeNull();
        expect(resolveImportPreviewCategoryPath('tagLikeName', categoriesById)).toBeNull();
    });
});

describe('import preview server-paged reset guards', () => {
    test('parent keeps the initial preview request pinned to the default sort contract', () => {
        const source = readSource('src/views/desktop/transactions/import/ImportDialog.vue');

        expect(source).toContain('const normalizedSortBy = normalizePreviewPageSortBy(sortOptions?.sortBy ?? previewPageSortBy.value);');
        expect(source).toContain('&& pendingRequest.sortBy === normalizedSortBy');
        expect(source).toContain('&& pendingRequest.sortDirection === normalizedSortDirection');
        expect(source).toContain('await fetchPreviewPage(normalizedPage, normalizedPageSize, {');
        expect(source).toContain("logger.error('[三阶段导入-预览分页] Check Data 加载失败:', error);");
        expect(source).toContain("snackbar.value?.showError(`导入失败: ${error}`);");
        expect(source).toContain('filters: sortOptions?.filters,');
        expect(source).toContain('appendPreviewPageFilters(searchParams, sortOptions.filters);');
        expect(source).not.toContain('void fetchPreviewPage(1, 10, {');

        const pendingIndex = source.indexOf('pendingInitialCheckDataPageRequest.value = {');
        const stepIndex = source.indexOf("currentStep.value = 'checkData';");
        const handlerIndex = source.indexOf('if (pendingRequest');
        const clearPendingIndex = source.indexOf('pendingInitialCheckDataPageRequest.value = null;', handlerIndex);
        const fetchIndex = source.indexOf('await fetchPreviewPage(normalizedPage, normalizedPageSize, {', handlerIndex);

        expect(pendingIndex).toBeGreaterThanOrEqual(0);
        expect(stepIndex).toBeGreaterThan(pendingIndex);
        expect(handlerIndex).toBeGreaterThanOrEqual(0);
        expect(clearPendingIndex).toBeGreaterThan(handlerIndex);
        expect(fetchIndex).toBeGreaterThan(clearPendingIndex);
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

    test('check-data decision sync tolerates matching payloads without annotation section', () => {
        const source = readSource('src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue');

        expect(source).toContain('previewData.matching?.annotation?.is_manually_annotated');
    });

    test('transfer decision expected state tracks source and destination accounts', () => {
        const typeSource = readSource('src/views/desktop/transactions/import/checkDataTypes.ts');
        const tabSource = readSource('src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue');

        expect(typeSource).toContain('sourceAccountId: string;');
        expect(typeSource).toContain('destinationAccountId: string;');
        expect(tabSource).toContain("sourceAccountId: item.sourceAccountId || '',");
        expect(tabSource).toContain("destinationAccountId: item.destinationAccountId || '',");
        expect(tabSource).toContain("|| baseline.sourceAccountId !== (item.sourceAccountId || '')");
        expect(tabSource).toContain("|| baseline.destinationAccountId !== (item.destinationAccountId || '')");
        expect(tabSource).toContain('sourceAccountId: item.sourceAccountId,');
        expect(tabSource).toContain('destinationAccountId: item.destinationAccountId');
    });

    test('check-data server paging no longer fetches a full preview index before filtering', () => {
        const parentSource = readSource('src/views/desktop/transactions/import/ImportDialog.vue');
        const tabSource = readSource('src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue');

        expect(parentSource).toContain(':preview-metadata="previewMetadata"');
        expect(parentSource).toContain('preserve_unpatched_selection: serverPagedPreviewMode.value');
        expect(tabSource).toContain('buildServerPreviewQueryFilters()');
        expect(tabSource).toContain("emit('requestPage', normalizedPage, normalizedPageSize, getCurrentServerPagedRequestOptions());");
        expect(tabSource).toContain('return serverPagedMode.value ? buildTrackedPreviewUpdates() : buildSelectedPreviewUpdates();');
        expect(tabSource).toContain('previewMetadata.value.counts?.selected_invalid');
        expect(tabSource).not.toContain('/index');
        expect(tabSource).not.toContain('loadServerPagedPreviewIndex');
    });
});
