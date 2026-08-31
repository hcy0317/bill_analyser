import type {
    LLMCandidateItem,
    LLMConfigForm,
    LLMConfigItem,
    LLMProviderOption,
} from './types.ts';
import { createEmptyLLMConfigForm } from './options.ts';

export function createLLMConfigFormFromSaved(config: LLMConfigItem): LLMConfigForm {
    const defaults = createEmptyLLMConfigForm();
    const advanced = config.advanced_settings || {};
    const credentialMode = typeof config.credential_config?.['credential_mode'] === 'string'
        ? String(config.credential_config['credential_mode'])
        : 'api_key';
    const hasAdvancedOverrides = Boolean(
        advanced.reasoning_depth
        || advanced.temperature !== undefined
        || advanced.max_tokens !== undefined
        || advanced.system_prompt
        || advanced.classification_prompt_template
        || advanced.rule_prompt_template
    );
    const customPrompts = Boolean(
        advanced.system_prompt
        || advanced.classification_prompt_template
        || advanced.rule_prompt_template
    );
    const apiProtocol = ['responses', 'chat_completions'].includes(String(advanced.api_protocol))
        ? advanced.api_protocol as 'responses' | 'chat_completions'
        : 'auto';
    return {
        ...defaults,
        name: config.name,
        provider: config.provider,
        model: config.model,
        base_url: config.base_url || '',
        api_key: '',
        credential_mode: credentialMode,
        credential_json: '',
        api_protocol: apiProtocol,
        advancedMode: hasAdvancedOverrides,
        customPrompts,
        reasoning_depth: advanced.reasoning_depth || '',
        temperature: advanced.temperature !== undefined
            ? String(advanced.temperature)
            : defaults.temperature,
        max_tokens: advanced.max_tokens !== undefined
            ? String(advanced.max_tokens)
            : defaults.max_tokens,
        system_prompt: advanced.system_prompt || defaults.system_prompt,
        classification_prompt_template: advanced.classification_prompt_template
            || defaults.classification_prompt_template,
        rule_prompt_template: advanced.rule_prompt_template || defaults.rule_prompt_template,
    };
}

/**
 * 将配置列表响应转成前端模型，并在进入组件状态前移除 api_key。
 */
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

/**
 * 归一 LLM 候选响应，兼容数组响应和 `{ candidates }` 包装响应。
 */
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
        const accountName = typeof record['suggested_account_name'] === 'string'
            ? record['suggested_account_name']
            : '';
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
            target_name: accountName || categoryName || undefined,
            suggested_main_category: mainCategory || undefined,
            suggested_sub_category: subCategory || undefined,
            suggested_account_id: Number(record['suggested_account_id'] || 0) || undefined,
            suggested_account_name: accountName || undefined,
            suggested_rule_expression: suggestedRuleExpression || undefined,
        };
    });
}

/**
 * 解析 provider 展示名，兼容历史 provider key 和当前下拉配置。
 */
export function getLLMProviderLabel(provider: string, options: LLMProviderOption[]): string {
    const providerLabels: Record<string, string> = {
        anthropic: 'Claude (Anthropic)',
        'openai-compatible': 'OpenAI-compatible',
        azure_openai: 'Azure OpenAI',
    };

    return options.find(option => option.value === provider)?.title
        ?? providerLabels[provider]
        ?? provider;
}
