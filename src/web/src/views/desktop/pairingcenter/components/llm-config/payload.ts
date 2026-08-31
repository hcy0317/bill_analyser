import type {
    LLMAdvancedSettings,
    LLMConfigForm,
} from './types.ts';

/**
 * 解析可选 JSON 对象字段，空字符串视为未配置，非对象 JSON 直接报错。
 */
function parseOptionalJsonObject(text: string, label: string): Record<string, unknown> {
    const trimmed = text.trim();
    if (!trimmed) {
        return {};
    }

    const parsed = JSON.parse(trimmed) as unknown;
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        throw new Error(`${label} must be a JSON object`);
    }

    return parsed as Record<string, unknown>;
}

/**
 * 根据接入方式构造 credential_config payload；OAuth 只提交用户粘贴的完整 JSON。
 */
export function buildCredentialConfigPayload(form: LLMConfigForm): Record<string, unknown> {
    const credentialMode = form.credential_mode || 'api_key';
    const payload: Record<string, unknown> = {
        credential_mode: credentialMode,
    };
    if (credentialMode === 'api_key') {
        return payload;
    }

    const credentialJson = parseOptionalJsonObject(form.credential_json, 'Credential JSON');
    if (Object.keys(credentialJson).length > 0) {
        payload['credential_json'] = credentialJson;
    }

    return payload;
}

/**
 * 根据高级模式构造 advanced_settings；未开启高级模式时不提交任何高级字段。
 */
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

    if (form.customPrompts) {
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
    }

    return settings;
}
