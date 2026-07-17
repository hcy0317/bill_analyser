import type { AxiosRequestConfig, AxiosRequestHeaders, AxiosResponse } from 'axios';

import type { ApiResponse } from '@/core/api.ts';

export interface ApiRequestConfig extends AxiosRequestConfig {
    headers: AxiosRequestHeaders;
    readonly noAuth?: boolean;
    readonly preserveExplicitAuthorization?: boolean;
    readonly ignoreBlocked?: boolean;
    readonly ignoreError?: boolean;
    readonly timeout?: number;
    readonly cancelableUuid?: string;
}

export type ApiResponsePromise<T> = Promise<AxiosResponse<ApiResponse<T>>>;

export interface ApiDataResponse<T> {
    success: boolean;
    data: T;
}

/**
 * 中文说明：把新后端 data envelope 适配回旧前端 `ApiResponse.result` 形态，避免拆分 services 时改变调用方契约。
 */
export function buildApiResponse<T>(response: AxiosResponse<any>, result: T): AxiosResponse<ApiResponse<T>> {
    return {
        ...response,
        data: {
            success: response.data?.success ?? false,
            result
        }
    } as AxiosResponse<ApiResponse<T>>;
}
