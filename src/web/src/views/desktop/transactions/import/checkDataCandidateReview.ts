import { TransactionType } from '@/core/transaction.ts';

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
    return Number.isNaN(parsedValue) ? null : parsedValue;
}

interface ImportCheckDecisionExpectedStateInput {
    sessionId: string;
    reviewStatus: string;
    type: number;
    categoryId?: string | null;
    recurringTemplateId?: string | null;
}

interface ImportCheckLearningDecisionExpectedStateInput extends ImportCheckDecisionExpectedStateInput {
    sourceAccountId?: string | null;
    destinationAccountId?: string | null;
}

export function buildImportCheckDecisionExpectedState(
    input: ImportCheckDecisionExpectedStateInput
): Record<string, string | number | null> {
    return {
        sessionId: input.sessionId,
        reviewStatus: input.reviewStatus,
        previewType: getImportCheckPreviewTypeText(input.type),
        categoryId: parseOptionalId(input.categoryId),
        recurringId: parseOptionalId(input.recurringTemplateId)
    };
}

export function buildImportCheckLearningDecisionExpectedState(
    input: ImportCheckLearningDecisionExpectedStateInput
): Record<string, string | number | null> {
    return {
        ...buildImportCheckDecisionExpectedState(input),
        sourceAccountId: parseOptionalId(input.sourceAccountId),
        destinationAccountId: parseOptionalId(input.destinationAccountId)
    };
}
