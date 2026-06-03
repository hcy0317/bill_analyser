import { isObject, isString } from '@/lib/common.ts';

function stringField(value: unknown): string | null {
    if (!isString(value)) {
        return null;
    }

    const trimmed = value.trim();
    return trimmed ? trimmed : null;
}

function objectField(value: object, key: string): unknown {
    return (value as Record<string, unknown>)[key];
}

function responseDataMessage(data: unknown): string | null {
    if (isString(data)) {
        return stringField(data);
    }

    if (!isObject(data)) {
        return null;
    }

    return stringField(objectField(data, 'error'))
        ?? stringField(objectField(data, 'message'));
}

export function getApiErrorMessage(error: unknown): string | null {
    if (isObject(error)) {
        const response = objectField(error, 'response');
        if (isObject(response)) {
            const message = responseDataMessage(objectField(response, 'data'));
            if (message) {
                return message;
            }
        }

        const directMessage = stringField(objectField(error, 'message'));
        if (directMessage) {
            return directMessage;
        }
    }

    return stringField(error);
}

export function getApiErrorMessageOrDefault(error: unknown, fallback: string): string {
    return getApiErrorMessage(error) || fallback;
}
