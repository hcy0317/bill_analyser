export type ImportConfigJsonValue =
    | string
    | number
    | boolean
    | null
    | ImportConfigJsonValue[]
    | ImportConfigJsonObject;

export interface ImportConfigJsonObject {
    [key: string]: ImportConfigJsonValue;
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

export interface ImportConfigDto {
    id: number;
    name: string;
    fileFormat: string;
    description: string;
    fieldMappings: ImportFieldMappings;
    sampleHeaders: string[];
    dateFormat: string;
    delimiter: string | null;
    encoding: string;
    skipRows: number;
    hasHeader: boolean;
    customRules: ImportConfigJsonObject;
    isDefault: boolean;
    createdAt: string;
    updatedAt: string;
}

export interface ImportConfigMatchDto extends ImportConfigDto {
    descriptionSummary: string;
    defaultRecommendation: boolean;
    matchScore: number;
    matchReason: string;
}

export interface ImportConfigColumnSuggestion {
    columnType: number;
    columnIndex: number;
    header: string;
    score: number;
}

export interface ImportConfigSuggestion {
    includeHeader: boolean;
    columnMapping: Record<string, number>;
    transactionTypeMapping: Record<string, number>;
    suggestions: ImportConfigColumnSuggestion[];
}

export interface ImportConfigSaveRequest {
    id?: number;
    name: string;
    fileFormat: string;
    description: string;
    fieldMappings: ImportFieldMappings;
    sampleHeaders: string[];
    dateFormat: string;
    delimiter: string | null;
    encoding: string;
    skipRows: number;
    hasHeader: boolean;
    customRules: ImportConfigJsonObject;
    isDefault: boolean;
}

export interface ImportConfigListRequest {
    fileFormat?: string;
}

export interface ImportConfigMatchRequest {
    fileFormat: string;
    headers: string[];
}

export interface ImportConfigSuggestRequest extends ImportConfigMatchRequest {
    sampleRows?: string[][];
}

export interface ImportFilePreviewRequest {
    importFile: File;
    fileEncoding?: string;
    delimiter?: string;
}

export interface ImportTempFilePreviewRequest {
    sessionId: string;
    tempPath: string;
    fileEncoding?: string;
    delimiter?: string;
}

export interface ImportFilePreviewData {
    headers: string[];
    sampleData: string[][];
    previewRows: string[][];
    totalRows: number;
    encoding: string;
    delimiter: string | null;
}

export interface ImportGenericParseRequest {
    sessionId: string;
    tempPath: string;
    columnMapping: Record<string, number>;
    transactionTypeMapping?: Record<string, number>;
    hasHeaderLine?: boolean;
    timeFormat?: string;
    timezoneFormat?: string;
    amountDecimalSeparator?: string;
    amountDigitGroupingSymbol?: string;
    fileEncoding?: string;
    delimiter?: string;
}

export interface ImportGenericParseData {
    session_id: string;
    parsed_count: number;
    files: unknown[];
    unmatched_files: unknown[];
    errors: string[];
}
