interface RootResetContext {
    readonly accountsStore: { resetAccounts(): void };
    readonly exchangeRatesStore: { resetLatestExchangeRates(): void };
    readonly overviewStore: { resetTransactionOverview(): void };
    readonly statisticsStore: { resetTransactionStatistics(): void };
    readonly transactionCategoriesStore: { resetTransactionCategories(): void };
    readonly transactionTagsStore: { resetTransactionTags(): void };
    readonly transactionTemplatesStore: { resetTransactionTemplates(): void };
    readonly transactionsStore: { resetTransactions(): void };
    readonly userStore: { resetUserBasicInfo(): void };
    readonly clearNotification: () => void;
}

export function resetRootDomainStores(context: RootResetContext, resetUserInfoAndSettings: boolean): void {
    if (resetUserInfoAndSettings) {
        context.exchangeRatesStore.resetLatestExchangeRates();
    }

    context.clearNotification();

    context.statisticsStore.resetTransactionStatistics();
    context.overviewStore.resetTransactionOverview();
    context.transactionsStore.resetTransactions();
    context.transactionTagsStore.resetTransactionTags();
    context.transactionCategoriesStore.resetTransactionCategories();
    context.transactionTemplatesStore.resetTransactionTemplates();
    context.accountsStore.resetAccounts();

    if (resetUserInfoAndSettings) {
        context.userStore.resetUserBasicInfo();
    }
}
