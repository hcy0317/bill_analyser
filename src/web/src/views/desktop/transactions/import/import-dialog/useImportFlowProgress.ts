import { computed, type ComputedRef, type Ref } from 'vue';

import type {
    ImportConfigMatchResult,
    ImportFlowProgressItem,
    ImportFlowProgressKey,
    ImportTransactionDialogStep,
    UnmatchedFileInfo
} from './types.ts';

const IMPORT_FLOW_PROGRESS_ORDER: ImportFlowProgressKey[] = [
    'selectSource',
    'parseStageRows',
    'reviewPreview',
    'confirmImport',
    'result'
];

const FALLBACK_IMPORT_FLOW_PROGRESS_ITEM: ImportFlowProgressItem = {
    active: true,
    complete: false,
    key: 'selectSource',
    title: ''
};

export interface ImportFlowProgressOptions {
    currentStep: Ref<ImportTransactionDialogStep>;
    submitting: Ref<boolean>;
    importProcess: Ref<number>;
    unmatchedFilesQueue: Ref<UnmatchedFileInfo[]>;
    currentUnmatchedIndex: Ref<number>;
    matchedImportConfig: Ref<ImportConfigMatchResult | null>;
    previewTotalCount: Ref<number>;
    importedCount: Ref<number | null>;
    fileName: ComputedRef<string>;
    supportedImportFileExtensions: ComputedRef<string>;
    translate: (key: string, params?: Record<string, unknown>) => string;
    formatNumber: (value: number, fractionDigits?: number) => string;
    formatCount: (count: number) => string;
}

// 导入进度条只派生展示状态，不修改父组件流程状态，便于复用为其他导入入口模板。
export function useImportFlowProgress(options: ImportFlowProgressOptions) {
    const importFlowProgressTitleMap = computed<Record<ImportFlowProgressKey, string>>(() => ({
        selectSource: options.translate('Select File'),
        parseStageRows: `${options.translate('Parser')} / ${options.translate('Define Columns')}`,
        reviewPreview: options.translate('Check Data'),
        confirmImport: `${options.translate('Confirm')} ${options.translate('Import')}`,
        result: options.translate('Import Result')
    }));

    const currentImportFlowProgressKey = computed<ImportFlowProgressKey>(() => {
        if (options.currentStep.value === 'finalResult') {
            return 'result';
        }

        if (options.currentStep.value === 'checkData') {
            return options.submitting.value ? 'confirmImport' : 'reviewPreview';
        }

        if (
            options.currentStep.value === 'defineColumn'
            || options.currentStep.value === 'executeCustomScript'
            || (options.currentStep.value === 'uploadFile' && options.submitting.value)
        ) {
            return 'parseStageRows';
        }

        return 'selectSource';
    });

    const currentFlowProgressIndex = computed<number>(() => {
        const index = IMPORT_FLOW_PROGRESS_ORDER.indexOf(currentImportFlowProgressKey.value);
        return index >= 0 ? index : 0;
    });

    const importFlowProgressItems = computed<ImportFlowProgressItem[]>(() => {
        const activeIndex = currentFlowProgressIndex.value;
        const titles = importFlowProgressTitleMap.value;

        return IMPORT_FLOW_PROGRESS_ORDER.map((key, index) => ({
            active: index === activeIndex,
            complete: index < activeIndex,
            key,
            title: titles[key]
        }));
    });

    const currentFlowProgressItem = computed<ImportFlowProgressItem>(() => {
        return importFlowProgressItems.value[currentFlowProgressIndex.value]
            ?? importFlowProgressItems.value[0]
            ?? FALLBACK_IMPORT_FLOW_PROGRESS_ITEM;
    });

    const currentFlowProgressValue = computed<number>(() => {
        return ((currentFlowProgressIndex.value + 1) / IMPORT_FLOW_PROGRESS_ORDER.length) * 100;
    });

    const currentFlowProgressDetail = computed<string>(() => {
        if (currentImportFlowProgressKey.value === 'parseStageRows') {
            if (options.unmatchedFilesQueue.value.length > 0) {
                const fileName = options.unmatchedFilesQueue.value[options.currentUnmatchedIndex.value]?.originalName || '';
                return `${fileName} (${options.currentUnmatchedIndex.value + 1}/${options.unmatchedFilesQueue.value.length})`;
            }

            return options.matchedImportConfig.value?.name || options.translate('Parser');
        }

        if (currentImportFlowProgressKey.value === 'reviewPreview') {
            return options.previewTotalCount.value > 0
                ? options.translate('format.misc.previewCount', { count: options.formatCount(options.previewTotalCount.value) })
                : options.translate('Import Preview');
        }

        if (currentImportFlowProgressKey.value === 'confirmImport') {
            return options.importProcess.value > 0
                ? options.translate('format.misc.importingTransactions', { process: options.formatNumber(options.importProcess.value, 2) })
                : options.translate('Confirm');
        }

        if (currentImportFlowProgressKey.value === 'result') {
            return options.translate('format.misc.importTransactionResult', { count: options.formatCount(options.importedCount.value || 0) });
        }

        return options.fileName.value || options.supportedImportFileExtensions.value;
    });

    return {
        currentFlowProgressDetail,
        currentFlowProgressIndex,
        currentFlowProgressItem,
        currentFlowProgressValue,
        currentImportFlowProgressKey,
        importFlowProgressItems
    };
}
