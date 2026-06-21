export type ImportTransactionDialogStep = 'uploadFile' | 'defineColumn' | 'executeCustomScript' | 'checkData' | 'finalResult';
export type ImportFlowProgressKey = 'selectSource' | 'parseStageRows' | 'reviewPreview' | 'confirmImport' | 'result';

export interface ImportFlowProgressItem {
    complete: boolean;
    key: ImportFlowProgressKey;
    title: string;
    active: boolean;
}

export enum ImportDSVProcessMethod {
    AutoDetect,
    ColumnMapping,
}

export interface ImportFieldMappings {
    includeHeader?: boolean;
    columnMapping?: Record<number, number>;
    transactionTypeMapping?: Record<string, number>;
    timeFormat?: string;
    timezoneFormat?: string;
    amountDecimalSeparator?: string;
    amountDigitGroupingSymbol?: string;
    geoLocationSeparator?: string;
    geoLocationOrder?: string;
    tagSeparator?: string;
}

export interface ImportConfigMatchResult {
    id: number;
    name: string;
    fileFormat?: string;
    description?: string;
    descriptionSummary?: string;
    fieldMappings: ImportFieldMappings;
    sampleHeaders?: string[];
    dateFormat?: string;
    delimiter?: string;
    encoding?: string;
    skipRows?: number;
    hasHeader?: boolean;
    customRules?: Record<string, unknown>;
    isDefault?: boolean;
    defaultRecommendation?: boolean;
    matchScore?: number;
    matchReason?: string;
}

export interface ImportFilePreviewResult {
    headers: string[];
    sampleData: string[][];
    previewRows?: string[][];
    totalRows: number;
    encoding?: string;
    delimiter?: string;
}

export interface ImportConfigSuggestionResult {
    includeHeader?: boolean;
    columnMapping?: Record<string, number>;
    transactionTypeMapping?: Record<string, number>;
    suggestions?: Array<{
        columnType: number;
        columnIndex: number;
        header: string;
        score: number;
    }>;
}

export interface ImportTransactionCheckDataFilterMenuGroup {
    title: string;
    summary?: string;
}

export interface UnmatchedFileInfo {
    originalName: string;
    tempPath: string;
}

export const SERVER_PAGED_PREVIEW_SORTABLE_COLUMNS = new Set<string>([
    'time',
    'type',
    'sourceAmountCents',
    'counterparty',
    'paymentMethod',
    'comment'
]);
