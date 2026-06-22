import { normalizeStrictAbsoluteCents } from './strictCents.ts';

export interface ImportCheckLearningPreviewTextSyncState {
    previewId?: number | null;
    counterparty?: string;
    paymentMethod?: string;
    comment?: string;
    selected?: boolean;
}

export interface ImportCheckLearningPreviewTextSyncPayload {
    id: number;
    counterparty: string;
    paymentMethod: string;
    description: string;
    isSelected: boolean;
}

export interface ImportCheckLearningDecisionBaseline {
    inputFingerprint: string;
    type: number;
    categoryId: string;
    recurringTemplateId: string;
    sourceAccountId: string;
    destinationAccountId: string;
}

function normalizePreviewId(previewId?: number | null): number | null {
    if (typeof previewId !== 'number' || !Number.isInteger(previewId) || previewId <= 0) {
        return null;
    }

    return previewId;
}

/**
 * 构造学习建议文本同步 payload，preview id 无效时返回 null 以阻止无效提交。
 */
export function buildImportCheckLearningPreviewTextSyncPayload(
    state: ImportCheckLearningPreviewTextSyncState
): ImportCheckLearningPreviewTextSyncPayload | null {
    const previewId = normalizePreviewId(state.previewId);
    if (previewId === null) {
        return null;
    }

    return {
        id: previewId,
        counterparty: state.counterparty || '',
        paymentMethod: state.paymentMethod || '',
        description: state.comment || '',
        isSelected: !!state.selected
    };
}

/**
 * 判断学习建议输入文本是否发生漂移，用于阻止基于旧文本的决策继续应用。
 */
export function hasImportCheckLearningTextDrift(
    baseline: ImportCheckLearningDecisionBaseline,
    current: ImportCheckLearningDecisionBaseline
): boolean {
    return baseline.inputFingerprint !== current.inputFingerprint;
}

/**
 * 判断学习建议期望状态是否漂移，覆盖类型、分类、周期和账户字段。
 */
export function hasImportCheckLearningExpectedStateDrift(
    baseline: ImportCheckLearningDecisionBaseline,
    current: ImportCheckLearningDecisionBaseline
): boolean {
    return baseline.type !== current.type
        || baseline.categoryId !== current.categoryId
        || baseline.recurringTemplateId !== current.recurringTemplateId
        || baseline.sourceAccountId !== current.sourceAccountId
        || baseline.destinationAccountId !== current.destinationAccountId;
}

/**
 * 归一化导入预览金额分值，保持学习链路使用严格绝对分合同。
 */
export function normalizeImportPreviewAmountCents(amountInCents: unknown, fallbackInCents: number): number {
    return normalizeStrictAbsoluteCents(amountInCents, fallbackInCents);
}
