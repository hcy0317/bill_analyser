import { nextTick, ref, type ComputedRef, type Ref } from 'vue';

import { getElementActualHeights, getElementBoundingRect } from '@/lib/ui/mobile.ts';

import type { TextualYearMonth } from '@/core/datetime.ts';
import type { TransactionMonthList } from '@/stores/transaction.ts';
import type { Transaction } from '@/models/transaction.ts';

import { isNumber } from '@/lib/common.ts';

export function useMobileTransactionMonthList(transactions: ComputedRef<TransactionMonthList[]>): {
    transactionInvisibleYearMonths: Ref<Record<TextualYearMonth, boolean>>;
    resetTransactionMonthListState: () => void;
    getTransactionMonthTitleDomId: (yearMonth: TextualYearMonth) => string;
    getTransactionMonthListDomId: (yearMonth: TextualYearMonth) => string;
    getTransactionDomId: (transaction: Transaction) => string;
    isTransactionMonthListInvisible: (transactionMonthList: TransactionMonthList) => boolean;
    getTransactionMonthListHeight: (transactionMonthList: TransactionMonthList) => string;
    setTransactionMonthListHeights: (reset: boolean) => Promise<void>;
    setTransactionInvisibleYearMonthList: () => void;
    getTransactionDateStyle: (transaction: Transaction, previousTransaction: Transaction | undefined) => Record<string, string>;
} {
    const transactionInvisibleYearMonths = ref<Record<TextualYearMonth, boolean>>({});
    const transactionYearMonthListHeights = ref<Record<TextualYearMonth, number>>({});

    function resetTransactionMonthListState(): void {
        transactionInvisibleYearMonths.value = {};
        transactionYearMonthListHeights.value = {};
    }

    function getTransactionMonthTitleDomId(yearMonth: TextualYearMonth): string {
        return 'transaction_month_title_' + yearMonth;
    }

    function getTransactionMonthListDomId(yearMonth: TextualYearMonth): string {
        return 'transaction_month_list_' + yearMonth;
    }

    function getTransactionDomId(transaction: Transaction): string {
        return 'transaction_' + transaction.id;
    }

    function isTransactionMonthListInvisible(transactionMonthList: TransactionMonthList): boolean {
        if (!transactionYearMonthListHeights.value[transactionMonthList.yearDashMonth]) {
            return false;
        }

        if (!transactionMonthList.opened) {
            return true;
        }

        if (transactionInvisibleYearMonths.value[transactionMonthList.yearDashMonth]) {
            return true;
        }

        return false;
    }

    function getTransactionMonthListHeight(transactionMonthList: TransactionMonthList): string {
        if (isTransactionMonthListInvisible(transactionMonthList)) {
            return transactionYearMonthListHeights.value[transactionMonthList.yearDashMonth] + 'px';
        }

        return 'auto';
    }

    function setTransactionMonthListHeights(reset: boolean): Promise<void> {
        return nextTick(() => {
            if (reset) {
                resetTransactionMonthListState();
            }

            if (transactions.value && transactions.value.length) {
                const heights: Record<string, number> = getElementActualHeights('.transaction-month-list');

                for (let i = 0; i < transactions.value.length - 1; i++) {
                    const transactionMonthList = transactions.value[i] as TransactionMonthList;
                    const yearDashMonth = transactionMonthList.yearDashMonth;
                    const domId = getTransactionMonthListDomId(yearDashMonth);
                    const height = heights[domId];

                    if (!transactionYearMonthListHeights.value[yearDashMonth] && isNumber(height)) {
                        transactionYearMonthListHeights.value[yearDashMonth] = height;
                    }
                }
            }
        });
    }

    function setTransactionInvisibleYearMonthList(): void {
        if (!transactions.value || !transactions.value.length) {
            return;
        }

        for (let i = 0; i < transactions.value.length - 1; i++) {
            const transactionMonthList = transactions.value[i] as TransactionMonthList;
            const yearDashMonth = transactionMonthList.yearDashMonth;

            const titleDomId = getTransactionMonthTitleDomId(yearDashMonth);
            const titleRect: DOMRect | null = getElementBoundingRect(`#${titleDomId}`);

            if (!titleRect) {
                continue;
            }

            const listHeight = transactionYearMonthListHeights.value[yearDashMonth] || 0;
            const listRectTop = titleRect.top + titleRect.height;
            const listRectBottom = listRectTop + listHeight;
            const invisible = listRectTop > 2 * window.innerHeight || listRectBottom < -2 * window.innerHeight;

            if (invisible) {
                transactionInvisibleYearMonths.value[yearDashMonth] = true;
            } else {
                delete transactionInvisibleYearMonths.value[yearDashMonth];
            }
        }
    }

    function getTransactionDateStyle(transaction: Transaction, previousTransaction: Transaction | undefined): Record<string, string> {
        // 使用完整日期比较（YYYY-MM-DD），而非仅比较月中的日（1-31）
        if (!previousTransaction || transaction.gregorianCalendarYearDashMonthDashDay !== previousTransaction.gregorianCalendarYearDashMonthDashDay) {
            return {};
        }

        return {
            color: 'transparent'
        };
    }

    return {
        transactionInvisibleYearMonths,
        resetTransactionMonthListState,
        getTransactionMonthTitleDomId,
        getTransactionMonthListDomId,
        getTransactionDomId,
        isTransactionMonthListInvisible,
        getTransactionMonthListHeight,
        setTransactionMonthListHeights,
        setTransactionInvisibleYearMonthList,
        getTransactionDateStyle
    };
}
