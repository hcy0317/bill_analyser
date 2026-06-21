import type { ImportConfigMatchResult } from './types.ts';

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

export function looksLikeStructuredBillStatementFile(file?: File): boolean {
    const fileName = file?.name?.toLowerCase() || '';
    return /微信支付账单|wechat|wxpay|支付宝交易明细|alipay/.test(fileName);
}

export function resolveImportConfigFileFormat(file?: File): string {
    const lowerName = file?.name.toLowerCase() || '';
    if (lowerName.endsWith('.csv') || lowerName.endsWith('.txt')) {
        return 'csv';
    }
    if (lowerName.endsWith('.xlsx') || lowerName.endsWith('.xls')) {
        return 'excel';
    }
    return 'csv';
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
