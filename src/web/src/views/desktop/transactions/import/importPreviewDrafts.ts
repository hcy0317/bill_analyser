import { toRaw } from 'vue';

type ImportPreviewDraftCloneable = {
    tagIds?: string[];
    originalTagNames?: string[];
    parserTags?: string[];
    dedupSourceIds?: Array<number | string>;
    geoLocation?: unknown;
    matching?: unknown;
    _previewDecisionBaseline?: unknown;
    _learningDecisionBaseline?: unknown;
};

function cloneStructuredValue<T>(value: T): T {
    if (value === undefined || value === null) {
        return value;
    }

    return structuredClone(toRaw(value));
}

export function cloneImportPreviewDraftTransaction<T extends object & ImportPreviewDraftCloneable>(transaction: T): T {
    const rawTransaction = toRaw(transaction) as T;
    const clonedTransaction = Object.assign(
        Object.create(Object.getPrototypeOf(rawTransaction)),
        rawTransaction
    ) as T;

    clonedTransaction.tagIds = [...(rawTransaction.tagIds || [])] as T['tagIds'];
    clonedTransaction.originalTagNames = [...(rawTransaction.originalTagNames || [])] as T['originalTagNames'];
    clonedTransaction.parserTags = [...(rawTransaction.parserTags || [])] as T['parserTags'];
    clonedTransaction.dedupSourceIds = [...(rawTransaction.dedupSourceIds || [])] as T['dedupSourceIds'];

    if (rawTransaction.geoLocation !== undefined) {
        clonedTransaction.geoLocation = cloneStructuredValue(rawTransaction.geoLocation) as T['geoLocation'];
    }

    if (rawTransaction.matching !== undefined) {
        clonedTransaction.matching = cloneStructuredValue(rawTransaction.matching) as T['matching'];
    }

    if (rawTransaction._previewDecisionBaseline !== undefined) {
        clonedTransaction._previewDecisionBaseline = cloneStructuredValue(
            rawTransaction._previewDecisionBaseline
        ) as T['_previewDecisionBaseline'];
    }

    if (rawTransaction._learningDecisionBaseline !== undefined) {
        clonedTransaction._learningDecisionBaseline = cloneStructuredValue(
            rawTransaction._learningDecisionBaseline
        ) as T['_learningDecisionBaseline'];
    }

    return clonedTransaction;
}
