import type {
    ImportConfigMatchDto,
    ImportConfigSuggestion,
    ImportFieldMappings as NeutralImportFieldMappings,
    ImportFilePreviewData
} from '@/models/import_config.ts';

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

export type ImportFieldMappings = NeutralImportFieldMappings;

export type ImportConfigMatchResult =
    Pick<ImportConfigMatchDto, 'id' | 'name' | 'fieldMappings'>
    & Partial<Omit<ImportConfigMatchDto, 'id' | 'name' | 'fieldMappings'>>;

export type ImportFilePreviewResult = ImportFilePreviewData;

export type ImportConfigSuggestionResult = ImportConfigSuggestion;

export interface ImportTransactionCheckDataFilterMenuGroup {
    title: string;
    summary?: string;
}

export interface UnmatchedFileInfo {
    originalName: string;
    tempPath: string;
    reason?: string;
}

export const SERVER_PAGED_PREVIEW_SORTABLE_COLUMNS = new Set<string>([
    'time',
    'type',
    'sourceAmountCents',
    'counterparty',
    'paymentMethod',
    'comment'
]);
