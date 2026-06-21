import { computed, type ComputedRef, type Ref } from 'vue';

import { CategoryType } from '@/core/category.ts';
import { Account } from '@/models/account.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import type { User } from '@/models/user.ts';

import {
    getTransactionPrimaryCategoryName,
    getTransactionSecondaryCategoryName
} from '@/lib/category.ts';

type Translate = (key: string, params?: Record<string, unknown>) => string;

export function createAccountCategorySelectionTexts({
    allAccounts,
    allCategories,
    newProfile,
    tt
}: {
    allAccounts: ComputedRef<Account[]>;
    allCategories: ComputedRef<Record<number, TransactionCategory[]>>;
    newProfile: Ref<User>;
    tt: Translate;
}) {
    const defaultAccountSelectionText = computed<string>(() => Account.findAccountNameById(allAccounts.value, newProfile.value.defaultAccountId, tt('Unspecified')) || tt('Unspecified'));
    const cashAccountSelectionText = computed<string>(() => Account.findAccountNameById(allAccounts.value, newProfile.value.cashAccountId, tt('Unspecified')) || tt('Unspecified'));
    const cashTransferCategoryPrimaryText = computed<string>(() => getTransactionPrimaryCategoryName(
        newProfile.value.cashTransferCategoryId,
        allCategories.value[CategoryType.Transfer] || []
    ));
    const cashTransferCategorySecondaryText = computed<string>(() => getTransactionSecondaryCategoryName(
        newProfile.value.cashTransferCategoryId,
        allCategories.value[CategoryType.Transfer] || []
    ));

    return {
        defaultAccountSelectionText,
        cashAccountSelectionText,
        cashTransferCategoryPrimaryText,
        cashTransferCategorySecondaryText
    };
}
