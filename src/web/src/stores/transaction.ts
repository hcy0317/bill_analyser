import { ref, computed } from 'vue';
import { defineStore } from 'pinia';

import { useSettingsStore } from './setting.ts';
import { useUserStore } from './user.ts';
import { useAccountsStore } from './account.ts';
import { useTransactionCategoriesStore } from './transactionCategory.ts';
import { useOverviewStore } from './overview.ts';
import { useStatisticsStore } from './statistics.ts';
import { useExchangeRatesStore } from './exchangeRates.ts';

import { type BeforeResolveFunction, itemAndIndex, entries, keys } from '@/core/base.ts';
import { DateRange } from '@/core/datetime.ts';
import { CategoryType } from '@/core/category.ts';
import { TransactionType, TransactionTagFilterType } from '@/core/transaction.ts';
import { TRANSACTION_MIN_AMOUNT, TRANSACTION_MAX_AMOUNT } from '@/consts/transaction.ts';
import {
    type TransactionDraft,
    type TransactionInfoResponse,
    type TransactionImportRequest,
    type TransactionPageWrapper,
    type TransactionReconciliationStatementResponse,
    Transaction
} from '@/models/transaction.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import type {
    TransactionPictureInfoBasicResponse
} from '@/models/transaction_picture_info.ts';
import {
    type ImportTransactionResponsePageWrapper
} from '@/models/imported_transaction.ts';
import {
    type ExportTransactionDataRequest
} from '@/models/data_management.ts';
import type {
    RecognizedReceiptImageResponse,
    ReceiptImageErrorCode
} from '@/models/large_language_model.ts';

import {
    getUserTransactionDraft,
    updateUserTransactionDraft,
    clearUserTransactionDraft
} from '@/lib/userstate.ts';
import {
    isDefined,
    isNumber,
    isString,
    isArray1SubsetOfArray2,
    splitItemsToMap,
    countSplitItems
} from '@/lib/common.ts';
import {
    getTimezoneOffsetMinutes,
    getBrowserTimezoneOffsetMinutes,
    getActualUnixTimeForStore,
    parseDateTimeFromUnixTime
} from '@/lib/datetime.ts';
import { getAmountWithDecimalNumberCount } from '@/lib/numeral.ts';
import { getCurrencyFraction } from '@/lib/currency.ts';
import { getFirstAvailableCategoryId } from '@/lib/category.ts';
import services, { type ApiResponsePromise } from '@/lib/services.ts';
import logger from '@/lib/logger.ts';
import type {
    TransactionListPartialFilter,
    TransactionListFilter,
    TransactionMonthList,
    TransactionTotalAmountCents
} from './transaction/types.ts';
import {
    buildRecognizeReceiptImageError,
    mapReceiptImageErrorCode,
    normalizeReceiptTransactionDraft
} from './transaction/receiptDraft.ts';
import {
    createTransactionListRequestCoordinator,
    type LoadMonthlyTransactionsOptions,
    type LoadTransactionsOptions
} from './transaction/listRequestCoordinator.ts';

export type {
    TransactionListPartialFilter,
    TransactionListFilter,
    TransactionMonthList,
    TransactionTotalAmountCents
} from './transaction/types.ts';
export {
    buildRecognizeReceiptImageError,
    mapReceiptImageErrorCode
} from './transaction/receiptDraft.ts';

/** 正式交易 store facade，集中暴露列表、CRUD、批量操作、导出、对账和图片 OCR 动作。 */
export const useTransactionsStore = defineStore('transactions', () => {
    const settingsStore = useSettingsStore();
    const userStore = useUserStore();
    const accountsStore = useAccountsStore();
    const transactionCategoriesStore = useTransactionCategoriesStore();
    const overviewStore = useOverviewStore();
    const statisticsStore = useStatisticsStore();
    const exchangeRatesStore = useExchangeRatesStore();

    const transactionDraft = ref<TransactionDraft | null>(getUserTransactionDraft());

    const transactionsFilter = ref<TransactionListFilter>({
        dateType: DateRange.All.type,
        maxTime: 0,
        minTime: 0,
        type: 0,
        categoryIds: '',
        accountIds: '',
        tagIds: '',
        tagFilterType: TransactionTagFilterType.Default.type,
        amountFilterCents: '',
        keyword: ''
    });

    const transactions = ref<TransactionMonthList[]>([]);
    const transactionsNextTimeId = ref<number>(0);
    const transactionListStateInvalid = ref<boolean>(true);
    const transactionReconciliationStatementStateInvalid = ref<boolean>(true);
    const transactionListRequestCoordinator = createTransactionListRequestCoordinator({
        getFilter: () => transactionsFilter.value,
        getNextTimeId: () => transactionsNextTimeId.value,
        expandAccountIds: accountIds => accountsStore.expandAccountIds(accountIds),
        applyPage: loadTransactionList,
        isListInvalid: () => transactionListStateInvalid.value,
        updateListInvalidState: updateTransactionListInvalidState
    });

    const allFilterCategoryIds = computed<Record<string, boolean>>(() => splitItemsToMap(transactionsFilter.value.categoryIds, ','));
    const allFilterAccountIds = computed<Record<string, boolean>>(() => splitItemsToMap(transactionsFilter.value.accountIds, ','));
    const allFilterTagIds = computed<Record<string, boolean>>(() => splitItemsToMap(transactionsFilter.value.tagIds, ','));

    const allFilterCategoryIdsCount = computed<number>(() => countSplitItems(transactionsFilter.value.categoryIds, ','));
    const allFilterAccountIdsCount = computed<number>(() => countSplitItems(transactionsFilter.value.accountIds, ','));
    const allFilterTagIdsCount = computed<number>(() => countSplitItems(transactionsFilter.value.tagIds, ','));

    const noTransaction = computed<boolean>(() => {
        for (const transactionMonthList of transactions.value) {
            for (const transaction of transactionMonthList.items) {
                if (transaction) {
                    return false;
                }
            }
        }

        return true;
    });

    const hasMoreTransaction = computed<boolean>(() => {
        return transactionsNextTimeId.value > 0;
    });

    function loadTransactionList({ transactionPageWrapper, reload, autoExpand, defaultCurrency, nextTimeSequenceId }: { transactionPageWrapper: TransactionPageWrapper, reload: boolean, autoExpand: boolean, defaultCurrency: string, nextTimeSequenceId?: number }): void {
        if (reload) {
            transactions.value = [];
        }

        if (transactionPageWrapper.items && transactionPageWrapper.items.length) {
            const currentUtcOffset = getTimezoneOffsetMinutes(settingsStore.appSettings.timeZone);
            let currentMonthListIndex = -1;
            let currentMonthList: TransactionMonthList | null = null;

            for (const [item, index] of itemAndIndex(transactionPageWrapper.items)) {
                fillTransactionObject(item);

                const transactionTime = parseDateTimeFromUnixTime(item.time, item.utcOffset, currentUtcOffset);
                const transactionYear = transactionTime.getGregorianCalendarYear();
                const transactionMonth = transactionTime.getGregorianCalendarMonth();
                const transactionYearDashMonth = transactionTime.getGregorianCalendarYearDashMonth();

                if (index === 0 && transactions.value.length > 0) {
                    const lastMonthList = transactions.value[transactions.value.length - 1] as TransactionMonthList;

                    if (lastMonthList.totalAmountCents.incompleteExpense || lastMonthList.totalAmountCents.incompleteIncome) {
                        // calculate the total amount of last month which has incomplete total amount before starting to process a new request
                        calculateMonthTotalAmount(lastMonthList, defaultCurrency, transactionsFilter.value.accountIds, false);
                    }
                }

                if (currentMonthList && currentMonthList.year === transactionYear && currentMonthList.month === transactionMonth) {
                    currentMonthList.items.push(Object.freeze(item));

                    if (index === transactionPageWrapper.items.length - 1) {
                        // calculate the total amount of current month when processing the last transaction item of this request
                        calculateMonthTotalAmount(currentMonthList, defaultCurrency, transactionsFilter.value.accountIds, true);
                    }
                    continue;
                }

                for (let j = currentMonthListIndex + 1; j < transactions.value.length; j++) {
                    if (transactions.value[j]!.year === transactionYear && transactions.value[j]!.month === transactionMonth) {
                        currentMonthListIndex = j;
                        currentMonthList = transactions.value[j] as TransactionMonthList;
                        break;
                    }
                }

                if (!currentMonthList || currentMonthList.year !== transactionYear || currentMonthList.month !== transactionMonth) {
                    // calculate the total amount of current month when processing the first transaction item of the next month
                    calculateMonthTotalAmount(currentMonthList, defaultCurrency, transactionsFilter.value.accountIds, false);

                    const monthList: TransactionMonthList = {
                        year: transactionYear,
                        month: transactionMonth,
                        yearDashMonth: transactionYearDashMonth,
                        opened: autoExpand,
                        items: [],
                        totalAmountCents: {
                            expenseCents: 0,
                            incompleteExpense: true,
                            incomeCents: 0,
                            incompleteIncome: true
                        },
                        dailyTotalAmountsCents: {}
                    };

                    transactions.value.push(monthList);

                    currentMonthListIndex = transactions.value.length - 1;
                    currentMonthList = transactions.value[transactions.value.length - 1] as TransactionMonthList;
                }

                currentMonthList.items.push(Object.freeze(item));
                // init the total amount struct of current month when processing the first transaction item of current month
                calculateMonthTotalAmount(currentMonthList, defaultCurrency, transactionsFilter.value.accountIds, true);
            }
        }

        if (nextTimeSequenceId) {
            transactionsNextTimeId.value = nextTimeSequenceId;
        } else {
            calculateMonthTotalAmount(transactions.value[transactions.value.length - 1] as TransactionMonthList, defaultCurrency, transactionsFilter.value.accountIds, false);
            transactionsNextTimeId.value = -1;
        }
    }

    function updateTransactionInTransactionList({ currentTransaction, defaultCurrency }: { currentTransaction: Transaction, defaultCurrency: string }): void {
        const currentUtcOffset = getTimezoneOffsetMinutes(settingsStore.appSettings.timeZone);
        const transactionTime = parseDateTimeFromUnixTime(currentTransaction.time, currentTransaction.utcOffset, currentUtcOffset);
        const transactionYear = transactionTime.getGregorianCalendarYear();
        const transactionMonth = transactionTime.getGregorianCalendarMonth();

        for (const [transactionMonthList, monthIndex] of itemAndIndex(transactions.value)) {
            if (!transactionMonthList.items) {
                continue;
            }

            for (const [transaction, transactionIndex] of itemAndIndex(transactionMonthList.items)) {
                if (transaction.id === currentTransaction.id) {
                    fillTransactionObject(currentTransaction);

                    if (transactionYear !== transactionMonthList.year ||
                        transactionMonth !== transactionMonthList.month ||
                        currentTransaction.gregorianCalendarDayOfMonth !== transaction.gregorianCalendarDayOfMonth) {
                        transactionListStateInvalid.value = true;
                        return;
                    }

                    if ((transactionsFilter.value.categoryIds && !allFilterCategoryIds.value[currentTransaction.categoryId]) ||
                        (transactionsFilter.value.accountIds && !allFilterAccountIds.value[currentTransaction.sourceAccountId] && !allFilterAccountIds.value[currentTransaction.destinationAccountId] &&
                            (!currentTransaction.sourceAccount || !allFilterAccountIds.value[currentTransaction.sourceAccount.parentId]) &&
                            (!currentTransaction.destinationAccount || !allFilterAccountIds.value[currentTransaction.destinationAccount.parentId])
                        )
                    ) {
                        transactionMonthList.items.splice(transactionIndex, 1);
                    } else {
                        transactionMonthList.items.splice(transactionIndex, 1, currentTransaction);
                    }

                    if (transactionMonthList.items.length < 1) {
                        transactions.value.splice(monthIndex, 1);
                    } else {
                        calculateMonthTotalAmount(transactionMonthList, defaultCurrency, transactionsFilter.value.accountIds, monthIndex >= transactions.value.length - 1 && transactionsNextTimeId.value > 0);
                    }

                    return;
                }
            }
        }
    }

    function removeTransactionFromTransactionList({ currentTransaction, defaultCurrency }: { currentTransaction: TransactionInfoResponse, defaultCurrency: string }): void {
        for (const [transactionMonthList, monthIndex] of itemAndIndex(transactions.value)) {
            if (!transactionMonthList.items ||
                transactionMonthList.items[0]!.time < currentTransaction.time ||
                transactionMonthList.items[transactionMonthList.items.length - 1]!.time > currentTransaction.time) {
                continue;
            }

            for (const [transaction, transactionIndex] of itemAndIndex(transactionMonthList.items)) {
                if (transaction.id === currentTransaction.id) {
                    transactionMonthList.items.splice(transactionIndex, 1);
                }
            }

            if (transactionMonthList.items.length < 1) {
                transactions.value.splice(monthIndex, 1);
            } else {
                calculateMonthTotalAmount(transactionMonthList, defaultCurrency, transactionsFilter.value.accountIds, monthIndex >= transactions.value.length - 1 && transactionsNextTimeId.value > 0);
            }
        }
    }

    function calculateMonthTotalAmount(transactionMonthList: TransactionMonthList | null, defaultCurrency: string, accountIds: string, incomplete: boolean): void {
        if (!transactionMonthList) {
            return;
        }

        let totalExpense = 0;
        let totalIncome = 0;
        let hasUnCalculatedTotalExpense = false;
        let hasUnCalculatedTotalIncome = false;
        const dailyTotalAmountsCents: Record<string, TransactionTotalAmountCents> = {};

        const allAccountIdsMap: Record<string, boolean> = {};
        let totalAccountIdsCount = 0;

        if (accountIds && accountIds !== '0') {
            const allAccountIdsArray = accountIds.split(',');

            for (const accountId of allAccountIdsArray) {
                if (accountId) {
                    allAccountIdsMap[accountId] = true;
                    totalAccountIdsCount++;
                }
            }
        }

        for (const transaction of transactionMonthList.items) {
            const transactionDay = isNumber(transaction.gregorianCalendarDayOfMonth) ? transaction.gregorianCalendarDayOfMonth.toString() : '0';
            let dailyTotalAmount = dailyTotalAmountsCents[transactionDay];

            if (!dailyTotalAmount) {
                dailyTotalAmount = {
                    expenseCents: 0,
                    incompleteExpense: false,
                    incomeCents: 0,
                    incompleteIncome: false
                };
                dailyTotalAmountsCents[transactionDay] = dailyTotalAmount;
            }

            let amount = transaction.sourceAmountCents;
            let account = transaction.sourceAccount;

            if (totalAccountIdsCount > 0 && transaction.destinationAccount
                && (!allAccountIdsMap[transaction.sourceAccount?.id || ''] && !allAccountIdsMap[transaction.sourceAccount?.parentId || ''])
                && (allAccountIdsMap[transaction.destinationAccount.id] || allAccountIdsMap[transaction.destinationAccount.parentId])) {
                amount = transaction.destinationAmountCents;
                account = transaction.destinationAccount;
            }

            if (!account) {
                continue;
            }

            if (account.currency !== defaultCurrency) {
                const balance = exchangeRatesStore.getExchangedAmount(amount, account.currency, defaultCurrency);

                if (!isNumber(balance)) {
                    if (transaction.type === TransactionType.Expense) {
                        hasUnCalculatedTotalExpense = true;
                        dailyTotalAmount.incompleteExpense = true;
                    } else if (transaction.type === TransactionType.Income) {
                        hasUnCalculatedTotalIncome = true;
                        dailyTotalAmount.incompleteIncome = true;
                    }

                    continue;
                }

                amount = balance;
            }

            if (transaction.type === TransactionType.Expense) {
                totalExpense += amount;
                dailyTotalAmount.expenseCents += amount;
            } else if (transaction.type === TransactionType.Income) {
                totalIncome += amount;
                dailyTotalAmount.incomeCents += amount;
            } else if (transaction.type === TransactionType.Transfer && totalAccountIdsCount > 0) {
                if (allAccountIdsMap[transaction.sourceAccountId] && allAccountIdsMap[transaction.destinationAccountId]) {
                    // 不做处理
                } else if (transaction.sourceAccount && transaction.destinationAccount && allAccountIdsMap[transaction.sourceAccount.parentId] && allAccountIdsMap[transaction.destinationAccount.parentId]) {
                    // 不做处理
                } else if (transaction.sourceAccount && allAccountIdsMap[transaction.sourceAccount.parentId] && allAccountIdsMap[transaction.destinationAccountId]) {
                    // 不做处理
                } else if (transaction.destinationAccount && allAccountIdsMap[transaction.sourceAccountId] && allAccountIdsMap[transaction.destinationAccount.parentId]) {
                    // 不做处理
                } else if (allAccountIdsMap[transaction.sourceAccountId] || (transaction.sourceAccount && allAccountIdsMap[transaction.sourceAccount.parentId])) {
                    totalExpense += amount;
                    dailyTotalAmount.expenseCents += amount;
                } else if (allAccountIdsMap[transaction.destinationAccountId] || (transaction.destinationAccount && allAccountIdsMap[transaction.destinationAccount.parentId])) {
                    totalIncome += amount;
                    dailyTotalAmount.incomeCents += amount;
                }
            }
        }

        transactionMonthList.totalAmountCents.expenseCents = Math.trunc(totalExpense);
        transactionMonthList.totalAmountCents.incompleteExpense = incomplete || hasUnCalculatedTotalExpense;
        transactionMonthList.totalAmountCents.incomeCents = Math.trunc(totalIncome);
        transactionMonthList.totalAmountCents.incompleteIncome = incomplete || hasUnCalculatedTotalIncome;

        for (const day of keys(transactionMonthList.dailyTotalAmountsCents)) {
            delete transactionMonthList.dailyTotalAmountsCents[day];
        }

        for (const [day, dailyTotalAmount] of entries(dailyTotalAmountsCents)) {
            transactionMonthList.dailyTotalAmountsCents[day] = {
                expenseCents: Math.trunc(dailyTotalAmount.expenseCents),
                incompleteExpense: incomplete || dailyTotalAmount.incompleteExpense,
                incomeCents: Math.trunc(dailyTotalAmount.incomeCents),
                incompleteIncome: incomplete || dailyTotalAmount.incompleteIncome
            };
        }
    }

    function fillTransactionObject(transaction: Transaction): void {
        if (!transaction.category) {
            if (transactionCategoriesStore.allTransactionCategoriesMap[transaction.categoryId]) {
                transaction.setCategory(transactionCategoriesStore.allTransactionCategoriesMap[transaction.categoryId]);
            } else {
                // 若未找到分类，则回退到默认分类
                const defaultCategory = TransactionCategory.createNewCategory(transaction.type);
                defaultCategory.id = transaction.categoryId;
                defaultCategory.name = 'Unknown';
                transaction.setCategory(defaultCategory);
            }
        }

        if (!transaction.sourceAccount && accountsStore.allAccountsMap[transaction.sourceAccountId]) {
            transaction.setSourceAccount(accountsStore.allAccountsMap[transaction.sourceAccountId]);
        }

        if (!transaction.destinationAccount && accountsStore.allAccountsMap[transaction.destinationAccountId]) {
            transaction.setDestinationAccount(accountsStore.allAccountsMap[transaction.destinationAccountId]);
        }
    }

    function initTransactionDraft(): void {
        if (settingsStore.appSettings.autoSaveTransactionDraft === 'enabled' || settingsStore.appSettings.autoSaveTransactionDraft === 'confirmation') {
            transactionDraft.value = getUserTransactionDraft();
        } else {
            transactionDraft.value = null;
        }
    }

    function isTransactionDraftModified(transaction?: Transaction, initAmount?: number, initCategoryId?: string, initAccountId?: string, initTagIds?: string, firstVisibleAccountId?: string): boolean {
        if (!transaction) {
            return false;
        }

        if (transaction.sourceAmountCents !== 0 && transaction.sourceAmountCents !== initAmount) {
            return true;
        }

        if (transaction.type === TransactionType.Transfer && transaction.destinationAmountCents !== 0) {
            return true;
        }

        if (transaction.sourceAccountId && transaction.sourceAccountId !== '0' && transaction.sourceAccountId !== userStore.currentUserDefaultAccountId && ((userStore.currentUserDefaultAccountId !== '' && userStore.currentUserDefaultAccountId !== '0') || transaction.sourceAccountId !== firstVisibleAccountId) && transaction.sourceAccountId !== initAccountId) {
            return true;
        }

        if (transaction.type === TransactionType.Transfer && transaction.destinationAccountId && transaction.destinationAccountId !== '0' && transaction.destinationAccountId !== userStore.currentUserDefaultAccountId && transaction.destinationAccountId !== initAccountId) {
            return true;
        }

        const allCategories = transactionCategoriesStore.allTransactionCategories;

        if (allCategories) {
            if (transaction.type === TransactionType.Expense) {
                const defaultCategoryId = getFirstAvailableCategoryId(allCategories[CategoryType.Expense]);

                if (transaction.expenseCategoryId && transaction.expenseCategoryId !== '0' && transaction.expenseCategoryId !== defaultCategoryId && transaction.expenseCategoryId !== initCategoryId) {
                    return true;
                }
            } else if (transaction.type === TransactionType.Income) {
                const defaultCategoryId = getFirstAvailableCategoryId(allCategories[CategoryType.Income]);

                if (transaction.incomeCategoryId && transaction.incomeCategoryId !== '0' && transaction.incomeCategoryId !== defaultCategoryId && transaction.incomeCategoryId !== initCategoryId) {
                    return true;
                }
            } else if (transaction.type === TransactionType.Transfer) {
                const defaultCategoryId = getFirstAvailableCategoryId(allCategories[CategoryType.Transfer]);

                if (transaction.transferCategoryId && transaction.transferCategoryId !== '0' && transaction.transferCategoryId !== defaultCategoryId && transaction.transferCategoryId !== initCategoryId) {
                    return true;
                }
            }
        }

        if (transaction.hideAmount) {
            return true;
        }

        if (transaction.tagIds && transaction.tagIds.length > 0) {
            return !initTagIds || !isArray1SubsetOfArray2(transaction.tagIds, initTagIds.split(','));
        }

        if (transaction.pictures && transaction.pictures.length > 0) {
            return true;
        }

        if (transaction.comment && transaction.comment.trim()) {
            return true;
        }

        return false;
    }

    function saveTransactionDraft(transaction?: Transaction, initAmount?: number, initCategoryId?: string, initAccountId?: string, initTagIds?: string, firstVisibleAccountId?: string): void {
        if (settingsStore.appSettings.autoSaveTransactionDraft !== 'enabled' && settingsStore.appSettings.autoSaveTransactionDraft !== 'confirmation') {
            clearTransactionDraft();
            return;
        }

        if (transaction) {
            if (!isTransactionDraftModified(transaction, initAmount, initCategoryId, initAccountId, initTagIds, firstVisibleAccountId)) {
                clearTransactionDraft();
                return;
            }

            transactionDraft.value = transaction.toTransactionDraft();
        }

        updateUserTransactionDraft(transactionDraft.value);
    }

    function clearTransactionDraft(): void {
        transactionDraft.value = null;
        clearUserTransactionDraft();
    }

    function setTransactionSuitableDestinationAmount(transaction: Transaction, oldValue: number, newValue: number): void {
        if (transaction.type === TransactionType.Expense || transaction.type === TransactionType.Income) {
            transaction.destinationAmountCents = newValue;
        } else if (transaction.type === TransactionType.Transfer) {
            const sourceAccount = accountsStore.allAccountsMap[transaction.sourceAccountId];
            const destinationAccount = accountsStore.allAccountsMap[transaction.destinationAccountId];

            if (sourceAccount && destinationAccount && sourceAccount.currency !== destinationAccount.currency) {
                const decimalNumberCount = getCurrencyFraction(destinationAccount.currency);
                const exchangedOldValue = exchangeRatesStore.getExchangedAmount(oldValue, sourceAccount.currency, destinationAccount.currency);
                const exchangedNewValue = exchangeRatesStore.getExchangedAmount(newValue, sourceAccount.currency, destinationAccount.currency);

                if (isNumber(decimalNumberCount) && isNumber(exchangedOldValue)) {
                    oldValue = Math.trunc(exchangedOldValue);
                    oldValue = getAmountWithDecimalNumberCount(oldValue, decimalNumberCount);
                }

                if (isNumber(decimalNumberCount) && isNumber(exchangedNewValue)) {
                    newValue = Math.trunc(exchangedNewValue);
                    newValue = getAmountWithDecimalNumberCount(newValue, decimalNumberCount);
                } else {
                    return;
                }
            }

            if ((!sourceAccount || !destinationAccount || transaction.destinationAmountCents === oldValue || transaction.destinationAmountCents === 0) &&
                (TRANSACTION_MIN_AMOUNT <= newValue && newValue <= TRANSACTION_MAX_AMOUNT)) {
                transaction.destinationAmountCents = newValue;
            }
        }
    }

    function updateTransactionListInvalidState(invalidState: boolean): void {
        transactionListStateInvalid.value = invalidState;
    }

    function updateTransactionReconciliationStatementInvalidState(invalidState: boolean): void {
        transactionReconciliationStatementStateInvalid.value = invalidState;
    }

    function resetTransactions(): void {
        transactionListRequestCoordinator.reset();
        transactionsFilter.value.dateType = DateRange.All.type;
        transactionsFilter.value.maxTime = 0;
        transactionsFilter.value.minTime = 0;
        transactionsFilter.value.type = 0;
        transactionsFilter.value.categoryIds = '';
        transactionsFilter.value.accountIds = '';
        transactionsFilter.value.tagIds = '';
        transactionsFilter.value.tagFilterType = TransactionTagFilterType.Default.type;
        transactionsFilter.value.amountFilterCents = '';
        transactionsFilter.value.keyword = '';
        transactions.value = [];
        transactionsNextTimeId.value = 0;
        transactionListStateInvalid.value = true;
        transactionReconciliationStatementStateInvalid.value = true;
    }

    function clearTransactions(): void {
        transactionListRequestCoordinator.reset();
        transactions.value = [];
        transactionsNextTimeId.value = 0;
        transactionListStateInvalid.value = true;
    }

    function initTransactionListFilter(filter: TransactionListPartialFilter): void {
        transactionListRequestCoordinator.invalidate();
        if (filter && isNumber(filter.dateType)) {
            transactionsFilter.value.dateType = filter.dateType;
        } else {
            transactionsFilter.value.dateType = DateRange.All.type;
        }

        if (filter && isNumber(filter.maxTime)) {
            transactionsFilter.value.maxTime = filter.maxTime;
        } else {
            transactionsFilter.value.maxTime = 0;
        }

        if (filter && isNumber(filter.minTime)) {
            transactionsFilter.value.minTime = filter.minTime;
        } else {
            transactionsFilter.value.minTime = 0;
        }

        if (filter && isNumber(filter.type)) {
            transactionsFilter.value.type = filter.type;
        } else {
            transactionsFilter.value.type = 0;
        }

        if (filter && isString(filter.categoryIds)) {
            transactionsFilter.value.categoryIds = filter.categoryIds;
        } else {
            transactionsFilter.value.categoryIds = '';
        }

        if (filter && isString(filter.accountIds)) {
            transactionsFilter.value.accountIds = filter.accountIds;
        } else {
            transactionsFilter.value.accountIds = '';
        }

        if (filter && isString(filter.tagIds)) {
            transactionsFilter.value.tagIds = filter.tagIds;
        } else {
            transactionsFilter.value.tagIds = '';
        }

        if (filter && isNumber(filter.tagFilterType)) {
            transactionsFilter.value.tagFilterType = filter.tagFilterType;
        } else {
            transactionsFilter.value.tagFilterType = TransactionTagFilterType.Default.type;
        }

        if (filter && isString(filter.amountFilterCents)) {
            transactionsFilter.value.amountFilterCents = filter.amountFilterCents;
        } else {
            transactionsFilter.value.amountFilterCents = '';
        }

        if (filter && isString(filter.keyword)) {
            transactionsFilter.value.keyword = filter.keyword;
        } else {
            transactionsFilter.value.keyword = '';
        }
    }

    function updateTransactionListFilter(filter: TransactionListPartialFilter): boolean {
        let changed = false;

        if (filter && isNumber(filter.dateType) && transactionsFilter.value.dateType !== filter.dateType) {
            transactionsFilter.value.dateType = filter.dateType;
            changed = true;
        }

        if (filter && isNumber(filter.maxTime) && transactionsFilter.value.maxTime !== filter.maxTime) {
            transactionsFilter.value.maxTime = filter.maxTime;
            changed = true;
        }

        if (filter && isNumber(filter.minTime) && transactionsFilter.value.minTime !== filter.minTime) {
            transactionsFilter.value.minTime = filter.minTime;
            changed = true;
        }

        if (filter && isNumber(filter.type) && transactionsFilter.value.type !== filter.type) {
            transactionsFilter.value.type = filter.type;
            changed = true;
        }

        if (filter && isString(filter.categoryIds) && transactionsFilter.value.categoryIds !== filter.categoryIds) {
            transactionsFilter.value.categoryIds = filter.categoryIds;
            changed = true;
        }

        if (filter && isString(filter.accountIds) && transactionsFilter.value.accountIds !== filter.accountIds) {
            if (DateRange.isBillingCycle(transactionsFilter.value.dateType) &&
                (!accountsStore.getAccountStatementDate(filter.accountIds) || accountsStore.getAccountStatementDate(filter.accountIds) !== accountsStore.getAccountStatementDate(transactionsFilter.value.accountIds))) {
                transactionsFilter.value.dateType = DateRange.Custom.type;
            }

            transactionsFilter.value.accountIds = filter.accountIds;
            changed = true;
        }

        if (filter && isString(filter.tagIds) && transactionsFilter.value.tagIds !== filter.tagIds) {
            transactionsFilter.value.tagIds = filter.tagIds;
            changed = true;
        }

        if (filter && isNumber(filter.tagFilterType) && transactionsFilter.value.tagFilterType !== filter.tagFilterType) {
            transactionsFilter.value.tagFilterType = filter.tagFilterType;
            changed = true;
        }

        if (filter && isString(filter.amountFilterCents) && transactionsFilter.value.amountFilterCents !== filter.amountFilterCents) {
            transactionsFilter.value.amountFilterCents = filter.amountFilterCents;
            changed = true;
        }

        if (filter && isString(filter.keyword) && transactionsFilter.value.keyword !== filter.keyword) {
            transactionsFilter.value.keyword = filter.keyword;
            changed = true;
        }

        if (changed) {
            transactionListRequestCoordinator.invalidate();
        }

        return changed;
    }

    function getTransactionListPageParams(pageType: number): string {
        const querys: string[] = [];

        querys.push('pageType=' + pageType);

        if (transactionsFilter.value.type) {
            querys.push('type=' + transactionsFilter.value.type);
        }

        if (transactionsFilter.value.accountIds) {
            querys.push('accountIds=' + transactionsFilter.value.accountIds);
        }

        if (transactionsFilter.value.categoryIds) {
            querys.push('categoryIds=' + transactionsFilter.value.categoryIds);
        }

        if (transactionsFilter.value.tagIds) {
            querys.push('tagIds=' + transactionsFilter.value.tagIds);
        }

        if (transactionsFilter.value.tagFilterType) {
            querys.push('tagFilterType=' + transactionsFilter.value.tagFilterType);
        }

        querys.push('dateType=' + transactionsFilter.value.dateType);

        if (DateRange.isBillingCycle(transactionsFilter.value.dateType) || transactionsFilter.value.dateType === DateRange.Custom.type) {
            querys.push('maxTime=' + transactionsFilter.value.maxTime);
            querys.push('minTime=' + transactionsFilter.value.minTime);
        }

        if (transactionsFilter.value.amountFilterCents) {
            querys.push('amountFilterCents=' + encodeURIComponent(transactionsFilter.value.amountFilterCents));
        }

        if (transactionsFilter.value.keyword) {
            querys.push('keyword=' + encodeURIComponent(transactionsFilter.value.keyword));
        }

        return querys.join('&');
    }

    function getExportTransactionDataRequestByTransactionFilter(): ExportTransactionDataRequest {
        return {
            maxTime: transactionsFilter.value.maxTime,
            minTime: transactionsFilter.value.minTime,
            type: transactionsFilter.value.type,
            categoryIds: transactionsFilter.value.categoryIds,
            accountIds: transactionsFilter.value.accountIds,
            tagIds: transactionsFilter.value.tagIds,
            tagFilterType: transactionsFilter.value.tagFilterType,
            amountFilterCents: transactionsFilter.value.amountFilterCents,
            keyword: transactionsFilter.value.keyword
        };
    }

    function loadTransactions(options: LoadTransactionsOptions): Promise<TransactionPageWrapper> {
        return transactionListRequestCoordinator.loadTransactions(options);
    }

    function loadMonthlyAllTransactions(options: LoadMonthlyTransactionsOptions): Promise<TransactionPageWrapper> {
        return transactionListRequestCoordinator.loadMonthlyAllTransactions(options);
    }

    function getReconciliationStatements({ accountId, startTime, endTime }: { accountId: string, startTime: number, endTime: number }): Promise<TransactionReconciliationStatementResponse> {
        return new Promise((resolve, reject) => {
            services.getReconciliationStatements({
                accountId: accountId,
                startTime: startTime,
                endTime: endTime
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    if (!transactionReconciliationStatementStateInvalid.value) {
                        updateTransactionReconciliationStatementInvalidState(true);
                    }

                    reject({ message: 'Unable to retrieve reconciliation statements' });
                    return;
                }

                if (transactionReconciliationStatementStateInvalid.value) {
                    updateTransactionReconciliationStatementInvalidState(false);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to load reconciliation statements', error);

                if (!transactionReconciliationStatementStateInvalid.value) {
                    updateTransactionReconciliationStatementInvalidState(true);
                }

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve reconciliation statements' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function getTransaction({ transactionId, withPictures }: { transactionId: string, withPictures?: boolean }): Promise<Transaction> {
        return new Promise((resolve, reject) => {
            if (!isDefined(withPictures)) {
                withPictures = true;
            }

            services.getTransaction({
                id: transactionId,
                withPictures: withPictures
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to retrieve transaction' });
                    return;
                }

                const transaction = Transaction.of(data.result);

                resolve(transaction);
            }).catch(error => {
                logger.error('failed to load transaction info', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve transaction' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function saveTransaction({ transaction, defaultCurrency, isEdit, clientSessionId }: { transaction: Transaction, defaultCurrency: string, isEdit: boolean, clientSessionId: string }): Promise<Transaction> {
        return new Promise((resolve, reject) => {
            const actualTime = getActualUnixTimeForStore(transaction.time, transaction.utcOffset, getBrowserTimezoneOffsetMinutes());
            let promise: ApiResponsePromise<TransactionInfoResponse>;

            if (transaction.type !== TransactionType.Expense &&
                transaction.type !== TransactionType.Income &&
                transaction.type !== TransactionType.Transfer &&
                transaction.type !== TransactionType.Investment &&
                transaction.type !== TransactionType.ModifyBalance) {
                reject({ message: 'An error occurred' });
                return;
            } else if (!isEdit && transaction.type === TransactionType.ModifyBalance) {
                reject({ message: 'An error occurred' });
                return;
            }

            if (!isEdit) {
                promise = services.addTransaction(transaction.toCreateRequest(clientSessionId, actualTime));
            } else {
                promise = services.modifyTransaction(transaction.toModifyRequest(actualTime));
            }

            promise.then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    if (!isEdit) {
                        reject({ message: 'Unable to add transaction' });
                    } else {
                        reject({ message: 'Unable to save transaction' });
                    }
                }

                const transaction = Transaction.of(data.result);

                if (!isEdit) {
                    if (!transactionListStateInvalid.value) {
                        updateTransactionListInvalidState(true);
                    }
                } else {
                    updateTransactionInTransactionList({
                        currentTransaction: transaction,
                        defaultCurrency: defaultCurrency
                    });
                }

                if (!transactionReconciliationStatementStateInvalid.value) {
                    updateTransactionReconciliationStatementInvalidState(true);
                }

                // 强制重新加载账户，确保余额为最新值
                accountsStore.updateAccountListInvalidState(true);
                accountsStore.loadAllAccounts({ force: true });

                if (!overviewStore.transactionOverviewStateInvalid) {
                    overviewStore.updateTransactionOverviewInvalidState(true);
                }

                if (!statisticsStore.transactionStatisticsStateInvalid) {
                    statisticsStore.updateTransactionStatisticsInvalidState(true);
                }

                resolve(transaction);
            }).catch(error => {
                logger.error('failed to save transaction', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    if (!isEdit) {
                        reject({ message: 'Unable to add transaction' });
                    } else {
                        reject({ message: 'Unable to save transaction' });
                    }
                } else {
                    reject(error);
                }
            });
        });
    }

    function saveTransactions({
        transactions,
        clientSessionId
    }: {
        transactions: Transaction[],
        clientSessionId: string
    }): Promise<Transaction[]> {
        return new Promise((resolve, reject) => {
            const validTransactions = transactions.filter(transaction =>
                transaction.type === TransactionType.Expense ||
                transaction.type === TransactionType.Income ||
                transaction.type === TransactionType.Transfer ||
                transaction.type === TransactionType.Investment
            );

            if (!validTransactions.length) {
                reject({ message: 'Unable to add transaction' });
                return;
            }

            const requestBody: TransactionImportRequest = {
                transactions: validTransactions.map(transaction => {
                    const actualTime = getActualUnixTimeForStore(
                        transaction.time,
                        transaction.utcOffset,
                        getBrowserTimezoneOffsetMinutes()
                    );

                    return transaction.toCreateRequest(clientSessionId, actualTime);
                }),
                clientSessionId: clientSessionId
            };

            services.addTransactions(requestBody).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result?.items) {
                    reject({ message: 'Unable to add transaction' });
                    return;
                }

                const createdTransactions = data.result.items.map(item => Transaction.of(item));

                if (!transactionListStateInvalid.value) {
                    updateTransactionListInvalidState(true);
                }

                if (!transactionReconciliationStatementStateInvalid.value) {
                    updateTransactionReconciliationStatementInvalidState(true);
                }

                accountsStore.updateAccountListInvalidState(true);
                accountsStore.loadAllAccounts({ force: true });

                if (!overviewStore.transactionOverviewStateInvalid) {
                    overviewStore.updateTransactionOverviewInvalidState(true);
                }

                if (!statisticsStore.transactionStatisticsStateInvalid) {
                    statisticsStore.updateTransactionStatisticsInvalidState(true);
                }

                resolve(createdTransactions);
            }).catch(error => {
                logger.error('failed to save transactions', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to add transaction' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function moveAllTransactionsBetweenAccounts({ fromAccountId, toAccountId, password }: { fromAccountId: string, toAccountId: string, password: string }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.moveAllTransactionsBetweenAccounts({ fromAccountId, toAccountId, password }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to move transactions' });
                    return;
                }

                if (!transactionListStateInvalid.value) {
                    updateTransactionListInvalidState(true);
                }

                if (!transactionReconciliationStatementStateInvalid.value) {
                    updateTransactionReconciliationStatementInvalidState(true);
                }

                if (!accountsStore.accountListStateInvalid) {
                    accountsStore.updateAccountListInvalidState(true);
                }

                if (!overviewStore.transactionOverviewStateInvalid) {
                    overviewStore.updateTransactionOverviewInvalidState(true);
                }

                if (!statisticsStore.transactionStatisticsStateInvalid) {
                    statisticsStore.updateTransactionStatisticsInvalidState(true);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to move transactions', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to move transactions' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function deleteTransaction({ transaction, defaultCurrency, beforeResolve }: { transaction: TransactionInfoResponse, defaultCurrency: string, beforeResolve?: BeforeResolveFunction }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.deleteTransaction({
                id: transaction.id
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to delete this transaction' });
                    return;
                }

                if (beforeResolve) {
                    beforeResolve(() => {
                        removeTransactionFromTransactionList({
                            currentTransaction: transaction,
                            defaultCurrency: defaultCurrency
                        });
                    });
                } else {
                    removeTransactionFromTransactionList({
                        currentTransaction: transaction,
                        defaultCurrency: defaultCurrency
                    });
                }

                if (!transactionReconciliationStatementStateInvalid.value) {
                    updateTransactionReconciliationStatementInvalidState(true);
                }

                // 强制重新加载账户，确保余额为最新值
                accountsStore.updateAccountListInvalidState(true);
                accountsStore.loadAllAccounts({ force: true });

                // 强制重新加载交易列表
                updateTransactionListInvalidState(true);

                if (!overviewStore.transactionOverviewStateInvalid) {
                    overviewStore.updateTransactionOverviewInvalidState(true);
                }

                if (!statisticsStore.transactionStatisticsStateInvalid) {
                    statisticsStore.updateTransactionStatisticsInvalidState(true);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to delete transaction', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to delete this transaction' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function recognizeReceiptImage({ imageFile, cancelableUuid }: { imageFile: File, cancelableUuid?: string }): Promise<RecognizedReceiptImageResponse> {
        return new Promise((resolve, reject) => {
            services.recognizeReceiptImage({ imageFile, cancelableUuid }).then(response => {
                const data = response.data as unknown as {
                    success?: boolean;
                    result?: {
                        amount?: number | null;
                        trade_time?: string | null;
                        description?: string | null;
                        payment_platform?: string | null;
                        provenance?: { provider?: string; model?: string; request_id?: string };
                        confidence?: number | null;
                        draft?: unknown;
                    };
                };

                if (!data || !data.success || !data.result) {
                    reject(buildRecognizeReceiptImageError('unknown', 'Unable to recognize image', 0));
                    return;
                }

                const raw = data.result;
                const provenance = raw.provenance || {};
                const draft = normalizeReceiptTransactionDraft(raw.draft);
                const normalized: RecognizedReceiptImageResponse = {
                    amount: typeof raw.amount === 'number' ? raw.amount : null,
                    tradeTime: typeof raw.trade_time === 'string' ? raw.trade_time : null,
                    description: typeof raw.description === 'string' ? raw.description : null,
                    paymentPlatform: typeof raw.payment_platform === 'string' ? raw.payment_platform : null,
                    provenance: {
                        provider: typeof provenance.provider === 'string' ? provenance.provider : 'unknown',
                        model: typeof provenance.model === 'string' ? provenance.model : undefined,
                        requestId: typeof provenance.request_id === 'string' ? provenance.request_id : ''
                    },
                    confidence: typeof raw.confidence === 'number' ? raw.confidence : null,
                    ...(draft ? { draft } : {})
                };

                resolve(normalized);
            }).catch(error => {
                if (error && error.canceled) {
                    reject(buildRecognizeReceiptImageError('cancelled', 'Recognition cancelled', 499, error));
                    return;
                }

                logger.error('failed to recognize image', error);

                const responseData = error && error.response && error.response.data;
                const status: number = (error && error.response && typeof error.response.status === 'number') ? error.response.status : 0;
                const rawErrorCode: string | undefined = responseData && typeof responseData.errorCode === 'string' ? responseData.errorCode : undefined;
                const errorCode: ReceiptImageErrorCode = mapReceiptImageErrorCode(rawErrorCode, status);
                const message: string = (responseData && typeof responseData.message === 'string' && responseData.message)
                    || 'Unable to recognize image';

                reject(buildRecognizeReceiptImageError(errorCode, message, status, error));
            });
        });
    }

    function cancelRecognizeReceiptImage(cancelableUuid: string): void {
        services.cancelRequest(cancelableUuid);
    }

    function parseImportTransaction({ fileType, fileEncoding, importFile, columnMapping, transactionTypeMapping, hasHeaderLine, timeFormat, timezoneFormat, amountDecimalSeparator, amountDigitGroupingSymbol, geoSeparator, geoOrder, tagSeparator, delimiter }: { fileType: string, fileEncoding?: string, importFile: File, columnMapping?: Record<number, number>, transactionTypeMapping?: Record<string, TransactionType>, hasHeaderLine?: boolean, timeFormat?: string, timezoneFormat?: string, amountDecimalSeparator?: string, amountDigitGroupingSymbol?: string, geoSeparator?: string, geoOrder?: string, tagSeparator?: string, delimiter?: string }): Promise<ImportTransactionResponsePageWrapper> {
        return new Promise((resolve, reject) => {
            services.parseImportTransaction({ fileType, fileEncoding, importFile, columnMapping, transactionTypeMapping, hasHeaderLine, timeFormat, timezoneFormat, amountDecimalSeparator, amountDigitGroupingSymbol, geoSeparator, geoOrder, tagSeparator, delimiter }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to parse import file' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('Unable to parse import file', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to parse import file' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function uploadTransactionPicture({ pictureFile, clientSessionId }: { pictureFile: File, clientSessionId?: string }): Promise<TransactionPictureInfoBasicResponse> {
        return new Promise((resolve, reject) => {
            services.uploadTransactionPicture({ pictureFile, clientSessionId }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to upload transaction picture' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('Unable to upload transaction picture', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to upload transaction picture' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function removeUnusedTransactionPicture({ pictureInfo }: { pictureInfo: TransactionPictureInfoBasicResponse }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.removeUnusedTransactionPicture({ id: pictureInfo.pictureId }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to remove transaction picture' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to remove transaction picture', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to remove transaction picture' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function getTransactionPictureUrl(pictureInfo?: TransactionPictureInfoBasicResponse | null, disableBrowserCache?: boolean | string): string | undefined {
        if (!pictureInfo || !pictureInfo.originalUrl) {
            return undefined;
        }

        return services.getTransactionPictureUrlWithToken(pictureInfo.originalUrl, disableBrowserCache);
    }

    function collapseMonthInTransactionList({ monthList, collapse }: { monthList: TransactionMonthList, collapse: boolean }): void {
        if (monthList) {
            monthList.opened = !collapse;
        }
    }

    return {
        // 状态
        transactionDraft,
        transactionsFilter,
        transactions,
        transactionsNextTimeId,
        transactionListStateInvalid,
        transactionReconciliationStatementStateInvalid,
        // 计算状态
        allFilterCategoryIds,
        allFilterAccountIds,
        allFilterTagIds,
        allFilterCategoryIdsCount,
        allFilterAccountIdsCount,
        allFilterTagIdsCount,
        noTransaction,
        hasMoreTransaction,
        // 函数
        initTransactionDraft,
        isTransactionDraftModified,
        saveTransactionDraft,
        clearTransactionDraft,
        setTransactionSuitableDestinationAmount,
        updateTransactionListInvalidState,
        updateTransactionReconciliationStatementInvalidState,
        resetTransactions,
        clearTransactions,
        initTransactionListFilter,
        updateTransactionListFilter,
        getTransactionListPageParams,
        getExportTransactionDataRequestByTransactionFilter,
        loadTransactions,
        loadMonthlyAllTransactions,
        getReconciliationStatements,
        getTransaction,
        saveTransaction,
        saveTransactions,
        moveAllTransactionsBetweenAccounts,
        deleteTransaction,
        recognizeReceiptImage,
        cancelRecognizeReceiptImage,
        parseImportTransaction,
        uploadTransactionPicture,
        removeUnusedTransactionPicture,
        getTransactionPictureUrl,
        collapseMonthInTransactionList
    };
});
