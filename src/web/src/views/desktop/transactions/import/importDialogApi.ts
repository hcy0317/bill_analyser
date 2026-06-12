import { DEFAULT_UPLOAD_API_TIMEOUT } from '@/consts/api.ts';

export function extractApiErrorMessage(payload: unknown, fallbackMessage: string): string {
    if (!payload || typeof payload !== 'object') {
        return fallbackMessage;
    }

    const apiError = payload as {
        error?: string;
        message?: string;
    };

    return apiError.error || apiError.message || fallbackMessage;
}

export function isAbortError(error: unknown): boolean {
    return (error instanceof DOMException && error.name === 'AbortError')
        || (!!error && typeof error === 'object' && 'name' in error && error.name === 'AbortError');
}

export function formatTimeoutSeconds(timeoutMs: number): string {
    return `${Math.round(timeoutMs / 1000)}秒`;
}

export async function fetchImportStage(url: string, init: RequestInit, stageLabel: string, timeoutMs = DEFAULT_UPLOAD_API_TIMEOUT): Promise<Response> {
    const controller = new AbortController();
    const externalSignal = init.signal;
    let externalAborted = false;
    const abortFromExternalSignal = () => {
        externalAborted = true;
        controller.abort();
    };
    const timeoutId = window.setTimeout(() => controller.abort(), timeoutMs);

    if (externalSignal) {
        if (externalSignal.aborted) {
            abortFromExternalSignal();
        } else {
            externalSignal.addEventListener('abort', abortFromExternalSignal, { once: true });
        }
    }

    try {
        return await fetch(url, {
            ...init,
            signal: controller.signal
        });
    } catch (error) {
        if (isAbortError(error)) {
            if (externalAborted) {
                throw error;
            }
            throw new Error(`${stageLabel}客户端等待超时（${formatTimeoutSeconds(timeoutMs)}），请检查后端 parser/import 日志确认是否仍在解析或写入`);
        }
        throw error;
    } finally {
        window.clearTimeout(timeoutId);
        externalSignal?.removeEventListener('abort', abortFromExternalSignal);
    }
}
