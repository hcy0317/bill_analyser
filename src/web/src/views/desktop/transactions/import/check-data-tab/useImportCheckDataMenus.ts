import { computed, type ComputedRef, type Ref } from 'vue';
import { type NameValue } from '@/core/base.ts';
import { DateRange, type WeekDayValue } from '@/core/datetime.ts';
import { TransactionType } from '@/core/transaction.ts';
import {
    mdiAutoFix,
    mdiCheck,
    mdiFindReplace,
    mdiShapePlusOutline,
    mdiTransfer
} from '@mdi/js';
import type { BatchCreateDialogDataType } from '../dialogs/BatchCreateDialog.vue';
import type { BatchReplaceDialogDataType } from '../dialogs/BatchReplaceDialog.vue';
import { resolveImportCheckDatePresetRange } from '../checkDataFilters.ts';
import type {
    ImportTransactionCheckDataFilter,
    ImportTransactionCheckDataMenu,
    ImportTransactionCheckDataMenuGroup
} from '../checkDataTypes.ts';
import type { ImportPreviewFilterGroup } from '../importPreviewIndex.ts';
interface ImportCheckDataMenusOptions {
    allInvalidAccountNames: ComputedRef<NameValue[]>;
    allInvalidExpenseCategoryNames: ComputedRef<NameValue[]>;
    allInvalidIncomeCategoryNames: ComputedRef<NameValue[]>;
    allInvalidTransactionTagNames: ComputedRef<NameValue[]>;
    allInvalidTransferCategoryNames: ComputedRef<NameValue[]>;
    allOriginalTransactionTagNames: ComputedRef<NameValue[]>;
    allUsedAccountFilterGroups: ComputedRef<ImportPreviewFilterGroup[]>;
    allUsedCategoryFilterGroups: ComputedRef<ImportPreviewFilterGroup[]>;
    allUsedTagNames: ComputedRef<string[]>;
    clearSelectedRecurringMatches: () => void;
    convertTransactionType: (fromType: TransactionType, toType: TransactionType) => void;
    currentDateFilterType: ComputedRef<number>;
    currentDescriptionFilterValue: Ref<string | null>;
    displayFilterCustomDateRange: ComputedRef<string>;
    filters: Ref<ImportTransactionCheckDataFilter>;
    firstDayOfWeek: ComputedRef<WeekDayValue>;
    fiscalYearStartValue: ComputedRef<number>;
    getAnnotationFilterTitle: () => string;
    getNeedsReviewOrAnnotatedText: () => string;
    getNoAnnotationIssuesText: () => string;
    isEditing: ComputedRef<boolean>;
    selectedExpenseTransactionCount: ComputedRef<number>;
    selectedImportTransactionCount: ComputedRef<number>;
    selectedIncomeTransactionCount: ComputedRef<number>;
    selectedRecurringMatchCount: ComputedRef<number>;
    selectedTransferTransactionCount: ComputedRef<number>;
    showBatchAddDialog: (type: BatchReplaceDialogDataType) => void;
    showBatchCreateInvalidItemDialog: (type: BatchCreateDialogDataType, invalidItems: NameValue[]) => void;
    showBatchReplaceDialog: (type: BatchReplaceDialogDataType, allSourceTagItems?: NameValue[]) => void;
    showCustomDateRangeDialog: Ref<boolean>;
    showCustomDescriptionDialog: Ref<boolean>;
    showReplaceAllTypesDialog: () => void;
    showReplaceInvalidItemDialog: (type: BatchReplaceDialogDataType, invalidItems: NameValue[]) => void;
    tt: (key: string) => string;
}

// 预览页菜单由当前筛选、选择计数和编辑状态共同派生，集中在这里避免表格组件重复维护菜单禁用规则。
export function useImportCheckDataMenus(options: ImportCheckDataMenusOptions) {
    const {
        allInvalidAccountNames,
        allInvalidExpenseCategoryNames,
        allInvalidIncomeCategoryNames,
        allInvalidTransactionTagNames,
        allInvalidTransferCategoryNames,
        allOriginalTransactionTagNames,
        allUsedAccountFilterGroups,
        allUsedCategoryFilterGroups,
        allUsedTagNames,
        clearSelectedRecurringMatches,
        convertTransactionType,
        currentDateFilterType,
        currentDescriptionFilterValue,
        displayFilterCustomDateRange,
        filters,
        firstDayOfWeek,
        fiscalYearStartValue,
        getAnnotationFilterTitle,
        getNeedsReviewOrAnnotatedText,
        getNoAnnotationIssuesText,
        isEditing,
        selectedExpenseTransactionCount,
        selectedImportTransactionCount,
        selectedIncomeTransactionCount,
        selectedRecurringMatchCount,
        selectedTransferTransactionCount,
        showBatchAddDialog,
        showBatchCreateInvalidItemDialog,
        showBatchReplaceDialog,
        showCustomDateRangeDialog,
        showCustomDescriptionDialog,
        showReplaceAllTypesDialog,
        showReplaceInvalidItemDialog,
        tt
    } = options;

    // 菜单构建器只描述筛选和批量操作入口，实际 mutation 仍由父组件函数执行。
    function getDateFilterSummary(): string {
        switch (currentDateFilterType.value) {
            case DateRange.ThisWeek.type:
                return tt('This week');
            case DateRange.ThisMonth.type:
                return tt('This month');
            case DateRange.ThisYear.type:
                return tt('This year');
            case DateRange.Custom.type:
                return displayFilterCustomDateRange.value || tt('Custom');
            default:
                return tt('All');
        }
    }
    function isCurrentDateFilterPreset(dateType: number): boolean {
        return currentDateFilterType.value === dateType;
    }
    function applyDateFilterPreset(dateType: number): void {
        const range = resolveImportCheckDatePresetRange(
            dateType,
            firstDayOfWeek.value,
            fiscalYearStartValue.value
        );
        filters.value.minDatetime = range.minDatetime;
        filters.value.maxDatetime = range.maxDatetime;
    }
    function getTypeFilterSummary(): string {
        switch (filters.value.transactionType) {
            case TransactionType.Income:
                return tt('Income');
            case TransactionType.Expense:
                return tt('Expense');
            case TransactionType.Transfer:
                return tt('Transfer');
            case TransactionType.Investment:
                return tt('Investment');
            default:
                return tt('All');
        }
    }
    function getNamedFilterSummary(value: string | null | undefined, invalidLabel: string): string {
        if (value === null) {
            return tt('All');
        }
        if (value === undefined) {
            return invalidLabel;
        }
        if (value === '') {
            return tt('None');
        }
        return value;
    }
    function getAnnotationFilterSummary(): string {
        if (filters.value.annotation === 'needs-review') {
            return getNeedsReviewOrAnnotatedText();
        }
        if (filters.value.annotation === 'no-issues') {
            return getNoAnnotationIssuesText();
        }
        return tt('All');
    }
    function getSignalFilterSummary(): string {
        switch (filters.value.signal) {
            case 'parser':
                return tt('Parser');
            case 'platform_duplicate':
                return tt('Platform Duplicate');
            case 'transfer':
                return tt('Transfer Match');
            case 'history':
                return tt('History Rewrite');
            case 'learning':
                return tt('Learning Suggestion');
            case 'llm':
                return tt('LLM Suggestion');
            default:
                return tt('All');
        }
    }
    function getDescriptionFilterSummary(): string {
        if (filters.value.description === null) {
            return tt('All');
        }
        if (filters.value.description === '') {
            return tt('None');
        }
        return filters.value.description;
    }
    function buildGroupedFilterMenuItems(
        groups: Array<{ title: string; labels: string[] }>,
        selectedValue: string | null | undefined,
        onSelect: (value: string) => void,
        localizeGroupTitle = false
    ): ImportTransactionCheckDataMenu[] {
        return groups.map(group => ({
            title: localizeGroupTitle ? tt(group.title) : group.title,
            items: group.labels.map(label => ({
                title: label,
                appendIcon: selectedValue === label ? mdiCheck : undefined,
                onClick: () => onSelect(label)
            }))
        }));
    }
    const filterMenus = computed<ImportTransactionCheckDataMenuGroup[]>(() => [
        {
            title: getAnnotationFilterTitle(),
            summary: getAnnotationFilterSummary(),
            items: [
                {
                    title: tt('All'),
                    appendIcon: filters.value.annotation === null ? mdiCheck : undefined,
                    onClick: () => filters.value.annotation = null
                },
                {
                    title: getNeedsReviewOrAnnotatedText(),
                    appendIcon: filters.value.annotation === 'needs-review' ? mdiCheck : undefined,
                    onClick: () => filters.value.annotation = 'needs-review'
                },
                {
                    title: getNoAnnotationIssuesText(),
                    appendIcon: filters.value.annotation === 'no-issues' ? mdiCheck : undefined,
                    onClick: () => filters.value.annotation = 'no-issues'
                }
            ]
        },
        {
            title: tt('Signals'),
            summary: getSignalFilterSummary(),
            items: [
                {
                    title: tt('All'),
                    appendIcon: filters.value.signal === null ? mdiCheck : undefined,
                    onClick: () => filters.value.signal = null
                },
                {
                    title: tt('Parser'),
                    appendIcon: filters.value.signal === 'parser' ? mdiCheck : undefined,
                    onClick: () => filters.value.signal = 'parser'
                },
                {
                    title: tt('Platform Duplicate'),
                    appendIcon: filters.value.signal === 'platform_duplicate' ? mdiCheck : undefined,
                    onClick: () => filters.value.signal = 'platform_duplicate'
                },
                {
                    title: tt('Transfer Match'),
                    appendIcon: filters.value.signal === 'transfer' ? mdiCheck : undefined,
                    onClick: () => filters.value.signal = 'transfer'
                },
                {
                    title: tt('History Rewrite'),
                    appendIcon: filters.value.signal === 'history' ? mdiCheck : undefined,
                    onClick: () => filters.value.signal = 'history'
                },
                {
                    title: tt('Learning Suggestion'),
                    appendIcon: filters.value.signal === 'learning' ? mdiCheck : undefined,
                    onClick: () => filters.value.signal = 'learning'
                },
                {
                    title: tt('LLM Suggestion'),
                    appendIcon: filters.value.signal === 'llm' ? mdiCheck : undefined,
                    onClick: () => filters.value.signal = 'llm'
                }
            ]
        },
        {
            title: tt('Date Range'),
            summary: getDateFilterSummary(),
            items: [
                {
                    title: tt('All'),
                    appendIcon: isCurrentDateFilterPreset(DateRange.All.type) ? mdiCheck : undefined,
                    onClick: () => applyDateFilterPreset(DateRange.All.type)
                },
                {
                    title: tt('This week'),
                    appendIcon: isCurrentDateFilterPreset(DateRange.ThisWeek.type) ? mdiCheck : undefined,
                    onClick: () => applyDateFilterPreset(DateRange.ThisWeek.type)
                },
                {
                    title: tt('This month'),
                    appendIcon: isCurrentDateFilterPreset(DateRange.ThisMonth.type) ? mdiCheck : undefined,
                    onClick: () => applyDateFilterPreset(DateRange.ThisMonth.type)
                },
                {
                    title: tt('This year'),
                    appendIcon: isCurrentDateFilterPreset(DateRange.ThisYear.type) ? mdiCheck : undefined,
                    onClick: () => applyDateFilterPreset(DateRange.ThisYear.type)
                },
                {
                    title: tt('Custom'),
                    subTitle: currentDateFilterType.value === DateRange.Custom.type ? displayFilterCustomDateRange.value : undefined,
                    appendIcon: isCurrentDateFilterPreset(DateRange.Custom.type) ? mdiCheck : undefined,
                    onClick: () => showCustomDateRangeDialog.value = true
                }
            ]
        },
        {
            title: tt('Type'),
            summary: getTypeFilterSummary(),
            items: [
                {
                    title: tt('All'),
                    appendIcon: filters.value.transactionType === null ? mdiCheck : undefined,
                    onClick: () => filters.value.transactionType = null
                },
                {
                    title: tt('Income'),
                    appendIcon: filters.value.transactionType === TransactionType.Income ? mdiCheck : undefined,
                    onClick: () => filters.value.transactionType = TransactionType.Income
                },
                {
                    title: tt('Expense'),
                    appendIcon: filters.value.transactionType === TransactionType.Expense ? mdiCheck : undefined,
                    onClick: () => filters.value.transactionType = TransactionType.Expense
                },
                {
                    title: tt('Transfer'),
                    appendIcon: filters.value.transactionType === TransactionType.Transfer ? mdiCheck : undefined,
                    onClick: () => filters.value.transactionType = TransactionType.Transfer
                },
                {
                    title: tt('Investment'),
                    appendIcon: filters.value.transactionType === TransactionType.Investment ? mdiCheck : undefined,
                    onClick: () => filters.value.transactionType = TransactionType.Investment
                }
            ]
        },
        {
            title: tt('Category'),
            summary: getNamedFilterSummary(filters.value.category, tt('Invalid Category')),
            items: [
                {
                    title: tt('All'),
                    appendIcon: filters.value.category === null ? mdiCheck : undefined,
                    onClick: () => filters.value.category = null
                },
                {
                    title: tt('Invalid Category'),
                    appendIcon: filters.value.category === undefined ? mdiCheck : undefined,
                    onClick: () => filters.value.category = undefined
                },
                {
                    title: tt('None'),
                    appendIcon: filters.value.category === '' ? mdiCheck : undefined,
                    onClick: () => filters.value.category = ''
                },
                ...buildGroupedFilterMenuItems(
                    allUsedCategoryFilterGroups.value,
                    filters.value.category,
                    value => filters.value.category = value
                )
            ]
        },
        {
            title: tt('Account'),
            summary: getNamedFilterSummary(filters.value.account, tt('Invalid Account')),
            items: [
                {
                    title: tt('All'),
                    appendIcon: filters.value.account === null ? mdiCheck : undefined,
                    onClick: () => filters.value.account = null
                },
                {
                    title: tt('Invalid Account'),
                    appendIcon: filters.value.account === undefined ? mdiCheck : undefined,
                    onClick: () => filters.value.account = undefined
                },
                {
                    title: tt('None'),
                    appendIcon: filters.value.account === '' ? mdiCheck : undefined,
                    onClick: () => filters.value.account = ''
                },
                ...buildGroupedFilterMenuItems(
                    allUsedAccountFilterGroups.value,
                    filters.value.account,
                    value => filters.value.account = value,
                    true
                )
            ]
        },
        {
            title: tt('Tags'),
            summary: getNamedFilterSummary(filters.value.tag, tt('Invalid Tag')),
            items: [
                {
                    title: tt('All'),
                    appendIcon: filters.value.tag === null ? mdiCheck : undefined,
                    onClick: () => filters.value.tag = null
                },
                {
                    title: tt('Invalid Tag'),
                    appendIcon: filters.value.tag === undefined ? mdiCheck : undefined,
                    onClick: () => filters.value.tag = undefined
                },
                {
                    title: tt('None'),
                    appendIcon: filters.value.tag === '' ? mdiCheck : undefined,
                    onClick: () => filters.value.tag = ''
                },
                ...allUsedTagNames.value.map(name => ({
                    title: name,
                    appendIcon: filters.value.tag === name ? mdiCheck : undefined,
                    onClick: () => filters.value.tag = name
                }))
            ]
        },
        {
            title: tt('Description'),
            summary: getDescriptionFilterSummary(),
            items: [
                {
                    title: tt('All'),
                    appendIcon: filters.value.description === null ? mdiCheck : undefined,
                    onClick: () => filters.value.description = null
                },
                {
                    title: tt('None'),
                    appendIcon: filters.value.description === '' ? mdiCheck : undefined,
                    onClick: () => filters.value.description = ''
                },
                {
                    title: tt('Custom'),
                    subTitle: filters.value.description !== null ? filters.value.description : undefined,
                    appendIcon: filters.value.description !== null && filters.value.description !== '' ? mdiCheck : undefined,
                    onClick: () => {
                        currentDescriptionFilterValue.value = filters.value.description || '';
                        showCustomDescriptionDialog.value = true;
                    }
                }
            ]
        }
    ]);
    const toolMenus = computed<ImportTransactionCheckDataMenu[]>(() => [
        {
            prependIcon: mdiFindReplace,
            title: tt('Batch Replace Selected Expense Categories'),
            disabled: isEditing.value || selectedExpenseTransactionCount.value < 1,
            onClick: () => showBatchReplaceDialog('expenseCategory')
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Batch Replace Selected Income Categories'),
            disabled: isEditing.value || selectedIncomeTransactionCount.value < 1,
            onClick: () => showBatchReplaceDialog('incomeCategory')
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Batch Replace Selected Transfer Categories'),
            disabled: isEditing.value || selectedTransferTransactionCount.value < 1,
            onClick: () => showBatchReplaceDialog('transferCategory')
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Batch Replace Selected Accounts'),
            disabled: isEditing.value || selectedImportTransactionCount.value < 1,
            onClick: () => showBatchReplaceDialog('account')
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Batch Replace Selected Destination Accounts'),
            disabled: isEditing.value || selectedTransferTransactionCount.value < 1,
            onClick: () => showBatchReplaceDialog('destinationAccount')
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Batch Replace Selected Transaction Tags'),
            disabled: isEditing.value || selectedImportTransactionCount.value < 1,
            onClick: () => showBatchReplaceDialog('tag', allOriginalTransactionTagNames.value)
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Batch Add Transaction Tags'),
            disabled: isEditing.value || selectedImportTransactionCount.value < 1,
            onClick: () => showBatchAddDialog('tag')
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Replace Invalid Expense Categories'),
            disabled: isEditing.value || !allInvalidExpenseCategoryNames.value || allInvalidExpenseCategoryNames.value.length < 1,
            divider: true,
            onClick: () => showReplaceInvalidItemDialog('expenseCategory', allInvalidExpenseCategoryNames.value)
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Replace Invalid Income Categories'),
            disabled: isEditing.value || !allInvalidIncomeCategoryNames.value || allInvalidIncomeCategoryNames.value.length < 1,
            onClick: () => showReplaceInvalidItemDialog('incomeCategory', allInvalidIncomeCategoryNames.value)
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Replace Invalid Transfer Categories'),
            disabled: isEditing.value || !allInvalidTransferCategoryNames.value || allInvalidTransferCategoryNames.value.length < 1,
            onClick: () => showReplaceInvalidItemDialog('transferCategory', allInvalidTransferCategoryNames.value)
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Replace Invalid Accounts'),
            disabled: isEditing.value || !allInvalidAccountNames.value || allInvalidAccountNames.value.length < 1,
            onClick: () => showReplaceInvalidItemDialog('account', allInvalidAccountNames.value)
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Replace Invalid Transaction Tags'),
            disabled: isEditing.value || !allInvalidTransactionTagNames.value || allInvalidTransactionTagNames.value.length < 1,
            onClick: () => showReplaceInvalidItemDialog('tag', allInvalidTransactionTagNames.value)
        },
        {
            prependIcon: mdiFindReplace,
            title: tt('Batch Replace Categories / Accounts / Tags'),
            disabled: isEditing.value,
            divider: true,
            onClick: showReplaceAllTypesDialog
        },
        {
            prependIcon: mdiShapePlusOutline,
            title: tt('Create Nonexistent Expense Categories'),
            disabled: isEditing.value || !allInvalidExpenseCategoryNames.value || allInvalidExpenseCategoryNames.value.length < 1,
            divider: true,
            onClick: () => showBatchCreateInvalidItemDialog('expenseCategory', allInvalidExpenseCategoryNames.value)
        },
        {
            prependIcon: mdiShapePlusOutline,
            title: tt('Create Nonexistent Income Categories'),
            disabled: isEditing.value || !allInvalidIncomeCategoryNames.value || allInvalidIncomeCategoryNames.value.length < 1,
            onClick: () => showBatchCreateInvalidItemDialog('incomeCategory', allInvalidIncomeCategoryNames.value)
        },
        {
            prependIcon: mdiShapePlusOutline,
            title: tt('Create Nonexistent Transfer Categories'),
            disabled: isEditing.value || !allInvalidTransferCategoryNames.value || allInvalidTransferCategoryNames.value.length < 1,
            onClick: () => showBatchCreateInvalidItemDialog('transferCategory', allInvalidTransferCategoryNames.value)
        },
        {
            prependIcon: mdiShapePlusOutline,
            title: tt('Create Nonexistent Transaction Tags'),
            disabled: isEditing.value || !allInvalidTransactionTagNames.value || allInvalidTransactionTagNames.value.length < 1,
            onClick: () => showBatchCreateInvalidItemDialog('tag', allInvalidTransactionTagNames.value)
        },
        {
            prependIcon: mdiTransfer,
            title: tt('Batch Convert Expense Transaction to Income Transaction'),
            disabled: isEditing.value || selectedExpenseTransactionCount.value < 1,
            divider: true,
            onClick: () => convertTransactionType(TransactionType.Expense, TransactionType.Income)
        },
        {
            prependIcon: mdiTransfer,
            title: tt('Batch Convert Expense Transaction to Transfer Transaction'),
            disabled: isEditing.value || selectedExpenseTransactionCount.value < 1,
            onClick: () => convertTransactionType(TransactionType.Expense, TransactionType.Transfer)
        },
        {
            prependIcon: mdiTransfer,
            title: tt('Batch Convert Income Transaction to Expense Transaction'),
            disabled: isEditing.value || selectedIncomeTransactionCount.value < 1,
            onClick: () => convertTransactionType(TransactionType.Income, TransactionType.Expense)
        },
        {
            prependIcon: mdiTransfer,
            title: tt('Batch Convert Income Transaction to Transfer Transaction'),
            disabled: isEditing.value || selectedIncomeTransactionCount.value < 1,
            onClick: () => convertTransactionType(TransactionType.Income, TransactionType.Transfer)
        },
        {
            prependIcon: mdiTransfer,
            title: tt('Batch Convert Transfer Transaction to Expense Transaction'),
            disabled: isEditing.value || selectedTransferTransactionCount.value < 1,
            onClick: () => convertTransactionType(TransactionType.Transfer, TransactionType.Expense)
        },
        {
            prependIcon: mdiTransfer,
            title: tt('Batch Convert Transfer Transaction to Income Transaction'),
            disabled: isEditing.value || selectedTransferTransactionCount.value < 1,
            onClick: () => convertTransactionType(TransactionType.Transfer, TransactionType.Income)
        },
        {
            prependIcon: mdiAutoFix,
            title: tt('Clear Selected Scheduled Matches'),
            disabled: isEditing.value || selectedRecurringMatchCount.value < 1,
            divider: true,
            onClick: clearSelectedRecurringMatches
        }
    ]);
    return { filterMenus, toolMenus };
}
