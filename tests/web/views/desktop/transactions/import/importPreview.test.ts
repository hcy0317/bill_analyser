import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from '@jest/globals';

import { CategoryType } from '@/core/category.ts';
import { TransactionType } from '@/core/transaction.ts';
import type { ImportTransaction } from '@/models/imported_transaction.ts';
import {
    resolveImportPreviewCategoryId,
    resolveImportPreviewDefaultTransferCategoryId,
    resolveImportPreviewCategoryPath
} from '@/views/desktop/transactions/import/importPreview.ts';
import { buildImportPreviewServerQueryFilters } from '@/views/desktop/transactions/import/importPreviewIndex.ts';
import { buildImportPreviewUpdateFromTransaction } from '@/views/desktop/transactions/import/importPreviewUpdates.ts';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8').replace(/\r\n/g, '\n');
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

const baseServerFilters = {
    minDatetime: null,
    maxDatetime: null,
    transactionType: null,
    category: null,
    account: null,
    tag: null,
    signal: null,
    annotation: null,
    description: null
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

    test('does not fall back by name when canonical category identity is invalid', () => {
        expect(resolveImportPreviewCategoryId({
            id: 2,
            category_id: 999,
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡'
        }, categoriesById)).toBe('');

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
        }, categoriesById)).toBe('');
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

    test('does not resolve top-level categories from display names', () => {
        expect(resolveImportPreviewCategoryId({
            id: 7,
            preview_main_category: '餐饮',
            preview_sub_category: ''
        }, {
            ...categoriesById,
            emptySlot: undefined
        })).toBe('');
    });

    test('does not resolve nested categories from display names', () => {
        expect(resolveImportPreviewCategoryId({
            id: 8,
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡'
        }, {
            emptySlot: undefined,
            ...categoriesById
        })).toBe('');
    });

    test('does not scan same-name top-level categories without canonical identity', () => {
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
        })).toBe('');
    });

    test('trusts canonical category ids even when local type metadata differs', () => {
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
        }, typedCategoriesById)).toBe('expenseSub');

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
        expect(resolveImportPreviewCategoryPath('hiddenSub', {
            hiddenParent: {
                id: 'hiddenParent',
                name: '隐藏父类',
                parentId: '0',
                hidden: true,
                type: CategoryType.Expense
            },
            hiddenSub: {
                id: 'hiddenSub',
                name: '隐藏子类',
                parentId: 'hiddenParent',
                type: CategoryType.Expense
            },
            visibleParent: {
                id: 'visibleParent',
                name: '可见父类',
                parentId: '0',
                type: CategoryType.Expense
            },
            hiddenLeaf: {
                id: 'hiddenLeaf',
                name: '隐藏叶子类',
                parentId: 'visibleParent',
                hidden: true,
                type: CategoryType.Expense
            }
        })).toBeNull();
        expect(resolveImportPreviewCategoryPath('hiddenLeaf', {
            visibleParent: {
                id: 'visibleParent',
                name: '可见父类',
                parentId: '0',
                type: CategoryType.Expense
            },
            hiddenLeaf: {
                id: 'hiddenLeaf',
                name: '隐藏叶子类',
                parentId: 'visibleParent',
                hidden: true,
                type: CategoryType.Expense
            }
        })).toBeNull();
    });

    test('resolves the default transfer category from profile setting before visible fallback', () => {
        const transferCategories = [
            {
                id: 'transferParent',
                name: '账户互转',
                parentId: '0',
                type: CategoryType.Transfer,
                subCategories: [
                    {
                        id: 'transferSub',
                        name: '默认互转',
                        parentId: 'transferParent',
                        type: CategoryType.Transfer
                    }
                ]
            }
        ];
        const transferCategoryMap = {
            transferParent: {
                id: 'transferParent',
                name: '账户互转',
                parentId: '0',
                type: CategoryType.Transfer
            },
            transferSub: {
                id: 'transferSub',
                name: '默认互转',
                parentId: 'transferParent',
                type: CategoryType.Transfer
            },
            userTransferSub: {
                id: 'userTransferSub',
                name: '用户互转',
                parentId: 'transferParent',
                type: CategoryType.Transfer
            }
        };

        expect(resolveImportPreviewDefaultTransferCategoryId(
            transferCategoryMap,
            transferCategories,
            'userTransferSub'
        )).toBe('userTransferSub');
        expect(resolveImportPreviewDefaultTransferCategoryId(
            transferCategoryMap,
            transferCategories,
            'missing'
        )).toBe('transferSub');
        expect(resolveImportPreviewDefaultTransferCategoryId(
            transferCategoryMap,
            [
                {
                    id: 'hiddenParent',
                    name: '隐藏互转',
                    parentId: '0',
                    type: CategoryType.Transfer,
                    hidden: true,
                    subCategories: [
                        {
                            id: 'hiddenSub',
                            name: '隐藏子类',
                            parentId: 'hiddenParent',
                            type: CategoryType.Transfer
                        }
                    ]
                }
            ],
            'missing'
        )).toBe('');
        expect(resolveImportPreviewDefaultTransferCategoryId(
            transferCategoryMap,
            undefined,
            'missing'
        )).toBe('');
    });

    test('inherits a parent category type and skips fallback children without stable ids', () => {
        expect(resolveImportPreviewCategoryPath('childWithoutType', {
            parent: {
                id: 'parent',
                name: '父分类',
                parentId: '0',
                type: CategoryType.Transfer
            },
            childWithoutType: {
                id: 'childWithoutType',
                name: '子分类',
                parentId: 'parent'
            }
        })?.type).toBe(CategoryType.Transfer);

        expect(resolveImportPreviewDefaultTransferCategoryId({}, [{
            name: '账户互转',
            subCategories: [{ name: '无身份子类' }]
        }], null)).toBe('');
        expect(resolveImportPreviewDefaultTransferCategoryId({}, [{
            name: '无子类账户互转'
        }], null)).toBe('');
    });
});

describe('import preview identity-safe update/query payloads', () => {
    test('server query maps category labels to stable ids and preserves sentinels', () => {
        expect(buildImportPreviewServerQueryFilters({
            ...baseServerFilters,
            category: '餐饮/咖啡',
            account: '招商卡',
            tag: undefined
        }, {
            categoryValueByLabel: {'餐饮/咖啡': '11'},
            accountValueByLabel: {'招商卡': '21'}
        })).toEqual({
            category: '11',
            account: '21',
            tag: '__invalid__'
        });

        expect(buildImportPreviewServerQueryFilters({
            ...baseServerFilters,
            category: '',
            account: null,
            tag: ''
        })).toEqual({
            category: '__none__',
            tag: '__none__'
        });
    });

    test('preview update payload drops zero and non-map-backed account ids', () => {
        const transaction = {
            _previewId: 99,
            type: TransactionType.Expense,
            sourceAmountCents: 1234,
            destinationAmountCents: 0,
            sourceAccountId: '0',
            destinationAccountId: '404',
            recurringTemplateId: '0',
            recurringTemplateName: '',
            recurringCandidateCount: 0,
            recurringMatchScore: 0,
            recurringMatchReasons: '',
            recurringMatchedDate: '',
            selected: true
        } as unknown as ImportTransaction;

        const update = buildImportPreviewUpdateFromTransaction(transaction, {
            categoryPath: {
                id: '11',
                mainCategory: '餐饮',
                subCategory: '咖啡',
                displayCategory: '咖啡',
                type: TransactionType.Expense
            },
            validAccountIds: new Set(['21'])
        });

        expect(update.category_id).toBe(11);
        expect(update.preview_source_account_id).toBeNull();
        expect(update.preview_destination_account_id).toBeNull();
        expect(update.preview_recurring_id).toBeNull();
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

    test('check-data manual edits clear only actionable suggestion families', () => {
        const typeSource = readSource('src/views/desktop/transactions/import/checkDataTypes.ts');
        const tabSource = readSource('src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue');
        const updateSource = readSource('src/views/desktop/transactions/import/importPreviewUpdates.ts');

        expect(typeSource).toContain('_shouldClearTransferDecision?: boolean;');
        expect(typeSource).toContain('_shouldClearLearningDecision?: boolean;');
        expect(typeSource).toContain('_shouldClearLlmDecision?: boolean;');
        expect(tabSource).toContain('syncActionableSuggestionDraftState(item);');
        expect(tabSource).toContain("const shouldClearTransferDecision = baseline.reviewStatus === 'pending';");
        expect(tabSource).toContain('if (previewState._shouldClearLearningDecision) {');
        expect(tabSource).toContain('if (!item.hasPendingLearningRecommendation()) {');
        expect(tabSource).toContain('if (previewState._shouldClearLlmDecision) {');
        expect(tabSource).toContain('buildImportPreviewUpdateFromTransaction(transaction, {');
        expect(tabSource).toContain('includeSuggestionDecisionClears: true');
        expect(updateSource).toContain("clearLearningDecision ? 'learning' : ''");
        expect(updateSource).toContain("clearLlmDecision ? 'llm' : ''");
        expect(updateSource).toContain('if (clearLearningDecision) {');
        expect(updateSource).toContain("update['clear_learning_decision'] = true;");
        expect(updateSource).toContain('if (clearLlmDecision) {');
        expect(updateSource).toContain("update['clear_llm_decision'] = true;");
        expect(updateSource).toContain('if (clearActionableSuggestions.length > 0) {');
        expect(tabSource.match(/syncTransferDecisionDraftState\(/g)).toHaveLength(2);

        const rehydrateStart = tabSource.indexOf('function rehydrateCurrentPageDrafts');
        const rehydrateEnd = tabSource.indexOf('function getTrackedTransactionsForSelection', rehydrateStart);
        const rehydrateSource = tabSource.slice(rehydrateStart, rehydrateEnd);
        expect(rehydrateSource).not.toContain('syncTransferDecisionDraftState(transaction);');
        expect(rehydrateSource).toContain('transaction.matching = authoritativeMatching;');
    });

    test('check-data keeps investment recognition out of actionable signal UI', () => {
        const matchingSource = readSource('src/views/desktop/transactions/import/checkDataMatching.ts');
        const signalCellSource = readSource('src/views/desktop/transactions/import/tabs/ImportPreviewSignalCell.vue');

        expect(matchingSource).toContain('investment: null,');
        expect(signalCellSource).not.toContain('investment-signal-group');
        expect(signalCellSource).not.toContain('getInvestmentIcon');
        expect(signalCellSource).not.toContain('mdiChartLine');
    });

    test('llm recommendation sync does not reset transfer suggestions blindly', () => {
        const tabSource = readSource('src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue');
        const functionIndex = tabSource.indexOf('function syncTransactionFromLLMPreviewPayload');
        const nextFunctionIndex = tabSource.indexOf('function applyLLMSignalMemoryToTransactions', functionIndex);
        const functionSource = tabSource.slice(functionIndex, nextFunctionIndex);

        expect(functionIndex).toBeGreaterThanOrEqual(0);
        expect(nextFunctionIndex).toBeGreaterThan(functionIndex);
        expect(functionSource).not.toContain('resetTransferSuggestionDecisionState');
        expect(functionSource).toContain('syncTransferDecisionBaseline(item);');
        expect(functionSource).toContain('syncLearningDecisionBaseline(item);');
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

    test('check-data signal column uses the transfer signal adapter', () => {
        const signalSource = readSource('src/views/desktop/transactions/import/check-data-tab/useImportCheckDataSignals.ts');
        const signalModelIndex = signalSource.indexOf('const viewModel = buildImportPreviewSignalViewModelFromSnapshot({');
        const signalModelSource = signalSource.slice(signalModelIndex, signalSource.indexOf('}, item.previewState, {', signalModelIndex));

        expect(signalSource).toContain('getImportPreviewTransferSignalStatus,');
        expect(signalSource).toContain('getImportPreviewTransferSignalTitle');
        expect(signalSource).toContain('buildImportPreviewSignalViewModelFromSnapshot,');
        expect(signalModelSource).toContain('transferStatus: getImportPreviewTransferSignalStatus(item),');
        expect(signalModelSource).toContain('transferTitle: getImportPreviewTransferSignalTitle(item),');
        expect(signalSource).toContain('}, item.previewState, {');
    });

    test('check-data server paging no longer fetches a full preview index before filtering', () => {
        const parentSource = readSource('src/views/desktop/transactions/import/ImportDialog.vue');
        const tabSource = readSource('src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue');

        expect(parentSource).toContain(':preview-metadata="previewMetadata"');
        expect(parentSource).toContain('preserve_unpatched_selection: serverPagedPreviewMode.value');
        expect(tabSource).toContain('buildServerPreviewQueryFilters()');
        expect(tabSource).toContain("emit('requestPage', normalizedPage, normalizedPageSize, requestOptions);");
        expect(tabSource).toContain('return serverPagedMode.value ? buildTrackedPreviewUpdates() : buildSelectedPreviewUpdates();');
        expect(tabSource).toContain('previewMetadata.value.counts?.selected_invalid');
        expect(tabSource).not.toContain('/index');
        expect(tabSource).not.toContain('loadServerPagedPreviewIndex');
    });

    test('check-data server paging baselines use dynamic annotation issue state', () => {
        const annotationSource = readSource('src/views/desktop/transactions/import/check-data-tab/useImportCheckDataAnnotations.ts');
        const functionIndex = annotationSource.indexOf('function hasBaselineAnnotationIssue');
        const nextFunctionIndex = annotationSource.indexOf('function getAnnotationSummary', functionIndex);
        const functionSource = annotationSource.slice(functionIndex, nextFunctionIndex);

        expect(functionIndex).toBeGreaterThanOrEqual(0);
        expect(nextFunctionIndex).toBeGreaterThan(functionIndex);
        expect(functionSource).toContain('return hasCurrentAnnotationIssue(item);');
        expect(functionSource).not.toContain('hasRawPersistedMatchingAnnotationIssue');
    });

    test('check-data annotation resolution reads status-only missing category payloads', () => {
        const annotationSource = readSource('src/views/desktop/transactions/import/check-data-tab/useImportCheckDataAnnotations.ts');
        const functionIndex = annotationSource.indexOf('function getAnnotationType');
        const nextFunctionIndex = annotationSource.indexOf('function hasCurrentPersistedMatchingAnnotationIssue', functionIndex);
        const functionSource = annotationSource.slice(functionIndex, nextFunctionIndex);

        expect(functionIndex).toBeGreaterThanOrEqual(0);
        expect(nextFunctionIndex).toBeGreaterThan(functionIndex);
        expect(functionSource).toContain('return getAnnotationText(annotation).trim().toLowerCase();');
        expect(functionSource).not.toContain("annotation['type'] || ''");
    });
});
