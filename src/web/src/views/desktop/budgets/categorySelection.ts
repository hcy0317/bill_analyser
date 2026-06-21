import type { TransactionCategory } from '@/models/transaction_category.ts';
import { DateRange } from '@/core/datetime.ts';
import { getUnixTimeFromLocalDatetime } from '@/lib/datetime.ts';

export interface ResolvedBudgetCategorySelection {
    categoryId: string;
    primaryCategoryId: string;
    primaryCategoryName: string;
    secondaryCategoryId: string;
    secondaryCategoryName: string;
    isPrimaryCategory: boolean;
}

export interface BudgetDrilldownRouteQueryInput {
    categories: TransactionCategory[];
    primaryCategoryName: string;
    secondaryCategoryName?: string | null;
    fallbackCategoryId?: string;
    startDate?: string;
    endDate?: string;
    transactionType: number;
    accountIds?: string[];
    tagIds?: string[];
}

function parseBudgetDate(date: string | undefined, endOfDay: boolean): Date | null {
    if (!date || !/^\d{4}-\d{2}-\d{2}$/.test(date)) {
        return null;
    }

    const [yearText, monthText, dayText] = date.split('-');
    const year = Number(yearText);
    const month = Number(monthText);
    const day = Number(dayText);

    if (!Number.isInteger(year) || !Number.isInteger(month) || !Number.isInteger(day)) {
        return null;
    }

    return endOfDay
        ? new Date(year, month - 1, day, 23, 59, 59)
        : new Date(year, month - 1, day, 0, 0, 0);
}

/**
 * 根据预算记录与分类树解析主/子分类 ID，并保留可回显的分类名称。
 */
export function resolveBudgetCategorySelection(
    categories: TransactionCategory[],
    categoryId: string
): ResolvedBudgetCategorySelection | null {
    const normalizedCategoryId = String(categoryId || '');
    if (!normalizedCategoryId) {
        return null;
    }

    for (const primaryCategory of categories) {
        if (primaryCategory.id === normalizedCategoryId) {
            return {
                categoryId: primaryCategory.id,
                primaryCategoryId: primaryCategory.id,
                primaryCategoryName: primaryCategory.name,
                secondaryCategoryId: '',
                secondaryCategoryName: '',
                isPrimaryCategory: true
            };
        }

        for (const secondaryCategory of primaryCategory.subCategories || []) {
            if (secondaryCategory.id === normalizedCategoryId) {
                return {
                    categoryId: secondaryCategory.id,
                    primaryCategoryId: primaryCategory.id,
                    primaryCategoryName: primaryCategory.name,
                    secondaryCategoryId: secondaryCategory.id,
                    secondaryCategoryName: secondaryCategory.name,
                    isPrimaryCategory: false
                };
            }
        }
    }

    return null;
}

/**
 * 按主分类和子分类名称在分类树中查找预算钻取使用的分类 ID。
 */
export function findBudgetCategoryIdByNames(
    categories: TransactionCategory[],
    primaryCategoryName: string,
    secondaryCategoryName?: string
): string {
    for (const primaryCategory of categories) {
        if (primaryCategory.name !== primaryCategoryName) {
            continue;
        }

        if (!secondaryCategoryName) {
            return primaryCategory.id;
        }

        for (const secondaryCategory of primaryCategory.subCategories || []) {
            if (secondaryCategory.name === secondaryCategoryName) {
                return secondaryCategory.id;
            }
        }
    }

    return '';
}

/**
 * 从预算行提取交易钻取所需的主分类和子分类 ID。
 */
export function getBudgetDrilldownCategoryIds(
    categories: TransactionCategory[],
    primaryCategoryName: string,
    secondaryCategoryName?: string | null,
    fallbackCategoryId?: string
): string {
    for (const primaryCategory of categories) {
        if (primaryCategory.name !== primaryCategoryName) {
            continue;
        }

        if (secondaryCategoryName) {
            for (const secondaryCategory of primaryCategory.subCategories || []) {
                if (secondaryCategory.name === secondaryCategoryName) {
                    return secondaryCategory.id;
                }
            }

            return fallbackCategoryId || '';
        }

        const categoryIds = new Set<string>([primaryCategory.id]);

        for (const secondaryCategory of primaryCategory.subCategories || []) {
            if (secondaryCategory.id) {
                categoryIds.add(secondaryCategory.id);
            }
        }

        return Array.from(categoryIds).join(',');
    }

    return fallbackCategoryId || '';
}

/**
 * 构建预算执行明细跳转到交易列表时的路由 query。
 */
export function buildBudgetDrilldownRouteQuery({
    categories,
    primaryCategoryName,
    secondaryCategoryName,
    fallbackCategoryId,
    startDate,
    endDate,
    transactionType,
    accountIds,
    tagIds
}: BudgetDrilldownRouteQueryInput): Record<string, string> {
    const query: Record<string, string> = {
        type: String(transactionType)
    };

    const categoryIds = getBudgetDrilldownCategoryIds(
        categories,
        primaryCategoryName,
        secondaryCategoryName,
        fallbackCategoryId
    );

    if (categoryIds) {
        query['categoryIds'] = categoryIds;
    }

    const minDate = parseBudgetDate(startDate, false);
    const maxDate = parseBudgetDate(endDate, true);

    if (minDate && maxDate && minDate <= maxDate) {
        query['dateType'] = String(DateRange.Custom.type);
        query['minTime'] = String(getUnixTimeFromLocalDatetime(minDate));
        query['maxTime'] = String(getUnixTimeFromLocalDatetime(maxDate));
    }

    const normalizedAccountIds = (accountIds || []).filter(accountId => !!accountId);
    if (normalizedAccountIds.length > 0) {
        query['accountIds'] = normalizedAccountIds.join(',');
    }

    const normalizedTagIds = (tagIds || []).filter(tagId => !!tagId);
    if (normalizedTagIds.length > 0) {
        query['tagIds'] = normalizedTagIds.join(',');
    }

    return query;
}
