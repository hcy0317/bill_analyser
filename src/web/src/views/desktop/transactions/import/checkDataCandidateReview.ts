import { TransactionType } from '@/core/transaction.ts';
import type { ImportPreviewExpectedState } from '@/models/import_preview.ts';

function getImportCheckPreviewTypeText(type: number): string {
    if (type === TransactionType.Income) {
        return '收入';
    }
    if (type === TransactionType.Expense) {
        return '支出';
    }
    if (type === TransactionType.Transfer) {
        return '转账';
    }
    if (type === TransactionType.Investment) {
        return '投资';
    }

    return '支出';
}

function parseOptionalId(value: string | null | undefined): number | null {
    if (!value) {
        return null;
    }

    const parsedValue = parseInt(value, 10);
    return Number.isNaN(parsedValue) || parsedValue <= 0 ? null : parsedValue;
}

interface ImportCheckDecisionExpectedStateInput {
    sessionId: string;
    reviewStatus: string;
    type: number;
    categoryId?: string | null;
    recurringTemplateId?: string | null;
    sourceAccountId?: string | null;
    destinationAccountId?: string | null;
}

type ImportCheckLearningDecisionExpectedStateInput = ImportCheckDecisionExpectedStateInput & {
    rowVersion?: number;
};

export function withImportPreviewRowVersion<T extends ImportPreviewExpectedState>(
    expectedState: T,
    rowVersion: unknown
): T & { rowVersion?: number } {
    if (Number.isInteger(rowVersion) && Number(rowVersion) > 0) {
        return { ...expectedState, rowVersion: Number(rowVersion) };
    }
    return { ...expectedState };
}

export function buildImportCheckDecisionExpectedState(
    input: ImportCheckDecisionExpectedStateInput
): ImportPreviewExpectedState {
    return {
        sessionId: input.sessionId,
        reviewStatus: input.reviewStatus,
        previewType: getImportCheckPreviewTypeText(input.type),
        categoryId: parseOptionalId(input.categoryId),
        recurringId: parseOptionalId(input.recurringTemplateId),
        sourceAccountId: parseOptionalId(input.sourceAccountId),
        destinationAccountId: parseOptionalId(input.destinationAccountId)
    };
}

export function buildImportCheckLearningDecisionExpectedState(
    input: ImportCheckLearningDecisionExpectedStateInput
): ImportPreviewExpectedState {
    return withImportPreviewRowVersion(
        buildImportCheckDecisionExpectedState(input),
        input.rowVersion
    );
}
