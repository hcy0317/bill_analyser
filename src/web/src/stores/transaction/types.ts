import type { TextualYearMonth } from '@/core/datetime.ts';
import type { Transaction } from '@/models/transaction.ts';

export interface TransactionListPartialFilter {
    dateType?: number;
    maxTime?: number;
    minTime?: number;
    type?: number;
    categoryIds?: string;
    accountIds?: string;
    tagIds?: string;
    tagFilterType?: number;
    amountFilterCents?: string;
    keyword?: string;
}

export interface TransactionListFilter extends TransactionListPartialFilter {
    dateType: number;
    maxTime: number;
    minTime: number;
    type: number;
    categoryIds: string;
    accountIds: string;
    tagIds: string;
    tagFilterType: number;
    amountFilterCents: string;
    keyword: string;
}

export interface TransactionTotalAmountCents {
    expenseCents: number;
    incompleteExpense: boolean;
    incomeCents: number;
    incompleteIncome: boolean;
}

export interface TransactionMonthList {
    readonly year: number;
    readonly month: number; // 从 1 开始计数（1 = 一月，12 = 十二月）
    readonly yearDashMonth: TextualYearMonth;
    opened: boolean;
    readonly items: Transaction[];
    readonly totalAmountCents: TransactionTotalAmountCents;
    readonly dailyTotalAmountsCents: Record<string, TransactionTotalAmountCents>;
}
