export interface LLMConfigItem {
    id: number;
    name: string;
    provider: string;
    model: string;
    base_url?: string;
    credential_config?: Record<string, unknown>;
    advanced_settings?: LLMAdvancedSettings;
    is_active?: boolean;
    created_at?: string;
    updated_at?: string;
}

export interface LLMAdvancedSettings {
    api_protocol?: 'auto' | 'responses' | 'chat_completions';
    reasoning_depth?: string;
    temperature?: number;
    max_tokens?: number;
    system_prompt?: string;
    classification_prompt_template?: string;
    rule_prompt_template?: string;
}

export interface LLMConfigForm {
    name: string;
    provider: string;
    model: string;
    api_key: string;
    base_url: string;
    credential_mode: string;
    credential_json: string;
    api_protocol: 'auto' | 'responses' | 'chat_completions';
    advancedMode: boolean;
    customPrompts: boolean;
    reasoning_depth: string;
    temperature: string;
    max_tokens: string;
    system_prompt: string;
    classification_prompt_template: string;
    rule_prompt_template: string;
}

export interface LLMProviderOption {
    title: string;
    value: string;
    modelPlaceholder: string;
    apiKeyPlaceholder: string;
    baseUrlPlaceholder: string;
    defaultModel?: string;
    defaultBaseUrl?: string;
    requiresBaseUrl?: boolean;
    description: string;
    location: 'cloud' | 'local' | 'custom';
    recommended?: boolean;
    apiKeyRequired?: boolean;
}

export interface LLMCandidateItem {
    id: number;
    type?: string;
    status: string;
    confidence?: number;
    rule_type?: string;
    rule_content?: string;
    expression?: string;
    reason?: string;
    category_name?: string;
    target_category?: string;
    suggested_main_category?: string;
    suggested_sub_category?: string;
    suggested_account_id?: number;
    suggested_account_name?: string;
    target_name?: string;
    suggested_rule_expression?: string;
}

export interface SelectOption {
    title: string;
    value: string;
}

export type Translate = (key: string) => string;
