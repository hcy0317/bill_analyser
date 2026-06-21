import type { ImportPreviewFilterAccountCategoryLike, ImportPreviewFilterAccountLike, ImportPreviewFilterCategoryLike, ImportPreviewFilterGroup } from './types.ts';



// 预览 facet 分组只处理标签归属，避免表格组件和服务端查询逻辑重复遍历分类/账户树。

export function uniqueFilterLabels(labels: string[]): string[] {
    const seen = new Set<string>();
    const uniqueLabels: string[] = [];
    for (const label of labels) {
        const normalizedLabel = String(label || '').trim();
        if (!normalizedLabel || seen.has(normalizedLabel)) {
            continue;
        }
        seen.add(normalizedLabel);
        uniqueLabels.push(normalizedLabel);
    }
    return uniqueLabels;
}

export function pushGroupedLabel(
    groupedLabels: Map<string, string[]>,
    matchedLabels: Set<string>,
    groupTitle: string,
    label: string,
    availableLabels: Set<string>
): void {
    if (!availableLabels.has(label) || matchedLabels.has(label)) {
        return;
    }
    const labels = groupedLabels.get(groupTitle) || [];
    labels.push(label);
    groupedLabels.set(groupTitle, labels);
    matchedLabels.add(label);
}

export function groupImportPreviewCategoryFilterLabels(
    labels: string[],
    categoriesByType: Record<string | number, ImportPreviewFilterCategoryLike[]>,
    fallbackTitle: string
): ImportPreviewFilterGroup[] {
    const uniqueLabels = uniqueFilterLabels(labels);
    const availableLabels = new Set(uniqueLabels);
    const matchedLabels = new Set<string>();
    const groupedLabels = new Map<string, string[]>();

    for (const categories of Object.values(categoriesByType)) {
        for (const primaryCategory of categories || []) {
            if (!primaryCategory?.name) {
                continue;
            }
            pushGroupedLabel(groupedLabels, matchedLabels, primaryCategory.name, primaryCategory.name, availableLabels);
            for (const subCategory of primaryCategory.subCategories || []) {
                if (subCategory?.name) {
                    pushGroupedLabel(groupedLabels, matchedLabels, primaryCategory.name, subCategory.name, availableLabels);
                }
            }
        }
    }

    const groups = Array.from(groupedLabels, ([title, groupLabels]) => ({
        title,
        labels: groupLabels
    }));
    const ungroupedLabels = uniqueLabels.filter(label => !matchedLabels.has(label));
    if (ungroupedLabels.length > 0) {
        groups.push({
            title: fallbackTitle,
            labels: ungroupedLabels
        });
    }
    return groups;
}

export function groupImportPreviewAccountFilterLabels(
    labels: string[],
    accounts: ImportPreviewFilterAccountLike[],
    accountCategories: ImportPreviewFilterAccountCategoryLike[],
    fallbackTitle: string
): ImportPreviewFilterGroup[] {
    const uniqueLabels = uniqueFilterLabels(labels);
    const availableLabels = new Set(uniqueLabels);
    const matchedLabels = new Set<string>();
    const groupedLabels = new Map<string, string[]>();
    const categoryTitleByType = new Map<number, string>();
    for (const category of accountCategories) {
        categoryTitleByType.set(Number(category.type), category.name);
    }

    const visitAccount = (account: ImportPreviewFilterAccountLike, inheritedCategory?: number): void => {
        const categoryType = Number(account.category ?? inheritedCategory ?? NaN);
        const groupTitle = categoryTitleByType.get(categoryType) || fallbackTitle;
        if (account?.name) {
            pushGroupedLabel(groupedLabels, matchedLabels, groupTitle, account.name, availableLabels);
        }
        for (const subAccount of account.subAccounts || []) {
            visitAccount(subAccount, categoryType);
        }
    };

    for (const account of accounts || []) {
        visitAccount(account);
    }

    const groups = Array.from(groupedLabels, ([title, groupLabels]) => ({
        title,
        labels: groupLabels
    }));
    const ungroupedLabels = uniqueLabels.filter(label => !matchedLabels.has(label));
    if (ungroupedLabels.length > 0) {
        groups.push({
            title: fallbackTitle,
            labels: ungroupedLabels
        });
    }
    return groups;
}
