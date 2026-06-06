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
    | 'accountRecognitionRules'
    | 'llmConfigs'
    | 'ocrConfig';

export interface SettingsBundleExportAuth {
    readonly password?: string;
}

export const SETTINGS_BUNDLE_SECTION_LABEL_KEYS: Readonly<Record<SettingsBundleSectionKey, string>> = {
    accounts: 'Accounts',
    transactionCategories: 'Transaction Categories',
    transactionTags: 'Transaction Tags',
    transactionTemplates: 'Transaction Templates',
    scheduledTransactions: 'Scheduled Transactions',
    categoryRecognitionRules: 'Category Recognition Rules',
    accountRecognitionRules: 'Account Recognition Rules',
    llmConfigs: 'LLM Config',
    ocrConfig: 'OCR Config',
};

export interface SettingsBundleDataManagementEntry {
    readonly sectionKey: SettingsBundleSectionKey;
    readonly titleKey: string;
    readonly descriptionKey: string;
    readonly desktopRoute: string;
    readonly mobileRoute?: string;
}

export interface DisplaySettingsBundleDataManagementEntry extends SettingsBundleDataManagementEntry {
    readonly title: string;
    readonly description: string;
}

export const SETTINGS_BUNDLE_DATA_MANAGEMENT_ENTRIES: readonly SettingsBundleDataManagementEntry[] = [
    {
        sectionKey: 'accounts',
        titleKey: SETTINGS_BUNDLE_SECTION_LABEL_KEYS.accounts,
        descriptionKey: 'Import or export this settings JSON section. Open its management page for detailed edits.',
        desktopRoute: '/account/list',
        mobileRoute: '/account/list',
    },
    {
        sectionKey: 'transactionCategories',
        titleKey: SETTINGS_BUNDLE_SECTION_LABEL_KEYS.transactionCategories,
        descriptionKey: 'Import or export this settings JSON section. Open its management page for detailed edits.',
        desktopRoute: '/category/list',
        mobileRoute: '/category/all',
    },
    {
        sectionKey: 'transactionTags',
        titleKey: SETTINGS_BUNDLE_SECTION_LABEL_KEYS.transactionTags,
        descriptionKey: 'Import or export this settings JSON section. Open its management page for detailed edits.',
        desktopRoute: '/tag/list',
        mobileRoute: '/tag/list',
    },
    {
        sectionKey: 'transactionTemplates',
        titleKey: SETTINGS_BUNDLE_SECTION_LABEL_KEYS.transactionTemplates,
        descriptionKey: 'Import or export this settings JSON section. Open its management page for detailed edits.',
        desktopRoute: '/template/list',
        mobileRoute: '/template/list',
    },
    {
        sectionKey: 'scheduledTransactions',
        titleKey: SETTINGS_BUNDLE_SECTION_LABEL_KEYS.scheduledTransactions,
        descriptionKey: 'Import or export this settings JSON section. Open its management page for detailed edits.',
        desktopRoute: '/schedule/list',
        mobileRoute: '/schedule/list',
    },
    {
        sectionKey: 'categoryRecognitionRules',
        titleKey: SETTINGS_BUNDLE_SECTION_LABEL_KEYS.categoryRecognitionRules,
        descriptionKey: 'Import or export this settings JSON section. Open its management page for detailed edits.',
        desktopRoute: '/pairing/list?domain=transfer&tab=rules',
    },
    {
        sectionKey: 'accountRecognitionRules',
        titleKey: SETTINGS_BUNDLE_SECTION_LABEL_KEYS.accountRecognitionRules,
        descriptionKey: 'Import or export this settings JSON section. Open its management page for detailed edits.',
        desktopRoute: '/pairing/list?domain=transfer&tab=accounts',
        mobileRoute: '/account/rules',
    },
    {
        sectionKey: 'llmConfigs',
        titleKey: SETTINGS_BUNDLE_SECTION_LABEL_KEYS.llmConfigs,
        descriptionKey: 'Import or export this settings JSON section. Open its management page for detailed edits.',
        desktopRoute: '/pairing/list?domain=llm&tab=config',
    },
    {
        sectionKey: 'ocrConfig',
        titleKey: SETTINGS_BUNDLE_SECTION_LABEL_KEYS.ocrConfig,
        descriptionKey: 'Import or export this settings JSON section. Open its management page for detailed edits.',
        desktopRoute: '/pairing/list?domain=llm&tab=ocr-config',
    },
];

const WINDOWS_RESERVED_FILE_NAME_PATTERN = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i;

export function sanitizeWindowsFileNameSegment(value: string, fallback = 'settings'): string {
    const sanitized = value
        .replace(/[<>:"/\\|?*\u0000-\u001f]+/g, '_')
        .replace(/_+/g, '_')
        .replace(/\s+/g, ' ')
        .trim()
        .replace(/^[_. ]+|[_. ]+$/g, '');

    if (!sanitized) {
        return fallback;
    }

    if (WINDOWS_RESERVED_FILE_NAME_PATTERN.test(sanitized)) {
        return `_${sanitized}`;
    }

    return sanitized;
}

export function formatSettingsBundleSectionFileTimestamp(date: Date = new Date()): string {
    return date.toISOString().replace(/[-:T.Z]/g, '').slice(0, 14);
}

export function buildSettingsBundleSectionFileName(sectionLabel: string, date: Date = new Date()): string {
    return `${sanitizeWindowsFileNameSegment(sectionLabel)}_${formatSettingsBundleSectionFileTimestamp(date)}.json`;
}
