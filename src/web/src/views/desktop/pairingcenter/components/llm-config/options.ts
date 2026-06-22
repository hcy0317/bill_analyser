import type {
    LLMConfigForm,
    LLMProviderOption,
    SelectOption,
    Translate,
} from './types.ts';

/**
 * 构造规则中心 LLM provider 下拉选项和默认占位符，不参与实际 provider 调用。
 */
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

/**
 * 构造推理深度选项，保持前端值与后端 advanced settings 字段一致。
 */
export function createLLMReasoningDepthOptions(tt: Translate): SelectOption[] {
    return [
        { title: tt('Default'), value: '' },
        { title: tt('Low'), value: 'low' },
        { title: tt('Medium'), value: 'medium' },
        { title: tt('High'), value: 'high' },
    ];
}

/**
 * 构造凭据模式选项，覆盖 API key、JSON 凭据和 token 刷新等保存形态。
 */
export function createLLMCredentialModeOptions(tt: Translate): SelectOption[] {
    return [
        { title: tt('API Key'), value: 'api_key' },
        { title: tt('Session JSON'), value: 'session_json' },
        { title: tt('Auth JSON'), value: 'auth_json' },
        { title: tt('Account JSON'), value: 'account_json' },
        { title: tt('Sub2API JSON'), value: 'sub2api_json' },
        { title: tt('Access Token'), value: 'access_token' },
        { title: tt('Refresh Token'), value: 'refresh_token' },
    ];
}

/**
 * 创建新增配置表单的默认值，确保 credential 和 advanced settings 字段都有稳定初始值。
 */
export function createEmptyLLMConfigForm(): LLMConfigForm {
    return {
        name: '',
        provider: 'openai',
        model: '',
        api_key: '',
        base_url: '',
        credential_mode: 'api_key',
        credential_json: '',
        token_endpoint: '',
        refresh_headers: '',
        refresh_body: '',
        refresh_params: '',
        advancedMode: false,
        reasoning_depth: '',
        temperature: '0.3',
        max_tokens: '4096',
        system_prompt: '',
        classification_prompt_template: '',
        rule_prompt_template: '',
    };
}
