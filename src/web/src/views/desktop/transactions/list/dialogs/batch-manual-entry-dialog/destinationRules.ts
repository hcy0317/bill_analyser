import { TransactionType } from '@/core/transaction.ts';
import type { Transaction } from '@/models/transaction.ts';

export function requiresDestination(transaction: Transaction): boolean {
    return transaction.type === TransactionType.Transfer || transaction.type === TransactionType.Investment;
}

export function shouldSyncDestinationAmount(transaction: Transaction): boolean {
    return requiresDestination(transaction)
        && (!transaction.destinationAmountCents || transaction.destinationAmountCents === transaction.sourceAmountCents);
}
