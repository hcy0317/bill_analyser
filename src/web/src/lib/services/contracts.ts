import type { ImportPreviewExpectedState } from '@/models/import_preview.ts';

export interface AnalyzeLLMTransactionsRequest {
    billIds?: number[];
    limit?: number;
    sessionId?: string;
    previewIds?: number[];
    previewUpdates?: Array<Record<string, unknown>>;
    actionScope?: Record<string, unknown>;
}

interface LLMAdvancedSettings {
    reasoning_depth?: string;
    temperature?: number;
    max_tokens?: number;
    system_prompt?: string;
    classification_prompt_template?: string;
    rule_prompt_template?: string;
}

export interface CreateLLMConfigRequest {
    name: string;
    provider: string;
    model: string;
    api_key?: string;
    base_url?: string;
    credential_config?: Record<string, unknown>;
    is_active?: boolean;
    advanced_settings?: LLMAdvancedSettings;
}

export interface OCRConfigResponse {
    provider: string;
    lang: string;
    model?: string;
    base_url?: string;
    parameters?: Record<string, unknown>;
    credential_config?: Record<string, unknown>;
    available_providers: string[];
    configured: boolean;
    server_setup?: {
        local_model?: {
            provider: string;
            configured: boolean;
            bundled: boolean;
            display_name: string;
            model: string;
        };
    };
}

export type OCRConfigUpdateRequest = Pick<
    OCRConfigResponse,
    'provider' | 'lang' | 'model' | 'base_url' | 'parameters' | 'credential_config'
>;

export interface LLMAnalyzeTransactionCandidate {
    id: number;
    session_id?: string;
    source_preview_ids?: number[];
    rule_name?: string;
    rule_expression?: string;
    confidence?: number;
    category_name?: string;
    explanation?: string;
}

export interface LLMAnalyzeTransactionsResponse {
    candidates_created: number;
    candidates: LLMAnalyzeTransactionCandidate[];
    session_id?: string;
    mode?: 'import_session' | 'persisted_selection' | 'persisted_uncategorized';
}

export interface LLMRuleSynthesisResponse {
    candidates_created: number;
    candidates: LLMAnalyzeTransactionCandidate[];
    mode?: 'rule_synthesis';
    knowledge_summary_pack?: Record<string, unknown>;
}

export interface LLMMemoryEventsResponse {
    events: Array<Record<string, unknown>>;
    total: number;
}

export interface BudgetHistoryQueryRequest {
    type?: number;
    periodType?: string;
    year?: number;
    month?: number;
    quarter?: number;
    startDate?: string;
    endDate?: string;
    budgetId?: string;
    categoryId?: string;
    accountIds?: string[];
    tagIds?: string[];
}

export interface BudgetForecastQueryRequest {
    type?: number;
    periodType?: string;
    year?: number;
    month?: number;
    quarter?: number;
    monthsHistory?: number;
    forecastStrategy?: string;
    startDate?: string;
    endDate?: string;
}

export interface PagedStatusRequest {
    status?: string;
    limit?: number;
    offset?: number;
}

export interface LearningRuleListRequest {
    enabledOnly?: boolean;
    limit?: number;
    offset?: number;
}

export interface CalendarEventsRequest {
    startDate: string;
    endDate: string;
}

export interface AccountRuleCreateRequest {
    account_id: number;
    name: string;
    priority: number;
    rule_expression: string;
    regex_enabled?: boolean;
    enabled?: boolean;
}

export interface LLMRuleSynthesisRequest {
    limit?: number;
}

export interface LLMPreviewRecommendRequest {
    sessionId: string;
    previewIds?: number[];
    previewUpdates?: Record<string, any>[];
    actionScope?: Record<string, unknown>;
    limit?: number;
}

export interface LLMPreviewRecommendAcceptRequest {
    sessionId: string;
    previewId: number;
    suggestion: Record<string, any>;
    expectedState?: ImportPreviewExpectedState;
}

export interface LLMPreviewRecommendRejectRequest extends LLMPreviewRecommendAcceptRequest {
    userCorrection?: Record<string, any>;
}

export interface AnomalyListRequest {
    months?: number;
}
