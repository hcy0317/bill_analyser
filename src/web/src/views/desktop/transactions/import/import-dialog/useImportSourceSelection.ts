import { computed, type ComputedRef, type Ref } from 'vue';

import type { LocalizedImportFileTypeSubType } from '@/core/file.ts';
import { looksLikeStructuredBillStatementFile } from './importConfigHelpers.ts';

export interface ImportSourceSelectionOptions {
    importFile: ComputedRef<File | undefined>;
    importFiles: Ref<File[]>;
    selectedFileTypes: Ref<string[]>;
}

// 导入来源选择只根据文件名和选择状态派生 UI 可见项，不触发文件读取或服务端请求。
export function useImportSourceSelection(options: ImportSourceSelectionOptions) {
    const fileType = computed<string>(() => {
        const type = options.selectedFileTypes.value[0];
        if (options.selectedFileTypes.value.length > 0 && type && type !== 'auto') {
            return type;
        }

        const currentFile = options.importFile.value;
        const lowerName = currentFile?.name.toLowerCase() || '';
        if (lowerName.endsWith('.csv') || lowerName.endsWith('.txt')) {
            return 'dsv';
        }

        return 'auto';
    });

    const showHandlingMethodSelector = computed<boolean>(() => {
        if (options.importFiles.value.length > 1) {
            return false;
        }

        if (fileType.value !== 'dsv' && fileType.value !== 'dsv_data') {
            return false;
        }

        return !looksLikeStructuredBillStatementFile(options.importFile.value);
    });

    return {
        allFileSubTypes: computed<LocalizedImportFileTypeSubType[] | undefined>(() => undefined),
        exportFileGuideDocumentLanguageName: computed<string | undefined>(() => undefined),
        exportFileGuideDocumentUrl: computed<string | undefined>(() => undefined),
        fileType,
        isImportDataFromTextbox: computed<boolean>(() => false),
        showHandlingMethodSelector,
        supportedImportFileExtensions: computed<string>(() => '.csv,.xls,.xlsx,.txt')
    };
}
