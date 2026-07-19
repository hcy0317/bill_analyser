import { describe, expect, test } from '@jest/globals';

import {
    buildAdvancedSettingsPayload,
    buildCredentialConfigPayload,
    createEmptyLLMConfigForm,
    createLLMProviderOptions,
    getLLMProviderLabel,
    toLLMCandidates,
    toLLMConfigs,
    type LLMConfigForm,
} from '@/views/desktop/pairingcenter/components/llmConfigHelpers.ts';

function makeForm(overrides: Partial<LLMConfigForm> = {}): LLMConfigForm {
    return {
        ...createEmptyLLMConfigForm(),
        ...overrides,
    };
}

describe('LLM config helper contracts', () => {
    const tt = (key: string) => `t:${key}`;

    test('builds provider options with stable labels and base-url requirements', () => {
        const options = createLLMProviderOptions(tt);

        expect(options.map(option => option.value)).toEqual([
            'openai',
            'claude',
            'deepseek',
            'qwen',
            'siliconflow',
            'zhipu',
            'ollama',
            'xai',
            'google',
            'openrouter',
            'openai_compatible',
            'azure',
        ]);
        expect(options.find(option => option.value === 'openai_compatible')).toMatchObject({
            requiresBaseUrl: true,
            baseUrlPlaceholder: 'https://your-provider.example.com/v1',
        });
        expect(options.find(option => option.value === 'azure')).toMatchObject({
            requiresBaseUrl: true,
            modelPlaceholder: 'deployment-name',
        });
        expect(options.find(option => option.value === 'qwen')).toMatchObject({
            modelPlaceholder: 'qwen-plus',
            baseUrlPlaceholder: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
        });
        expect(getLLMProviderLabel('anthropic', options)).toBe('Claude (Anthropic)');
        expect(getLLMProviderLabel('openai-compatible', options)).toBe('OpenAI-compatible');
        expect(getLLMProviderLabel('unknown-provider', options)).toBe('unknown-provider');
    });

    test('builds OAuth credential payload from one pasted JSON document', () => {
        const payload = buildCredentialConfigPayload(makeForm({
            credential_mode: 'refresh_token',
            credential_json: '{"refresh_token":"refresh-secret","token_endpoint":"https://auth.example.test/token"}',
        }));

        expect(payload).toEqual({
            credential_mode: 'refresh_token',
            credential_json: {
                refresh_token: 'refresh-secret',
                token_endpoint: 'https://auth.example.test/token',
            },
        });
        expect(buildCredentialConfigPayload(makeForm())).toEqual({ credential_mode: 'api_key' });
        expect(buildCredentialConfigPayload(makeForm({ credential_mode: 'api_key', credential_json: '{"access_token":"stale-token"}' })))
            .toEqual({ credential_mode: 'api_key' });
        expect(() => buildCredentialConfigPayload(makeForm({
            credential_mode: 'session_json',
            credential_json: '[]',
        })))
            .toThrow('Credential JSON must be a JSON object');
    });

    test('builds advanced settings only when advanced mode is enabled', () => {
        expect(buildAdvancedSettingsPayload(makeForm({
            advancedMode: false,
            reasoning_depth: 'high',
            system_prompt: 'system',
        }))).toEqual({});

        expect(buildAdvancedSettingsPayload(makeForm({
            advancedMode: true,
            reasoning_depth: 'high',
            temperature: '0.15',
            max_tokens: '2048',
            system_prompt: '  system prompt  ',
            classification_prompt_template: ' classify {transactions_json} ',
            rule_prompt_template: ' rule {category_name} ',
        }))).toEqual({
            reasoning_depth: 'high',
            temperature: 0.15,
            max_tokens: 2048,
            system_prompt: 'system prompt',
            classification_prompt_template: 'classify {transactions_json}',
            rule_prompt_template: 'rule {category_name}',
        });
    });

    test('normalizes configs and candidates without leaking top-level api keys', () => {
        expect(toLLMConfigs([
            {
                id: 1,
                name: 'primary',
                provider: 'openai',
                model: 'gpt-test',
                api_key: 'sk-secret',
            },
        ])).toEqual([
            {
                id: 1,
                name: 'primary',
                provider: 'openai',
                model: 'gpt-test',
            },
        ]);

        const candidates = toLLMCandidates({
            candidates: [
                {
                    id: '7',
                    type: 'rule',
                    confidence: '0.82',
                    suggested_main_category: '餐饮',
                    suggested_sub_category: '咖啡',
                    suggested_rule_expression: 'OR={咖啡}',
                    llm_response_raw: '{"reason":"高频咖啡商户"}',
                },
                {
                    id: 8,
                    status: 'accepted',
                    llm_response_raw: '{not-json',
                },
                {
                    id: 9,
                    type: 'account_rule_induction',
                    suggested_account_id: 17,
                    suggested_account_name: '工资卡',
                    suggested_rule_expression: 'OR={工资,薪资}',
                },
            ],
        });

        expect(candidates[0]).toMatchObject({
            id: 7,
            status: 'pending',
            confidence: 0.82,
            rule_type: 'rule',
            rule_content: 'OR={咖啡}',
            expression: 'OR={咖啡}',
            reason: '高频咖啡商户',
            category_name: '餐饮/咖啡',
            target_category: '餐饮/咖啡',
        });
        expect(candidates[1]).toMatchObject({
            id: 8,
            status: 'accepted',
            reason: '',
        });
        expect(candidates[2]).toMatchObject({
            id: 9,
            target_name: '工资卡',
            suggested_account_id: 17,
            suggested_account_name: '工资卡',
        });
        expect(toLLMCandidates(null)).toEqual([]);
    });
});
