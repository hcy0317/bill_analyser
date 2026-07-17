import type { ImportConfigMatchResult, UnmatchedFileInfo } from './types.ts';

export interface ImportStageUnmatchedFile {
    original_name: string;
    temp_path: string;
    reason?: string;
    error_code?: string;
    errorCode?: string;
}

// 导入模板来自服务端历史数据，进入对话框前统一裁剪成可编辑的稳定形状。
export function normalizeImportConfigMatchResult(
    config: Partial<ImportConfigMatchResult>
): ImportConfigMatchResult | null {
    const id = Number(config.id);
    const name = typeof config.name === 'string' ? config.name.trim() : '';

    if (!Number.isFinite(id) || !name) {
        return null;
    }

    return {
        id,
        name,
        fileFormat: config.fileFormat,
        description: config.description,
        descriptionSummary: config.descriptionSummary,
        fieldMappings: config.fieldMappings || {},
        sampleHeaders: config.sampleHeaders || [],
        dateFormat: config.dateFormat,
        delimiter: config.delimiter,
        encoding: config.encoding,
        skipRows: config.skipRows,
        hasHeader: config.hasHeader,
        customRules: config.customRules,
        isDefault: config.isDefault,
        defaultRecommendation: config.defaultRecommendation,
        matchScore: config.matchScore,
        matchReason: config.matchReason
    };
}

type NamedFile = Pick<File, 'name'>;
type QueuedImportFile = { originalName?: string };

export function looksLikeStructuredBillStatementFile(file?: NamedFile): boolean {
    const fileName = file?.name?.toLowerCase() || '';
    return /微信支付账单|wechat|wxpay|支付宝交易明细|alipay/.test(fileName);
}

export function resolveImportConfigFileFormat(file?: NamedFile): string {
    const lowerName = file?.name.toLowerCase() || '';
    if (lowerName.endsWith('.csv') || lowerName.endsWith('.txt')) {
        return 'csv';
    }
    if (lowerName.endsWith('.xlsx') || lowerName.endsWith('.xls')) {
        return 'excel';
    }
    return 'csv';
}

export function resolveActiveImportSource(
    file?: NamedFile,
    queuedFile?: QueuedImportFile
): { fileName: string; fileFormat: string } {
    const fileName = queuedFile?.originalName || file?.name || 'import';
    return {
        fileName,
        fileFormat: resolveImportConfigFileFormat({ name: fileName })
    };
}

export function resolveUnmatchedImportFiles(files: readonly ImportStageUnmatchedFile[]): {
    errorMessage: string;
    queue: UnmatchedFileInfo[];
} {
    const unsupportedNames = files
        .filter(file => (file.error_code || file.errorCode) === 'unsupported_legacy_xls')
        .map(file => file.original_name);
    return {
        errorMessage: unsupportedNames.length > 0
            ? `不支持旧版二进制 XLS：${unsupportedNames.join('、')}。请先转换为 XLSX 或 CSV 后重新导入。`
            : '',
        queue: files.map(file => ({
            originalName: file.original_name,
            tempPath: file.temp_path,
            reason: file.reason
        }))
    };
}

export function getMatchedImportConfigMessage(config: ImportConfigMatchResult): string {
    if (config.matchReason === 'default_template_fallback') {
        return `已自动回退到默认模板：${config.name}`;
    }

    return `已自动套用模板：${config.name}`;
}

export function getImportConfigDisplayDescription(
    config: Partial<ImportConfigMatchResult> | null | undefined
): string {
    if (!config) {
        return '';
    }

    return config.description || config.descriptionSummary || config.sampleHeaders?.join(' / ') || '';
}
