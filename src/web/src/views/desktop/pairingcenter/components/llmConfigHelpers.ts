export interface LLMConfigItem {
    id: number;
    name: string;
    provider: string;
    model: string;
    base_url?: string;
    advanced_settings?: LLMAdvancedSettings;
    is_active?: boolean;
    created_at?: string;
    updated_at?: string;
}

export interface LLMAdvancedSettings {
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
    advancedMode: boolean;
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
    requiresBaseUrl?: boolean;
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
    suggested_rule_expression?: string;
}

export interface SelectOption {
    title: string;
    value: string;
}

type Translate = (key: string) => string;

export function createLLMProviderOptions(tt: Translate): LLMProviderOption[] {
    return [
        {
            title: 'OpenAI',
            value: 'openai',
            modelPlaceholder: 'gpt-4o-mini',
            apiKeyPlaceholder: 'sk-...',
            baseUrlPlaceholder: 'https://api.openai.com/v1',
        },
        {
            title: 'Claude (Anthropic)',
            value: 'claude',
            modelPlaceholder: 'claude-sonnet-4-20250514',
            apiKeyPlaceholder: 'sk-ant-...',
            baseUrlPlaceholder: 'https://api.anthropic.com/v1',
        },
        {
            title: 'DeepSeek',
            value: 'deepseek',
            modelPlaceholder: 'deepseek-chat',
            apiKeyPlaceholder: 'sk-...',
            baseUrlPlaceholder: 'https://api.deepseek.com/v1',
        },
        {
            title: 'Ollama (local)',
            value: 'ollama',
            modelPlaceholder: 'llama3.1',
            apiKeyPlaceholder: tt('Not required'),
            baseUrlPlaceholder: 'http://localhost:11434',
        },
        {
            title: 'xAI',
            value: 'xai',
            modelPlaceholder: 'grok-3-mini',
            apiKeyPlaceholder: 'xai-...',
            baseUrlPlaceholder: 'https://api.x.ai/v1',
        },
        {
            title: 'Google (Gemini)',
            value: 'google',
            modelPlaceholder: 'gemini-2.0-flash',
            apiKeyPlaceholder: 'AIza...',
            baseUrlPlaceholder: 'https://generativelanguage.googleapis.com/v1beta/openai',
        },
        {
            title: 'OpenRouter',
            value: 'openrouter',
            modelPlaceholder: 'openai/gpt-4o-mini',
            apiKeyPlaceholder: 'sk-or-...',
            baseUrlPlaceholder: 'https://openrouter.ai/api/v1',
        },
        {
            title: 'OpenAI-compatible',
            value: 'openai_compatible',
            modelPlaceholder: 'gpt-4o-mini',
            apiKeyPlaceholder: 'sk-...',
            baseUrlPlaceholder: 'https://your-provider.example.com/v1',
            requiresBaseUrl: true,
        },
        {
            title: 'Azure OpenAI',
            value: 'azure',
            modelPlaceholder: 'deployment-name',
            apiKeyPlaceholder: 'Azure API key',
            baseUrlPlaceholder: 'https://<resource>.openai.azure.com/openai/v1',
            requiresBaseUrl: true,
        },
    ];
}

export function createLLMReasoningDepthOptions(tt: Translate): SelectOption[] {
    return [
        { title: tt('Default'), value: '' },
        { title: tt('Low'), value: 'low' },
        { title: tt('Medium'), value: 'medium' },
        { title: tt('High'), value: 'high' },
    ];
}

export function createEmptyLLMConfigForm(): LLMConfigForm {
    return {
        name: '',
        provider: 'openai',
        model: '',
        api_key: '',
        base_url: '',
        advancedMode: false,
        reasoning_depth: '',
        temperature: '0.3',
        max_tokens: '4096',
        system_prompt: '',
        classification_prompt_template: '',
        rule_prompt_template: '',
    };
}

export function buildAdvancedSettingsPayload(form: LLMConfigForm): LLMAdvancedSettings {
    if (!form.advancedMode) {
        return {};
    }

    const settings: LLMAdvancedSettings = {};
    if (form.reasoning_depth) {
        settings.reasoning_depth = form.reasoning_depth;
    }

    const temperature = Number(form.temperature);
    if (Number.isFinite(temperature)) {
        settings.temperature = temperature;
    }

    const maxTokens = Number.parseInt(form.max_tokens, 10);
    if (Number.isFinite(maxTokens)) {
        settings.max_tokens = maxTokens;
    }

    const systemPrompt = form.system_prompt.trim();
    if (systemPrompt) {
        settings.system_prompt = systemPrompt;
    }

    const classificationPrompt = form.classification_prompt_template.trim();
    if (classificationPrompt) {
        settings.classification_prompt_template = classificationPrompt;
    }

    const rulePrompt = form.rule_prompt_template.trim();
    if (rulePrompt) {
        settings.rule_prompt_template = rulePrompt;
    }

    return settings;
}

export function toLLMConfigs(result: unknown): LLMConfigItem[] {
    if (!Array.isArray(result)) {
        return [];
    }

    return result.map((item) => {
        const safeItem = { ...(item as Record<string, unknown>) };
        delete safeItem['api_key'];
        return safeItem as unknown as LLMConfigItem;
    });
}

export function toLLMCandidates(result: unknown): LLMCandidateItem[] {
    const rawItems = Array.isArray(result)
        ? result
        : (result && typeof result === 'object' && 'candidates' in result && Array.isArray((result as { candidates?: unknown }).candidates)
            ? (result as { candidates: unknown[] }).candidates
            : []);

    return rawItems.map((item) => {
        const record = (item && typeof item === 'object') ? item as Record<string, unknown> : {};
        const mainCategory = typeof record['suggested_main_category'] === 'string'
            ? record['suggested_main_category']
            : '';
        const subCategory = typeof record['suggested_sub_category'] === 'string'
            ? record['suggested_sub_category']
            : '';
        const categoryName = typeof record['category_name'] === 'string' && record['category_name']
            ? record['category_name']
            : [mainCategory, subCategory].filter(Boolean).join('/');
        const suggestedRuleExpression = typeof record['suggested_rule_expression'] === 'string'
            ? record['suggested_rule_expression']
            : '';
        const llmResponseRaw = typeof record['llm_response_raw'] === 'string'
            ? record['llm_response_raw']
            : '';
        let parsedReason = '';
        if (llmResponseRaw) {
            try {
                const parsed = JSON.parse(llmResponseRaw) as Record<string, unknown>;
                const payloadReason = parsed['reason'];
                const payloadExplanation = parsed['explanation'];
                parsedReason = typeof payloadReason === 'string'
                    ? payloadReason
                    : (typeof payloadExplanation === 'string' ? payloadExplanation : '');
            } catch {
                parsedReason = '';
            }
        }

        return {
            id: Number(record['id'] || 0),
            type: typeof record['type'] === 'string' ? record['type'] : undefined,
            status: typeof record['status'] === 'string' ? record['status'] : 'pending',
            confidence: typeof record['confidence'] === 'number'
                ? record['confidence']
                : Number(record['confidence'] || 0),
            rule_type: typeof record['rule_type'] === 'string'
                ? record['rule_type']
                : (typeof record['type'] === 'string' ? record['type'] : undefined),
            rule_content: typeof record['rule_content'] === 'string' && record['rule_content']
                ? record['rule_content']
                : suggestedRuleExpression,
            expression: typeof record['expression'] === 'string' && record['expression']
                ? record['expression']
                : suggestedRuleExpression,
            reason: typeof record['reason'] === 'string' && record['reason']
                ? record['reason']
                : parsedReason,
            category_name: categoryName || undefined,
            target_category: typeof record['target_category'] === 'string'
                ? record['target_category']
                : categoryName || undefined,
            suggested_main_category: mainCategory || undefined,
            suggested_sub_category: subCategory || undefined,
            suggested_rule_expression: suggestedRuleExpression || undefined,
        };
    });
}

export function getLLMProviderLabel(provider: string, options: LLMProviderOption[]): string {
    const legacyLabels: Record<string, string> = {
        anthropic: 'Claude (Anthropic)',
        'openai-compatible': 'OpenAI-compatible',
        azure_openai: 'Azure OpenAI',
    };

    return options.find(option => option.value === provider)?.title
        ?? legacyLabels[provider]
        ?? provider;
}
