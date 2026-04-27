import { describe, expect, test } from '@jest/globals';

import { resolveImportPreviewCategoryId } from '@/views/desktop/transactions/import/importPreview.ts';

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
