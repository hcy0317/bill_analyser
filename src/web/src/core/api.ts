export interface ApiResponse<T> {
    readonly success: boolean;
    readonly result: T;
}

export interface ErrorResponse {
    readonly success: boolean;
    readonly errorCode: number;
    readonly message: string;
    readonly path: string;
}

export function buildErrorResponse(errorCode: number, message: string): ErrorResponse {
    const errorResponse: ErrorResponse = {
        success: false,
        errorCode: errorCode,
        message,
        path: ''
    };

    return errorResponse;
}
