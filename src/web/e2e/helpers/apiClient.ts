import type { APIRequestContext, APIResponse } from '@playwright/test';
import type { ReadStream } from 'fs';

import { apiURL, type E2EEnvironment } from './env';

export interface ApiEnvelope<T> {
    readonly success?: boolean;
    readonly result?: T;
    readonly data?: T;
    readonly message?: string;
    readonly errorCode?: number;
}

export class E2EApiError extends Error {
    public readonly status: number;
    public readonly body: unknown;

    public constructor(message: string, status: number, body: unknown) {
        super(message);
        this.name = 'E2EApiError';
        this.status = status;
        this.body = body;
    }
}

type MultipartValue = string | number | boolean | ReadStream | { name: string; mimeType: string; buffer: Buffer };

export class E2EApiClient {
    private token: string | null = null;

    public constructor(
        private readonly request: APIRequestContext,
        private readonly env: E2EEnvironment
    ) {
    }

    public setToken(token: string): void {
        this.token = token;
    }

    public async get<T>(path: string): Promise<T> {
        return this.unwrap<T>(await this.request.get(apiURL(path, this.env), {
            headers: this.authHeaders()
        }));
    }

    public async post<T>(path: string, data?: unknown): Promise<T> {
        return this.unwrap<T>(await this.request.post(apiURL(path, this.env), {
            data,
            headers: this.authHeaders()
        }));
    }

    public async put<T>(path: string, data?: unknown): Promise<T> {
        return this.unwrap<T>(await this.request.put(apiURL(path, this.env), {
            data,
            headers: this.authHeaders()
        }));
    }

    public async delete<T>(path: string): Promise<T> {
        return this.unwrap<T>(await this.request.delete(apiURL(path, this.env), {
            headers: this.authHeaders()
        }));
    }

    public async postMultipart<T>(
        path: string,
        multipart: Record<string, MultipartValue>
    ): Promise<T> {
        return this.unwrap<T>(await this.request.post(apiURL(path, this.env), {
            multipart,
            headers: this.authHeaders()
        }));
    }

    private authHeaders(): Record<string, string> {
        if (!this.token) {
            return {};
        }

        return {
            Authorization: `Bearer ${this.token}`
        };
    }

    private async unwrap<T>(response: APIResponse): Promise<T> {
        const body = await parseResponseBody(response);
        if (!response.ok()) {
            throw new E2EApiError(`HTTP ${response.status()} from ${response.url()}`, response.status(), body);
        }

        if (isApiEnvelope<T>(body)) {
            if (body.success === false) {
                throw new E2EApiError(body.message || `API failure from ${response.url()}`, response.status(), body);
            }

            if ('result' in body) {
                return body.result as T;
            }

            if ('data' in body) {
                return body.data as T;
            }

            return body as T;
        }

        return body as T;
    }
}

async function parseResponseBody(response: APIResponse): Promise<unknown> {
    const text = await response.text();
    if (!text) {
        return null;
    }

    try {
        return JSON.parse(text) as unknown;
    } catch {
        return text;
    }
}

function isApiEnvelope<T>(value: unknown): value is ApiEnvelope<T> {
    return typeof value === 'object' && value !== null && 'success' in value;
}
