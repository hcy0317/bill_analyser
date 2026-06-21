import type { ComputedRef, Ref } from 'vue';

import { type NameValue, reversed } from '@/core/base.ts';
import { TransactionType } from '@/core/transaction.ts';
import { getSecondaryTransactionMapByName, transactionTypeToCategoryType } from '@/lib/category.ts';
import type { Account } from '@/models/account.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import type { TransactionTag } from '@/models/transaction_tag.ts';

import type BatchCreateDialog from '../dialogs/BatchCreateDialog.vue';
import type { BatchCreateDialogDataType } from '../dialogs/BatchCreateDialog.vue';
import type BatchReplaceDialog from '../dialogs/BatchReplaceDialog.vue';
import type { BatchReplaceDialogDataType } from '../dialogs/BatchReplaceDialog.vue';
import type BatchReplaceAllTypesDialog from '../dialogs/BatchReplaceAllTypesDialog.vue';

type BatchReplaceDialogType = InstanceType<typeof BatchReplaceDialog>;
type BatchReplaceAllTypesDialogType = InstanceType<typeof BatchReplaceAllTypesDialog>;
type BatchCreateDialogType = InstanceType<typeof BatchCreateDialog>;

interface SnackbarLike {
    showMessage(message: string, payload?: Record<string, unknown>): void;
}

interface ImportCheckDataBatchActionsOptions {
    allAccountsMapByName: ComputedRef<Record<string, Account>>;
    allAccountsMap: ComputedRef<Record<string, Account>>;
    allCategories: ComputedRef<Record<number, TransactionCategory[]>>;
    allCategoriesMap: ComputedRef<Record<string, TransactionCategory>>;
    allInvalidAccountNames: ComputedRef<NameValue[]>;
    allInvalidExpenseCategoryNames: ComputedRef<NameValue[]>;
    allInvalidIncomeCategoryNames: ComputedRef<NameValue[]>;
    allInvalidTransactionTagNames: ComputedRef<NameValue[]>;
    allInvalidTransferCategoryNames: ComputedRef<NameValue[]>;
    allTagsMap: ComputedRef<Record<string, TransactionTag>>;
    assignCategoryIdIfKnown: (transaction: ImportTransaction, categoryId: string | number | null | undefined) => boolean;
    assignDestinationAccountIdIfKnown: (transaction: ImportTransaction, accountId: string | number | null | undefined) => boolean;
    assignSourceAccountIdIfKnown: (transaction: ImportTransaction, accountId: string | number | null | undefined) => boolean;
    batchCreateDialog: Readonly<Ref<BatchCreateDialogType | null>>;
    batchReplaceAllTypesDialog: Readonly<Ref<BatchReplaceAllTypesDialogType | null>>;
    batchReplaceDialog: Readonly<Ref<BatchReplaceDialogType | null>>;
    getDisplayCount: (count: number) => string;
    importTransactions: ComputedRef<ImportTransaction[]>;
    isEditing: ComputedRef<boolean>;
    snackbar: Readonly<Ref<SnackbarLike | null>>;
    syncActionableSuggestionDraftState: (transaction: ImportTransaction) => void;
    updateTransactionData: (transaction: ImportTransaction) => void;
}

// 批量动作直接修改导入预览草稿并同步建议基线，必须保持 category/account/tag 的当前用户可见性校验。
export function useImportCheckDataBatchActions(options: ImportCheckDataBatchActionsOptions) {
    const {
        allAccountsMapByName,
        allAccountsMap,
        allCategories,
        allCategoriesMap,
        allInvalidAccountNames,
        allInvalidExpenseCategoryNames,
        allInvalidIncomeCategoryNames,
        allInvalidTransactionTagNames,
        allInvalidTransferCategoryNames,
        allTagsMap,
        assignCategoryIdIfKnown,
        assignDestinationAccountIdIfKnown,
        assignSourceAccountIdIfKnown,
        batchCreateDialog,
        batchReplaceAllTypesDialog,
        batchReplaceDialog,
        getDisplayCount,
        importTransactions,
        isEditing,
        snackbar,
        syncActionableSuggestionDraftState,
        updateTransactionData
    } = options;

    // 批量动作集中处理导入预览草稿 mutation，菜单模块只负责触发这些动作入口。
    function showBatchReplaceDialog(type: BatchReplaceDialogDataType, allSourceTagItems?: NameValue[]): void {
        if (isEditing.value) {
            return;
        }

        batchReplaceDialog.value?.open({
            mode: 'batchReplace',
            type: type,
            allSourceTagItems: allSourceTagItems
        }).then(result => {
            if (!result) {
                return;
            }

            if (type !== 'tag') {
                if (!result.targetItem) {
                    return;
                }
            }

            let updatedCount = 0;

            if (importTransactions.value.length) {
                for (const importTransaction of importTransactions.value) {
                    if (!importTransaction.selected) {
                        continue;
                    }

                    let updated = false;

                    if (type === 'expenseCategory') {
                        if (importTransaction.type === TransactionType.Expense) {
                            updated = assignCategoryIdIfKnown(importTransaction, result.targetItem);
                        }
                    } else if (type === 'incomeCategory') {
                        if (importTransaction.type === TransactionType.Income) {
                            updated = assignCategoryIdIfKnown(importTransaction, result.targetItem);
                        }
                    } else if (type === 'transferCategory') {
                        if (importTransaction.type === TransactionType.Transfer) {
                            updated = assignCategoryIdIfKnown(importTransaction, result.targetItem);
                        }
                    } else if (type === 'account') {
                        updated = assignSourceAccountIdIfKnown(importTransaction, result.targetItem);
                    } else if (type === 'destinationAccount') {
                        if (importTransaction.type === TransactionType.Transfer) {
                            updated = assignDestinationAccountIdIfKnown(importTransaction, result.targetItem);
                        }
                    } else if (type === 'tag') {
                        const removeIndex: number[] = [];

                        for (let tagIndex = 0; tagIndex < importTransaction.originalTagNames.length; tagIndex++) {
                            const originalTagName = importTransaction.originalTagNames ? (importTransaction.originalTagNames[tagIndex] ?? '') : '';

                            if (originalTagName === result.sourceItem) {
                                if (result.targetItem) {
                                    importTransaction.tagIds[tagIndex] = result.targetItem;
                                    importTransaction.originalTagNames[tagIndex] = allTagsMap.value[result.targetItem]?.name || '';
                                } else {
                                    removeIndex.push(tagIndex);
                                }
                                updated = true;
                            }
                        }

                        for (const tagIndex of reversed(removeIndex)) {
                            importTransaction.tagIds.splice(tagIndex, 1);
                            importTransaction.originalTagNames.splice(tagIndex, 1);
                        }
                    }

                    if (updated) {
                        updatedCount++;
                        importTransaction.isManuallyAnnotated = true;
                        updateTransactionData(importTransaction);
                        if (type === 'expenseCategory' || type === 'incomeCategory' || type === 'transferCategory') {
                            syncActionableSuggestionDraftState(importTransaction);
                        }
                    }
                }
            }

            if (updatedCount > 0) {
                snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                    count: getDisplayCount(updatedCount)
                });
            }
        });
    }

    function showBatchAddDialog(type: BatchReplaceDialogDataType): void {
        if (isEditing.value) {
            return;
        }

        batchReplaceDialog.value?.open({
            mode: 'batchAdd',
            type: type
        }).then(result => {
            if (!result || !result.targetItem) {
                return;
            }

            let updatedCount = 0;

            if (importTransactions.value.length) {
                for (const importTransaction of importTransactions.value) {
                    if (!importTransaction.selected) {
                        continue;
                    }

                    let updated = false;

                    if (type === 'tag') {
                        let containsTag = false;

                        for (const tagName of importTransaction.originalTagNames) {
                            if (tagName === result.targetItem) {
                                containsTag = true;
                                break;
                            }
                        }

                        if (!containsTag) {
                            if (!importTransaction.tagIds) {
                                importTransaction.tagIds = [];
                            }

                            if (!importTransaction.originalTagNames) {
                                importTransaction.originalTagNames = [];
                            }

                            importTransaction.tagIds.push(result.targetItem);
                            importTransaction.originalTagNames.push(allTagsMap.value[result.targetItem]?.name ?? '');
                            updated = true;
                        }
                    }

                    if (updated) {
                        updatedCount++;
                        importTransaction.isManuallyAnnotated = true;
                        updateTransactionData(importTransaction);
                    }
                }
            }

            if (updatedCount > 0) {
                snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                    count: getDisplayCount(updatedCount)
                });
            }
        });
    }

    function showReplaceInvalidItemDialog(type: BatchReplaceDialogDataType, invalidItems: NameValue[]): void {
        if (isEditing.value) {
            return;
        }

        batchReplaceDialog.value?.open({
            mode: 'replaceInvalidItems',
            type: type,
            invalidItems: invalidItems
        }).then(result => {
            if (!result || (!result.sourceItem && result.sourceItem !== '')) {
                return;
            }

            if (type !== 'tag') {
                if (!result.targetItem) {
                    return;
                }
            }

            let updatedCount = 0;

            if (importTransactions.value.length) {
                for (const importTransaction of importTransactions.value) {
                    if (importTransaction.valid) {
                        continue;
                    }

                    let updated = false;

                    if (type === 'expenseCategory' || type === 'incomeCategory' || type === 'transferCategory') {
                        const categoryId = importTransaction.categoryId;
                        const originalCategoryName = importTransaction.originalCategoryName;

                        if (importTransaction.type !== TransactionType.ModifyBalance && originalCategoryName === result.sourceItem && (!categoryId || categoryId === '0' || !allCategoriesMap.value[categoryId])) {
                            if (type === 'expenseCategory' && importTransaction.type === TransactionType.Expense) {
                                updated = assignCategoryIdIfKnown(importTransaction, result.targetItem);
                            } else if (type === 'incomeCategory' && importTransaction.type === TransactionType.Income) {
                                updated = assignCategoryIdIfKnown(importTransaction, result.targetItem);
                            } else if (type === 'transferCategory' && importTransaction.type === TransactionType.Transfer) {
                                updated = assignCategoryIdIfKnown(importTransaction, result.targetItem);
                            }
                        }
                    } else if (type === 'account') {
                        const sourceAccountId = importTransaction.sourceAccountId;
                        const originalSourceAccountName = importTransaction.originalSourceAccountName;
                        const destinationAccountId = importTransaction.destinationAccountId;
                        const originalDestinationAccountName = importTransaction.originalDestinationAccountName;

                        if (originalSourceAccountName === result.sourceItem && (!sourceAccountId || sourceAccountId === '0' || !allAccountsMap.value[sourceAccountId])) {
                            updated = assignSourceAccountIdIfKnown(importTransaction, result.targetItem) || updated;
                        }

                        if (importTransaction.type === TransactionType.Transfer && originalDestinationAccountName === result.sourceItem && (!destinationAccountId || destinationAccountId === '0' || !allAccountsMap.value[destinationAccountId])) {
                            updated = assignDestinationAccountIdIfKnown(importTransaction, result.targetItem) || updated;
                        }
                    } else if (type === 'tag' && importTransaction.tagIds) {
                        const removeIndex: number[] = [];

                        for (let tagIndex = 0; tagIndex < importTransaction.tagIds.length; tagIndex++) {
                            const tagId = importTransaction.tagIds[tagIndex] as string;
                            const originalTagName = importTransaction.originalTagNames ? (importTransaction.originalTagNames[tagIndex] ?? '') : '';

                            if (originalTagName === result.sourceItem && (!tagId || tagId === '0' || !allTagsMap.value[tagId])) {
                                if (result.targetItem) {
                                    importTransaction.tagIds[tagIndex] = result.targetItem;
                                    importTransaction.originalTagNames[tagIndex] = allTagsMap.value[result.targetItem]?.name || '';
                                } else {
                                    removeIndex.push(tagIndex);
                                }
                                updated = true;
                            }
                        }

                        for (const tagIndex of reversed(removeIndex)) {
                            importTransaction.tagIds.splice(tagIndex, 1);
                            importTransaction.originalTagNames.splice(tagIndex, 1);
                        }
                    }

                    if (updated) {
                        updatedCount++;
                        importTransaction.isManuallyAnnotated = true;
                        updateTransactionData(importTransaction);
                        if (type === 'expenseCategory' || type === 'incomeCategory' || type === 'transferCategory') {
                            syncActionableSuggestionDraftState(importTransaction);
                        }
                    }
                }
            }

            if (updatedCount > 0) {
                snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                    count: getDisplayCount(updatedCount)
                });
            }
        });
    }

    function showReplaceAllTypesDialog(): void {
        if (isEditing.value) {
            return;
        }

        batchReplaceAllTypesDialog.value?.open({
            expenseCategoryNames: allInvalidExpenseCategoryNames.value,
            incomeCategoryNames: allInvalidIncomeCategoryNames.value,
            transferCategoryNames: allInvalidTransferCategoryNames.value,
            accountNames: allInvalidAccountNames.value,
            tagNames: allInvalidTransactionTagNames.value
        }).then(result => {
            if (!result || !result.rules) {
                return;
            }

            let updatedCount = 0;

            if (importTransactions.value.length) {
                for (const importTransaction of importTransactions.value) {
                    let updated = false;

                    for (const rule of result.rules) {
                        if (!rule || !rule.dataType || !rule.targetId) {
                            continue;
                        }

                        if (rule.dataType === 'expenseCategory' || rule.dataType === 'incomeCategory' || rule.dataType === 'transferCategory') {
                            if (importTransaction.type !== TransactionType.ModifyBalance && importTransaction.originalCategoryName === rule.sourceValue) {
                                if (rule.dataType === 'expenseCategory' && importTransaction.type === TransactionType.Expense) {
                                    updated = assignCategoryIdIfKnown(importTransaction, rule.targetId);
                                } else if (rule.dataType === 'incomeCategory' && importTransaction.type === TransactionType.Income) {
                                    updated = assignCategoryIdIfKnown(importTransaction, rule.targetId);
                                } else if (rule.dataType === 'transferCategory' && importTransaction.type === TransactionType.Transfer) {
                                    updated = assignCategoryIdIfKnown(importTransaction, rule.targetId);
                                }
                            }
                        } else if (rule.dataType === 'account') {
                            if (importTransaction.originalSourceAccountName === rule.sourceValue) {
                                updated = assignSourceAccountIdIfKnown(importTransaction, rule.targetId) || updated;
                            }

                            if (importTransaction.type === TransactionType.Transfer && importTransaction.originalDestinationAccountName === rule.sourceValue) {
                                updated = assignDestinationAccountIdIfKnown(importTransaction, rule.targetId) || updated;
                            }
                        } else if (rule.dataType === 'tag' && importTransaction.tagIds) {
                            for (let tagIndex = 0; tagIndex < importTransaction.tagIds.length; tagIndex++) {
                                const originalTagName = importTransaction.originalTagNames ? (importTransaction.originalTagNames[tagIndex] ?? '') : '';

                                if (originalTagName === rule.sourceValue) {
                                    importTransaction.tagIds[tagIndex] = rule.targetId;
                                    updated = true;
                                }
                            }
                        }
                    }

                    if (updated) {
                        updatedCount++;
                        importTransaction.isManuallyAnnotated = true;
                        updateTransactionData(importTransaction);
                        syncActionableSuggestionDraftState(importTransaction);
                    }
                }
            }

            if (updatedCount > 0) {
                snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                    count: getDisplayCount(updatedCount)
                });
            }
        });
    }

    function showBatchCreateInvalidItemDialog(type: BatchCreateDialogDataType, invalidItems: NameValue[]): void {
        if (isEditing.value) {
            return;
        }

        batchCreateDialog.value?.open({
            type: type,
            invalidItems: invalidItems
        }).then(result => {
            if (!result || !result.sourceTargetMap) {
                return;
            }

            let updatedCount = 0;

            if (importTransactions.value.length) {
                const sourceTargetMap: Record<string, string> = result.sourceTargetMap;

                for (const importTransaction of importTransactions.value) {
                    if (importTransaction.valid) {
                        continue;
                    }

                    let updated = false;

                    if (type === 'expenseCategory' || type === 'incomeCategory' || type === 'transferCategory') {
                        const categoryId = importTransaction.categoryId;
                        const originalCategoryName = importTransaction.originalCategoryName;
                        const targetItem = sourceTargetMap[originalCategoryName];

                        if (importTransaction.type !== TransactionType.ModifyBalance && targetItem && (!categoryId || categoryId === '0' || !allCategoriesMap.value[categoryId])) {
                            if (type === 'expenseCategory' && importTransaction.type === TransactionType.Expense) {
                                updated = assignCategoryIdIfKnown(importTransaction, targetItem);
                            } else if (type === 'incomeCategory' && importTransaction.type === TransactionType.Income) {
                                updated = assignCategoryIdIfKnown(importTransaction, targetItem);
                            } else if (type === 'transferCategory' && importTransaction.type === TransactionType.Transfer) {
                                updated = assignCategoryIdIfKnown(importTransaction, targetItem);
                            }
                        }
                    } else if (type === 'tag' && importTransaction.tagIds) {
                        for (let tagIndex = 0; tagIndex < importTransaction.tagIds.length; tagIndex++) {
                            const tagId = importTransaction.tagIds[tagIndex] as string;
                            const originalTagName = importTransaction.originalTagNames ? (importTransaction.originalTagNames[tagIndex] ?? '') : '';
                            const targetItem = sourceTargetMap[originalTagName];

                            if (targetItem && (!tagId || tagId === '0' || !allTagsMap.value[tagId])) {
                                importTransaction.tagIds[tagIndex] = targetItem;
                                updated = true;
                            }
                        }
                    }

                    if (updated) {
                        updatedCount++;
                        importTransaction.isManuallyAnnotated = true;
                        updateTransactionData(importTransaction);
                        if (type === 'expenseCategory' || type === 'incomeCategory' || type === 'transferCategory') {
                            syncActionableSuggestionDraftState(importTransaction);
                        }
                    }
                }
            }

            if (updatedCount > 0) {
                snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                    count: getDisplayCount(updatedCount)
                });
            }
        });
    }

    function convertTransactionType(fromType: TransactionType, toType: TransactionType): void {
        if (!importTransactions.value.length) {
            return;
        }

        const categoryType = transactionTypeToCategoryType(toType);

        if (!categoryType) {
            return;
        }

        const categoryMapByName: Record<string, TransactionCategory> = getSecondaryTransactionMapByName(allCategories.value[categoryType]);

        for (const importTransaction of importTransactions.value) {
            if (!importTransaction.selected || importTransaction.type !== fromType) {
                continue;
            }

            importTransaction.type = toType;
            const convertedCategoryId = categoryMapByName[importTransaction.originalCategoryName]?.id || '';
            if (convertedCategoryId) {
                assignCategoryIdIfKnown(importTransaction, convertedCategoryId);
            } else {
                importTransaction.categoryId = '';
            }

            if (importTransaction.type === TransactionType.Transfer) {
                const destinationAccountId = allAccountsMapByName.value[importTransaction.originalDestinationAccountName || '']?.id || '';
                if (!assignDestinationAccountIdIfKnown(importTransaction, destinationAccountId)) {
                    importTransaction.destinationAccountId = '';
                }
                importTransaction.destinationAmountCents = importTransaction.sourceAmountCents;
            } else {
                if (fromType === TransactionType.Transfer && toType === TransactionType.Income) {
                    const transferDestinationAccountId = importTransaction.destinationAccountId;
                    if (!assignSourceAccountIdIfKnown(importTransaction, transferDestinationAccountId)) {
                        importTransaction.sourceAccountId = '';
                    }
                    importTransaction.sourceAmountCents = importTransaction.destinationAmountCents;
                }

                importTransaction.destinationAccountId = '';
                importTransaction.destinationAmountCents = 0;
            }

            importTransaction.isManuallyAnnotated = true;
            updateTransactionData(importTransaction);
            syncActionableSuggestionDraftState(importTransaction);
        }
    }

    function clearSelectedRecurringMatches(): void {
        if (!importTransactions.value.length) {
            return;
        }

        let updatedCount = 0;
        for (const importTransaction of importTransactions.value) {
            if (!importTransaction.selected || !importTransaction.hasRecurringMatch()) {
                continue;
            }

            importTransaction.clearRecurringMatch(false);
            updateTransactionData(importTransaction);
            syncActionableSuggestionDraftState(importTransaction);
            updatedCount++;
        }

        if (updatedCount > 0) {
            snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                count: getDisplayCount(updatedCount)
            });
        }
    }

    return {
        clearSelectedRecurringMatches,
        convertTransactionType,
        showBatchAddDialog,
        showBatchCreateInvalidItemDialog,
        showBatchReplaceDialog,
        showReplaceAllTypesDialog,
        showReplaceInvalidItemDialog
    };
}
