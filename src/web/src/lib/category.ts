import { itemAndIndex, reversed, entries, keys, values } from '@/core/base.ts';
import { type LocalizedPresetCategory, CategoryType } from '@/core/category.ts';
import { TransactionType } from '@/core/transaction.ts';
import {
    type TransactionCategoriesWithVisibleCount,
    type TransactionCategoryCreateRequest,
    type TransactionCategoryCreateWithSubCategories,
    type TransactionCategoryInfoResponse,
    TransactionCategory
} from '@/models/transaction_category.ts';
import logger from '@/lib/logger.ts';  // 新增日志

export function transactionTypeToCategoryType(transactionType: TransactionType): CategoryType | null {
    if (transactionType === TransactionType.Income) {
        return CategoryType.Income;
    } else if (transactionType === TransactionType.Expense) {
        return CategoryType.Expense;
    } else if (transactionType === TransactionType.Transfer) {
        return CategoryType.Transfer;
    } else if (transactionType === TransactionType.Investment) {
        return CategoryType.Investment;
    } else {
        return null;
    }
}

export function categoryTypeToTransactionType(categoryType: CategoryType): TransactionType | null {
    if (categoryType === CategoryType.Income) {
        return TransactionType.Income;
    } else if (categoryType === CategoryType.Expense) {
        return TransactionType.Expense;
    } else if (categoryType === CategoryType.Transfer) {
        return TransactionType.Transfer;
    } else if (categoryType === CategoryType.Investment) {
        return TransactionType.Investment;
    } else {
        return null;
    }
}

export function localizedPresetCategoryToTransactionCategoryCreateWithSubCategorys(presetCategory: LocalizedPresetCategory): TransactionCategoryCreateWithSubCategories {
    const subCategories: TransactionCategoryCreateRequest[] = [];

    for (const subPresetCategory of presetCategory.subCategories) {
        const subCategory: TransactionCategoryCreateRequest = {
            name: subPresetCategory.name,
            type: subPresetCategory.type,
            parentId: '0',
            icon: subPresetCategory.icon,
            color: subPresetCategory.color,
            comment: '',
            displayOrder: 0,
            keywords: subPresetCategory.keywords || '',
            clientSessionId: ''
        };

        subCategories.push(subCategory);
    }

    const categoryWithSubCategories: TransactionCategoryCreateWithSubCategories = {
        name: presetCategory.name,
        type: presetCategory.type,
        icon: presetCategory.icon,
        color: presetCategory.color,
        subCategories: subCategories
    };

    return categoryWithSubCategories;
}

export function localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories(presetCategories: LocalizedPresetCategory[]): TransactionCategoryCreateWithSubCategories[] {
    const categories: TransactionCategoryCreateWithSubCategories[] = [];

    for (const presetCategory of presetCategories) {
        const categoryWithSubCategories = localizedPresetCategoryToTransactionCategoryCreateWithSubCategorys(presetCategory);
        categories.push(categoryWithSubCategories);
    }

    return categories;
}

export function getSecondaryTransactionMapByName(allCategories?: TransactionCategory[]): Record<string, TransactionCategory> {
    const ret: Record<string, TransactionCategory> = {};

    if (!allCategories) {
        return ret;
    }

    for (const category of allCategories) {
        if (category.subCategories) {
            for (const subCategory of category.subCategories) {
                ret[subCategory.name] = subCategory;
            }
        }
    }

    return ret;
}

interface TransactionCategoryPath {
    primaryName: string;
    secondaryName: string;
}

function findTransactionCategoryPath(categoryId: string, allCategories?: TransactionCategory[]): TransactionCategoryPath | null {
    if (!allCategories) {
        return null;
    }

    for (const category of allCategories) {
        if (category.id === categoryId) {
            return {
                primaryName: category.name,
                secondaryName: ''
            };
        }

        const subCategoryList = category.subCategories;

        if (!subCategoryList) {
            continue;
        }

        for (const subCategory of subCategoryList) {
            if (subCategory.id === categoryId) {
                return {
                    primaryName: category.name,
                    secondaryName: subCategory.name
                };
            }
        }
    }

    return null;
}

function splitTransactionCategoryDisplayName(transactionCategory?: TransactionCategoryInfoResponse): TransactionCategoryPath | null {
    if (!transactionCategory?.name) {
        return null;
    }

    const normalizedName = transactionCategory.name.trim();
    const separators = ['-', '＞', '>', '/'];

    for (const separator of separators) {
        const parts = normalizedName.split(separator).map(part => part.trim()).filter(Boolean);

        if (parts.length >= 2) {
            return {
                primaryName: parts[0]!,
                secondaryName: parts.slice(1).join(separator)
            };
        }
    }

    return null;
}

export function getTransactionPrimaryCategoryName(categoryId: string | null | undefined, allCategories?: TransactionCategory[], transactionCategory?: TransactionCategoryInfoResponse): string {
    // 如果 categoryId 为空或 '0'，直接返回空字符串，不记录警告
    if (!categoryId || categoryId === '0') {
        return '';
    }

    const categoryPath = findTransactionCategoryPath(categoryId, allCategories);

    if (categoryPath) {
        return categoryPath.primaryName;
    }

    if (transactionCategory && transactionCategory.id === categoryId) {
        const displayNamePath = splitTransactionCategoryDisplayName(transactionCategory);

        if (displayNamePath) {
            return displayNamePath.primaryName;
        }

        if (!transactionCategory.parentId || transactionCategory.parentId === '0') {
            return transactionCategory.name || '';
        }

        return '';
    }

    if (!allCategories && !transactionCategory) {
        return '';
    }

    // 只有在 categoryId 有值但找不到匹配时才记录警告
    logger.warn(`[分类显示] 未在allCategories中找到匹配的分类: categoryId=${categoryId}`);
    return '';
}

export function getTransactionSecondaryCategoryName(categoryId: string | null | undefined, allCategories?: TransactionCategory[], transactionCategory?: TransactionCategoryInfoResponse): string {
    // 如果 categoryId 为空或 '0'，直接返回空字符串，不记录警告
    if (!categoryId || categoryId === '0') {
        return '';
    }

    const categoryPath = findTransactionCategoryPath(categoryId, allCategories);

    if (categoryPath) {
        return categoryPath.secondaryName;
    }

    if (transactionCategory && transactionCategory.id === categoryId) {
        const displayNamePath = splitTransactionCategoryDisplayName(transactionCategory);

        if (displayNamePath) {
            return displayNamePath.secondaryName;
        }

        if (transactionCategory.parentId && transactionCategory.parentId !== '0') {
            return transactionCategory.name || '';
        }

        return '';
    }

    if (!allCategories && !transactionCategory) {
        return '';
    }

    // 只有在 categoryId 有值但找不到匹配时才记录警告
    logger.warn(`[分类显示] 未在allCategories中找到匹配的子分类: categoryId=${categoryId}`);
    return '';
}

export function allTransactionCategoriesWithVisibleCount(allTransactionCategories: Record<number, TransactionCategory[]>, allowCategoryTypes?: Record<number | string, boolean>): Record<number, TransactionCategoriesWithVisibleCount> {
    const ret: Record<string, TransactionCategoriesWithVisibleCount> = {};

    // 检查是否有允许的分类类型（支持数字和字符串键）
    const hasAllowCategoryTypes = allowCategoryTypes
        && (allowCategoryTypes[CategoryType.Income] || allowCategoryTypes[String(CategoryType.Income)]
            || allowCategoryTypes[CategoryType.Expense] || allowCategoryTypes[String(CategoryType.Expense)]
            || allowCategoryTypes[CategoryType.Transfer] || allowCategoryTypes[String(CategoryType.Transfer)]
            || allowCategoryTypes[CategoryType.Investment] || allowCategoryTypes[String(CategoryType.Investment)]);

    // 包含所有分类类型，包括投资
    const allCategoryTypes = [ CategoryType.Income, CategoryType.Expense, CategoryType.Transfer, CategoryType.Investment ];

    for (const categoryType of allCategoryTypes) {
        if (!allTransactionCategories[categoryType]) {
            continue;
        }

        // 同时检查数字和字符串键
        if (hasAllowCategoryTypes && !allowCategoryTypes[categoryType] && !allowCategoryTypes[String(categoryType)]) {
            continue;
        }

        const allCategories: TransactionCategory[] = allTransactionCategories[categoryType];
        const allSubCategories: Record<string, TransactionCategory[]> = {};
        const allVisibleSubCategoryCounts: Record<string, number> = {};
        const allFirstVisibleSubCategoryIndexes: Record<string, number> = {};
        let allVisibleCategoryCount = 0;
        let firstVisibleCategoryIndex = -1;

        for (const [category, cagtegoryIndex] of itemAndIndex(allCategories)) {
            if (!category.hidden) {
                allVisibleCategoryCount++;

                if (firstVisibleCategoryIndex === -1) {
                    firstVisibleCategoryIndex = cagtegoryIndex;
                }
            }

            if (category.subCategories) {
                let visibleSubCategoryCount = 0;
                let firstVisibleSubCategoryIndex = -1;

                for (const [subCategory, subCategoryIndex] of itemAndIndex(category.subCategories)) {
                    if (!subCategory.hidden) {
                        visibleSubCategoryCount++;

                        if (firstVisibleSubCategoryIndex === -1) {
                            firstVisibleSubCategoryIndex = subCategoryIndex;
                        }
                    }
                }

                if (category.subCategories.length > 0) {
                    allSubCategories[category.id] = category.subCategories;
                    allVisibleSubCategoryCounts[category.id] = visibleSubCategoryCount;
                    allFirstVisibleSubCategoryIndexes[category.id] = firstVisibleSubCategoryIndex;
                }
            }
        }

        ret[categoryType] = {
            type: categoryType,
            allCategories: allCategories,
            allVisibleCategoryCount: allVisibleCategoryCount,
            firstVisibleCategoryIndex: firstVisibleCategoryIndex,
            allSubCategories: allSubCategories,
            allVisibleSubCategoryCounts: allVisibleSubCategoryCounts,
            allFirstVisibleSubCategoryIndexes: allFirstVisibleSubCategoryIndexes
        };
    }

    return ret;
}

export function allVisiblePrimaryTransactionCategoriesByType(allTransactionCategories: Record<number, TransactionCategory[]>, categoryType: number): TransactionCategory[] {
    const allCategories = allTransactionCategories[categoryType];
    const visibleCategories: TransactionCategory[] = [];

    if (!allCategories) {
        return visibleCategories;
    }

    for (const category of allCategories) {
        if (category.hidden) {
            continue;
        }

        visibleCategories.push(category);
    }

    return visibleCategories;
}

export function getFinalCategoryIdsByFilteredCategoryIds(allTransactionCategoriesMap: Record<number, TransactionCategory>, filteredCategoryIds: Record<string, boolean>): string {
    let finalCategoryIds = '';

    if (!allTransactionCategoriesMap) {
        return finalCategoryIds;
    }

    for (const category of values(allTransactionCategoriesMap)) {
        if (filteredCategoryIds && !isCategoryOrSubCategoriesAllChecked(category, filteredCategoryIds)) {
            continue;
        }

        if (finalCategoryIds.length > 0) {
            finalCategoryIds += ',';
        }

        finalCategoryIds += category.id;
    }

    return finalCategoryIds;
}

export function isSubCategoryIdAvailable(categories: TransactionCategory[], categoryId: string): boolean {
    if (!categories || !categories.length) {
        return false;
    }

    for (const primaryCategory of categories) {
        if (primaryCategory.hidden) {
            continue;
        }

        const subCategoryList = primaryCategory.subCategories;

        if (!subCategoryList) {
            continue;
        }

        for (const secondaryCategory of subCategoryList) {
            if (secondaryCategory.hidden) {
                continue;
            }

            if (secondaryCategory.id === categoryId) {
                return true;
            }
        }
    }

    return false;
}

export function getFirstAvailableCategoryId(categories?: TransactionCategory[]): string {
    if (!categories || !categories.length) {
        return '';
    }

    for (const primaryCategory of categories) {
        if (primaryCategory.hidden) {
            continue;
        }

        const subCategoryList = primaryCategory.subCategories;

        if (!subCategoryList) {
            continue;
        }

        for (const secondaryCategory of subCategoryList) {
            if (secondaryCategory.hidden) {
                continue;
            }

            return secondaryCategory.id;
        }
    }

    return '';
}

export function getFirstAvailableSubCategoryId(categories: TransactionCategory[], categoryId: string): string {
    if (!categories || !categories.length) {
        return '';
    }

    for (const primaryCategory of categories) {
        if (primaryCategory.hidden || primaryCategory.id !== categoryId) {
            continue;
        }

        const subCategoryList = primaryCategory.subCategories;

        if (!subCategoryList) {
            return '';
        }

        for (const secondaryCategory of subCategoryList) {
            if (secondaryCategory.hidden) {
                continue;
            }

            return secondaryCategory.id;
        }

        return '';
    }

    return '';
}

export function isNoAvailableCategory(categories: TransactionCategory[], showHidden: boolean): boolean {
    for (const category of categories) {
        if (showHidden || !category.hidden) {
            return false;
        }
    }

    return true;
}

export function getAvailableCategoryCount(categories: TransactionCategory[], showHidden: boolean): number {
    let count = 0;

    for (const category of categories) {
        if (showHidden || !category.hidden) {
            count++;
        }
    }

    return count;
}

export function getFirstShowingId(categories: TransactionCategory[], showHidden: boolean): string | null {
    for (const category of categories) {
        if (showHidden || !category.hidden) {
            return category.id;
        }
    }

    return null;
}

export function getLastShowingId(categories: TransactionCategory[], showHidden: boolean): string | null {
    for (const category of reversed(categories)) {
        if (showHidden || !category.hidden) {
            return category.id;
        }
    }

    return null;
}

export function containsAnyAvailableCategory(allTransactionCategories: Record<number, TransactionCategoriesWithVisibleCount>, showHidden: boolean): boolean {
    for (const categoryType of values(allTransactionCategories)) {
        if (showHidden) {
            if (categoryType.allCategories && categoryType.allCategories.length > 0) {
                return true;
            }
        } else {
            if (categoryType.allVisibleCategoryCount > 0) {
                return true;
            }
        }
    }

    return false;
}

export function containsAvailableCategory(allTransactionCategories: Record<number, TransactionCategoriesWithVisibleCount>, showHidden: boolean): Record<number, boolean> {
    const result: Record<number, boolean> = {};

    for (const [type, categoryType] of entries(allTransactionCategories)) {
        if (showHidden) {
            result[parseInt(type)] = categoryType.allCategories && categoryType.allCategories.length > 0;
        } else {
            result[parseInt(type)] = categoryType.allVisibleCategoryCount > 0;
        }
    }

    return result;
}

export function selectAllSubCategories(filterCategoryIds: Record<string, boolean>, value: boolean, category?: TransactionCategory): void {
    if (!category || !category.subCategories || !category.subCategories.length) {
        return;
    }

    for (const subCategory of category.subCategories) {
        filterCategoryIds[subCategory.id] = value;
    }
}

export function selectAllVisible(filterCategoryIds: Record<string, boolean>, allTransactionCategoriesMap: Record<string, TransactionCategory>): void {
    for (const categoryId of keys(filterCategoryIds)) {
        const category = allTransactionCategoriesMap[categoryId];

        if (category) {
            if (category.hidden) {
                continue;
            }

            if (category.parentId && allTransactionCategoriesMap[category.parentId] && allTransactionCategoriesMap[category.parentId]!.hidden) {
                continue;
            }

            filterCategoryIds[category.id] = false;
        }
    }
}

export function selectAll(filterCategoryIds: Record<string, boolean>, allTransactionCategoriesMap: Record<string, TransactionCategory>): void {
    for (const categoryId of keys(filterCategoryIds)) {
        const category = allTransactionCategoriesMap[categoryId];

        if (category) {
            filterCategoryIds[category.id] = false;
        }
    }
}

export function selectNone(filterCategoryIds: Record<string, boolean>, allTransactionCategoriesMap: Record<string, TransactionCategory>): void {
    for (const categoryId of keys(filterCategoryIds)) {
        const category = allTransactionCategoriesMap[categoryId];

        if (category) {
            filterCategoryIds[category.id] = true;
        }
    }
}

export function selectInvert(filterCategoryIds: Record<string, boolean>, allTransactionCategoriesMap: Record<string, TransactionCategory>): void {
    for (const categoryId of keys(filterCategoryIds)) {
        const category = allTransactionCategoriesMap[categoryId];

        if (category) {
            filterCategoryIds[category.id] = !filterCategoryIds[category.id];
        }
    }
}

export function isCategoryOrSubCategoriesAllChecked(category: TransactionCategory, filterCategoryIds: Record<string, boolean>): boolean {
    if (!category.subCategories || category.subCategories.length < 1) {
        return !filterCategoryIds[category.id];
    }

    for (const subCategory of category.subCategories) {
        if (filterCategoryIds[subCategory.id]) {
            return false;
        }
    }

    return true;
}

export function isSubCategoriesAllChecked(category: TransactionCategory, filterCategoryIds: Record<string, boolean>): boolean {
    if (!category.subCategories || category.subCategories.length < 1) {
        return false;
    }

    for (const subCategory of category.subCategories) {
        if (filterCategoryIds[subCategory.id]) {
            return false;
        }
    }

    return true;
}

export function isSubCategoriesHasButNotAllChecked(category: TransactionCategory, filterCategoryIds: Record<string, boolean>): boolean {
    let checkedCount = 0;

    if (!category.subCategories || category.subCategories.length < 1) {
        return false;
    }

    for (const subCategory of category.subCategories) {
        if (!filterCategoryIds[subCategory.id]) {
            checkedCount++;
        }
    }

    return checkedCount > 0 && checkedCount < category.subCategories.length;
}
