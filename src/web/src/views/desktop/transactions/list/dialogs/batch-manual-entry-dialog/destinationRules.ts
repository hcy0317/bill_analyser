import { TransactionType } from '@/core/transaction.ts';
import type { Transaction } from '@/models/transaction.ts';

/** 判断批量录入行是否需要展示目标账户和目标金额字段。 */
export function requiresDestination(transaction: Transaction): boolean {
    return transaction.type === TransactionType.Transfer || transaction.type === TransactionType.Investment;
}

/** 判断来源金额变化时是否应同步目标金额，当前仅转账保持同步。 */
export function shouldSyncDestinationAmount(transaction: Transaction): boolean {
    return requiresDestination(transaction)
        && (!transaction.destinationAmountCents || transaction.destinationAmountCents === transaction.sourceAmountCents);
}
