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

    const rawValue = toRaw(value);
    if (typeof structuredClone === 'function') {
        try {
            return structuredClone(rawValue);
        } catch (error) {
            if (!isStructuredCloneError(error)) {
                throw error;
            }
        }
    }

    return clonePlainStructuredValue(rawValue, new WeakMap()) as T;
}

function isStructuredCloneError(error: unknown): boolean {
    if (typeof DOMException !== 'undefined' && error instanceof DOMException) {
        return error.name === 'DataCloneError';
    }

    return typeof error === 'object'
        && error !== null
        && 'name' in error
        && error.name === 'DataCloneError';
}

function isNonCloneableHostObject(value: object): boolean {
    if (typeof window !== 'undefined' && value === window) {
        return true;
    }

    if (typeof Node !== 'undefined' && value instanceof Node) {
        return true;
    }

    const objectTag = Object.prototype.toString.call(value);
    return objectTag === '[object Window]' || objectTag === '[object global]';
}

function clonePlainStructuredValue(value: unknown, seen: WeakMap<object, unknown>): unknown {
    const rawValue = toRaw(value);

    if (rawValue === null || rawValue === undefined) {
        return rawValue;
    }

    if (typeof rawValue === 'function' || typeof rawValue === 'symbol') {
        return undefined;
    }

    if (typeof rawValue !== 'object') {
        return rawValue;
    }

    if (isNonCloneableHostObject(rawValue)) {
        return undefined;
    }

    const existingClone = seen.get(rawValue);
    if (existingClone) {
        return existingClone;
    }

    if (rawValue instanceof Date) {
        return new Date(rawValue.getTime());
    }

    if (Array.isArray(rawValue)) {
        const clonedArray: unknown[] = [];
        seen.set(rawValue, clonedArray);
        for (const item of rawValue) {
            clonedArray.push(clonePlainStructuredValue(item, seen));
        }
        return clonedArray;
    }

    const clonedObject: Record<string, unknown> = {};
    seen.set(rawValue, clonedObject);
    for (const [key, entryValue] of Object.entries(rawValue)) {
        const clonedEntry = clonePlainStructuredValue(entryValue, seen);
        if (clonedEntry !== undefined || entryValue === undefined) {
            clonedObject[key] = clonedEntry;
        }
    }
    return clonedObject;
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
