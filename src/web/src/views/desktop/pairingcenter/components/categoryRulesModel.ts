import { CategoryType } from '@/core/category.ts';
import type { LocalizedPresetCategory } from '@/core/category.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';

export interface LearningRuleOverviewItem {
    matchType: string;
    matchValue: string;
    learnedType: string;
    appliedCount: number;
    enabled: boolean;
}

export interface RecurringRuleOverviewItem {
    name: string;
    amountCents: number | null;
    frequency: string;
    nextDate: string | null;
    enabled: boolean;
}

export interface RuleCenterOverview {
    learningRules: LearningRuleOverviewItem[];
    learningRuleCount: number;
    categoryRuleCount: number;
    recurringRules: RecurringRuleOverviewItem[];
    recurringRuleCount: number;
    totalRuleCount: number;
}

export interface CategoryRuleItem {
    id: number;
    name: string;
    category_id: number | null;
    category_name: string | null;
    sub_category_name: string | null;
    priority: number;
    rule_expression: string;
    regex_enabled: boolean;
    enabled: boolean;
    applied_count: number;
}

export interface DisplayCategoryRuleItem extends CategoryRuleItem {
    category_display_name: string;
    category_full_name: string;
    category_icon: string;
    category_color: string;
    category_group_key: string;
    category_group_name: string;
    category_group_icon: string;
    category_group_color: string;
}

export interface CategoryRuleTargetGroup {
    key: string;
    category_display_name: string;
    category_full_name: string;
    category_icon: string;
    category_color: string;
    category_group_key: string;
    category_group_name: string;
    category_group_icon: string;
    category_group_color: string;
    rules: DisplayCategoryRuleItem[];
    ruleCount: number;
}

export interface CategoryRuleGroup {
    key: string;
    title: string;
    icon: string;
    color: string;
    targets: CategoryRuleTargetGroup[];
    ruleCount: number;
}

export interface CategoryRuleForm {
    category_id: string;
    priority: number;
    rule_expression: string;
    regex_enabled: boolean;
    enabled: boolean;
}

export interface CategoryRulePayload {
    category_id: number;
    name: string;
    priority: number;
    rule_expression: string;
    regex_enabled: boolean;
    enabled: boolean;
}

export interface CategoryPickerSecondaryItem extends Record<string, unknown> {
    id: string;
    name: string;
    icon: string;
    color: string;
    hidden: boolean;
}

export interface CategoryPickerPrimaryItem extends Record<string, unknown> {
    id: string;
    name: string;
    icon: string;
    color: string;
    hidden: boolean;
    type: CategoryType;
    typeLabel: string;
    subCategories: CategoryPickerSecondaryItem[];
}

export interface ResolvedRuleCategorySelection {
    primaryText: string;
    secondaryText: string;
    label: string;
}

export interface CategoryRuleTestResult {
    matched?: boolean | null;
}

export interface PrimaryCategoryDisplayInfo {
    id?: string;
    name: string;
    icon: string;
    color: string;
}

interface CategoryDisplayDependencies {
    categoriesById: Record<string, TransactionCategory>;
    categoryPickerItems: CategoryPickerPrimaryItem[];
    localizedPresetPrimaryCategoryMap: Map<string, PrimaryCategoryDisplayInfo>;
    unassignedLabel: string;
}

const orderedCategoryPickerTypes = [
    CategoryType.Expense,
    CategoryType.Income,
    CategoryType.Transfer,
    CategoryType.Investment,
];

/**
 * 构造分类规则编辑器使用的主/子分类树，保持分类类型展示顺序稳定。
 */
export function buildCategoryPickerItems(
    allTransactionCategories: Record<number, TransactionCategory[]>,
    getCategoryTypeLabel: (type: number) => string
): CategoryPickerPrimaryItem[] {
    return orderedCategoryPickerTypes.flatMap(type => (
        allTransactionCategories[type] || []
    ).map(primaryCategory => ({
        id: String(primaryCategory.id),
        name: primaryCategory.name,
        icon: primaryCategory.icon,
        color: primaryCategory.color,
        hidden: primaryCategory.hidden,
        type: Number(primaryCategory.type) as CategoryType,
        typeLabel: getCategoryTypeLabel(primaryCategory.type),
        subCategories: (primaryCategory.subCategories || []).map(subCategory => ({
            id: String(subCategory.id),
            name: subCategory.name,
            icon: subCategory.icon,
            color: subCategory.color,
            hidden: subCategory.hidden,
        })),
    })));
}

/**
 * 构造本地化预设主分类索引，用于后端规则只返回分类名时补回图标和颜色。
 */
export function buildLocalizedPresetPrimaryCategoryMap(
    locales: string[],
    getAllTransactionDefaultCategories: (type: number, locale: string) => Record<string, LocalizedPresetCategory[]>
): Map<string, PrimaryCategoryDisplayInfo> {
    const metadataByName = new Map<string, PrimaryCategoryDisplayInfo>();

    for (const locale of locales) {
        const localizedCategories = getAllTransactionDefaultCategories(0, locale);
        for (const categories of Object.values(localizedCategories)) {
            for (const category of categories ?? []) {
                const categoryNameKey = normalizeCategoryNameKey(category.name);
                if (!categoryNameKey || metadataByName.has(categoryNameKey)) {
                    continue;
                }
                metadataByName.set(categoryNameKey, {
                    name: category.name,
                    icon: category.icon,
                    color: String(category.color),
                });
            }
        }
    }

    return metadataByName;
}

/**
 * 把 category id 解析为 two-column-select 的当前主分类/子分类展示文本。
 */
export function resolveRuleCategorySelection(
    categoryId: string,
    categoryPickerItems: CategoryPickerPrimaryItem[]
): ResolvedRuleCategorySelection {
    const normalizedCategoryId = String(categoryId || '');

    for (const primaryCategory of categoryPickerItems) {
        if (primaryCategory.id === normalizedCategoryId) {
            return { primaryText: primaryCategory.name, secondaryText: '', label: primaryCategory.name };
        }

        for (const secondaryCategory of primaryCategory.subCategories) {
            if (secondaryCategory.id === normalizedCategoryId) {
                return {
                    primaryText: primaryCategory.name,
                    secondaryText: secondaryCategory.name,
                    label: `${primaryCategory.name} / ${secondaryCategory.name}`,
                };
            }
        }
    }

    return { primaryText: '', secondaryText: '', label: '' };
}

/**
 * 生成分类规则保存 payload，并在缺少分类或表达式时抛出业务校验错误。
 */
export function buildCategoryRulePayload(
    form: CategoryRuleForm,
    autoRuleName: string,
    messages: { categoryRequired: string; expressionRequired: string }
): CategoryRulePayload {
    const categoryId = Number.parseInt(String(form.category_id || ''), 10);
    if (!Number.isFinite(categoryId)) {
        throw new Error(messages.categoryRequired);
    }

    const ruleExpression = String(form.rule_expression || '').trim();
    if (!ruleExpression) {
        throw new Error(messages.expressionRequired);
    }

    return {
        category_id: categoryId,
        name: autoRuleName,
        priority: form.priority,
        rule_expression: ruleExpression,
        regex_enabled: !!form.regex_enabled,
        enabled: !!form.enabled,
    };
}

/**
 * 归一化后端分类规则响应，兼容旧字段 `main_category/sub_category`。
 */
export function normalizeCategoryRuleItem(
    item: CategoryRuleItem & { main_category?: string | null; sub_category?: string | null }
): CategoryRuleItem {
    return {
        ...item,
        category_name: item.category_name ?? item.main_category ?? null,
        sub_category_name: item.sub_category_name ?? item.sub_category ?? null,
        regex_enabled: !!item.regex_enabled,
        enabled: !!item.enabled,
        applied_count: Number(item.applied_count ?? 0),
    };
}

/**
 * 构造分类规则表格展示行，集中处理缺失分类、历史分类名和本地化预设兜底。
 */
export function buildDisplayCategoryRules(
    items: CategoryRuleItem[],
    dependencies: CategoryDisplayDependencies
): DisplayCategoryRuleItem[] {
    return items.map(item => {
        const display = resolveCategoryDisplay(item, dependencies);
        const group = resolveCategoryGroup(item, dependencies);
        return {
            ...item,
            category_display_name: display.name,
            category_full_name: display.fullName,
            category_icon: display.icon,
            category_color: display.color,
            category_group_key: group.key,
            category_group_name: group.name,
            category_group_icon: group.icon,
            category_group_color: group.color,
        };
    });
}

/**
 * 将分类规则按主分类和目标分类聚合，供表格一次展示同目标下的多条表达式。
 */
export function buildCategoryRuleTargetGroups(items: DisplayCategoryRuleItem[]): CategoryRuleTargetGroup[] {
    const groupsByKey = new Map<string, CategoryRuleTargetGroup>();

    for (const item of items) {
        const targetKey = makeCategoryRuleTargetKey(item);
        const group = groupsByKey.get(targetKey) ?? {
            key: targetKey,
            category_display_name: item.category_display_name,
            category_full_name: item.category_full_name,
            category_icon: item.category_icon,
            category_color: item.category_color,
            category_group_key: item.category_group_key,
            category_group_name: item.category_group_name,
            category_group_icon: item.category_group_icon,
            category_group_color: item.category_group_color,
            rules: [],
            ruleCount: 0,
        };
        group.rules.push(item);
        group.ruleCount = group.rules.length;
        groupsByKey.set(targetKey, group);
    }

    return [...groupsByKey.values()]
        .map(group => ({
            ...group,
            rules: [...group.rules].sort(compareCategoryRuleExpressions),
            ruleCount: group.rules.length,
        }))
        .sort(compareCategoryRuleTargetGroups);
}

/**
 * 把当前页目标分类组再按主分类聚合，供可折叠主分类段落渲染。
 */
export function buildGroupedCategoryRuleTargets(targetGroups: CategoryRuleTargetGroup[]): CategoryRuleGroup[] {
    const groupsByKey = new Map<string, CategoryRuleGroup>();

    for (const targetGroup of targetGroups) {
        const group = groupsByKey.get(targetGroup.category_group_key) ?? {
            key: targetGroup.category_group_key,
            title: targetGroup.category_group_name,
            icon: targetGroup.category_group_icon,
            color: targetGroup.category_group_color,
            targets: [],
            ruleCount: 0,
        };
        group.targets.push(targetGroup);
        group.ruleCount += targetGroup.ruleCount;
        groupsByKey.set(targetGroup.category_group_key, group);
    }

    return [...groupsByKey.values()]
        .map(group => ({
            ...group,
            targets: [...group.targets].sort(compareCategoryRuleTargetGroups),
            ruleCount: group.targets.reduce((sum, item) => sum + item.ruleCount, 0),
        }))
        .sort((firstGroup, secondGroup) => firstGroup.title.localeCompare(secondGroup.title, 'zh-Hans'));
}

/**
 * 计算一个目标分类分组内包含的规则 id。
 */
export function getCategoryRuleTargetRuleIds(targetGroup: CategoryRuleTargetGroup): number[] {
    return targetGroup.rules.map(item => item.id);
}

/**
 * 生成主分类分组 key；优先用 id，缺失时用名称兜底以兼容历史规则。
 */
export function makePrimaryCategoryGroupKey(categoryId: string | number | null | undefined, name?: string | null): string {
    const normalizedCategoryId = String(categoryId ?? '').trim();
    if (normalizedCategoryId) {
        return `category:${normalizedCategoryId}`;
    }

    const normalizedName = normalizeCategoryNameKey(name);
    return normalizedName ? `category-name:${normalizedName}` : 'unassigned';
}

/**
 * 规则中心分类规则的稳定排序：先主分类，再目标分类，最后规则名称和 id。
 */
export function compareDisplayCategoryRuleOrder(
    firstRule: DisplayCategoryRuleItem,
    secondRule: DisplayCategoryRuleItem
): number {
    return firstRule.category_group_name.localeCompare(secondRule.category_group_name, 'zh-Hans')
        || firstRule.category_group_key.localeCompare(secondRule.category_group_key, 'zh-Hans')
        || compareDisplayCategoryRules(firstRule, secondRule);
}

function normalizeCategoryNameKey(value: string | null | undefined): string {
    return String(value || '').trim().toLocaleLowerCase();
}

function findPrimaryCategoryByName(
    name: string | null | undefined,
    categoryPickerItems: CategoryPickerPrimaryItem[],
    localizedPresetPrimaryCategoryMap: Map<string, PrimaryCategoryDisplayInfo>
): PrimaryCategoryDisplayInfo | null {
    const normalizedName = normalizeCategoryNameKey(name);
    if (!normalizedName) {
        return null;
    }

    const storedCategory = categoryPickerItems.find(primaryCategory => (
        normalizeCategoryNameKey(primaryCategory.name) === normalizedName
    ));
    return storedCategory ?? localizedPresetPrimaryCategoryMap.get(normalizedName) ?? null;
}

function resolveCategoryDisplay(
    item: CategoryRuleItem,
    dependencies: CategoryDisplayDependencies
): { name: string; fullName: string; icon: string; color: string } {
    const categoryId = item.category_id !== null && item.category_id !== undefined ? String(item.category_id) : '';
    const category = categoryId ? dependencies.categoriesById[categoryId] : null;
    const displayName = item.sub_category_name || category?.name || item.category_name || dependencies.unassignedLabel;
    const fullName = item.category_name && item.sub_category_name
        ? `${item.category_name} / ${item.sub_category_name}`
        : displayName;

    if (category) {
        return { name: displayName, fullName, icon: category.icon, color: String(category.color) };
    }

    const fallbackPrimaryCategory = findPrimaryCategoryByName(
        item.category_name,
        dependencies.categoryPickerItems,
        dependencies.localizedPresetPrimaryCategoryMap
    );
    return {
        name: item.sub_category_name || item.category_name || dependencies.unassignedLabel,
        fullName,
        icon: fallbackPrimaryCategory?.icon ?? '',
        color: fallbackPrimaryCategory?.color ?? '',
    };
}

function resolveCategoryGroup(
    item: CategoryRuleItem,
    dependencies: CategoryDisplayDependencies
): { key: string; name: string; icon: string; color: string } {
    const categoryId = item.category_id !== null && item.category_id !== undefined ? String(item.category_id) : '';
    const category = categoryId ? dependencies.categoriesById[categoryId] : null;
    const primaryCategory = category && category.parentId && category.parentId !== '0'
        ? dependencies.categoriesById[category.parentId]
        : category;

    if (primaryCategory) {
        return {
            key: makePrimaryCategoryGroupKey(primaryCategory.id, primaryCategory.name),
            name: primaryCategory.name,
            icon: primaryCategory.icon,
            color: String(primaryCategory.color),
        };
    }

    const fallbackPrimaryCategory = findPrimaryCategoryByName(
        item.category_name,
        dependencies.categoryPickerItems,
        dependencies.localizedPresetPrimaryCategoryMap
    );
    if (fallbackPrimaryCategory) {
        return {
            key: makePrimaryCategoryGroupKey(fallbackPrimaryCategory.id, fallbackPrimaryCategory.name),
            name: fallbackPrimaryCategory.name,
            icon: fallbackPrimaryCategory.icon,
            color: fallbackPrimaryCategory.color,
        };
    }

    const fallbackName = item.category_name || item.sub_category_name || dependencies.unassignedLabel;
    return {
        key: makePrimaryCategoryGroupKey(null, fallbackName),
        name: fallbackName || dependencies.unassignedLabel,
        icon: '',
        color: '',
    };
}

function compareDisplayCategoryRules(firstRule: DisplayCategoryRuleItem, secondRule: DisplayCategoryRuleItem): number {
    return firstRule.category_full_name.localeCompare(secondRule.category_full_name, 'zh-Hans')
        || firstRule.name.localeCompare(secondRule.name, 'zh-Hans')
        || firstRule.id - secondRule.id;
}

function compareCategoryRuleExpressions(firstRule: DisplayCategoryRuleItem, secondRule: DisplayCategoryRuleItem): number {
    return firstRule.priority - secondRule.priority
        || firstRule.name.localeCompare(secondRule.name, 'zh-Hans')
        || firstRule.id - secondRule.id;
}

function compareCategoryRuleTargetGroups(firstGroup: CategoryRuleTargetGroup, secondGroup: CategoryRuleTargetGroup): number {
    return firstGroup.category_group_name.localeCompare(secondGroup.category_group_name, 'zh-Hans')
        || firstGroup.category_group_key.localeCompare(secondGroup.category_group_key, 'zh-Hans')
        || firstGroup.category_full_name.localeCompare(secondGroup.category_full_name, 'zh-Hans')
        || firstGroup.key.localeCompare(secondGroup.key, 'zh-Hans');
}

function makeCategoryRuleTargetKey(item: DisplayCategoryRuleItem): string {
    const categoryId = item.category_id !== null && item.category_id !== undefined ? String(item.category_id).trim() : '';
    if (categoryId) {
        return `category:${categoryId}`;
    }

    const normalizedCategoryName = normalizeCategoryNameKey(item.category_full_name);
    return normalizedCategoryName ? `category-name:${normalizedCategoryName}` : 'unassigned';
}
