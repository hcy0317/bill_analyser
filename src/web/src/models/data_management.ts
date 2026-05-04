export interface ExportTransactionDataRequest {
    readonly maxTime: number;
    readonly minTime: number;
    readonly type: number;
    readonly categoryIds: string;
    readonly accountIds: string;
    readonly tagIds: string;
    readonly tagFilterType: number;
    readonly amountFilter: string;
    readonly keyword: string;
}

export interface ClearDataRequest {
    readonly password: string;
}

export interface ClearAccountTransactionsRequest {
    readonly accountId: string;
    readonly password: string;
}

export interface DataStatisticsResponse {
    readonly totalAccountCount: string;
    readonly totalTransactionCategoryCount: string;
    readonly totalTransactionTagCount: string;
    readonly totalTransactionCount: string;
    readonly totalTransactionPictureCount: string;
    readonly totalTransactionTemplateCount: string;
    readonly totalScheduledTransactionCount: string;
}

export interface DisplayDataStatistics {
    readonly totalAccountCount: string;
    readonly totalTransactionCategoryCount: string;
    readonly totalTransactionTagCount: string;
    readonly totalTransactionCount: string;
    readonly totalTransactionPictureCount: string;
    readonly totalTransactionTemplateCount: string;
    readonly totalScheduledTransactionCount: string;
}

export interface SettingsBundleImportSectionSummary {
    readonly created: number;
    readonly updated: number;
    readonly skipped: number;
}

export interface SettingsBundleImportResult {
    readonly dryRun: boolean;
    readonly schemaVersion: number;
    readonly sections: Record<string, SettingsBundleImportSectionSummary>;
    readonly warnings: string[];
}

export type SettingsBundleSectionKey =
    'accounts'
    | 'transactionCategories'
    | 'transactionTags'
    | 'transactionTemplates'
    | 'scheduledTransactions'
    | 'categoryRecognitionRules'
    | 'llmConfigs'
    | 'ocrConfig';

export interface SettingsBundleExportAuth {
    readonly password?: string;
}
