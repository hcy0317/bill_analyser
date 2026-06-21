import axios from 'axios';

import type { ApiResponse, ErrorResponse } from '@/core/api.ts';

/**
 * 从后端错误载荷中递归提取可展示的错误文本。
 *
 * 规则中心多个面板都会接收 `{ error: { message } }`、`{ message }` 或纯字符串错误。
 * 这里把解析深度限制在 3 层，避免异常载荷导致递归过深。
 */
export function extractPayloadMessage(payload: unknown, depth = 0): string | null {
    if (depth > 2) {
        return null;
    }

    if (typeof payload === 'string' && payload) {
        return payload;
    }

    if (!payload || typeof payload !== 'object') {
        return null;
    }

    const typedPayload = payload as Partial<ErrorResponse> & {
        error?: unknown;
        message?: unknown;
    };

    return extractPayloadMessage(
        typedPayload.error ?? typedPayload.message,
        depth + 1
    );
}

/**
 * 将 axios、普通 Error 和未知异常统一转换成页面可展示错误文案。
 */
export function getRequestErrorMessage(error: unknown, fallback: string): string {
    if (axios.isAxiosError(error)) {
        return extractPayloadMessage(error.response?.data) || error.message || fallback;
    }

    if (error instanceof Error && error.message) {
        return error.message;
    }

    return fallback;
}

/**
 * 校验标准 API 响应并返回业务结果，失败时抛出调用方提供的兜底错误。
 */
export function requireApiSuccess<T>(response: { data?: ApiResponse<T> }, fallback: string): T {
    if (response.data?.success) {
        return response.data.result;
    }

    throw new Error(fallback);
}
