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

/**
 * 中文说明：集中执行根 store 退出/切换用户时的跨域状态清理，按参数决定是否同步清理用户资料和设置缓存。
 */
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
