import { describe, expect, test } from '@jest/globals';

import {
    EMPTY_PAIRING_CENTER_INVESTMENT_SETTINGS,
    buildPairingCenterInvestmentSettingsPayload,
    normalizePairingCenterInvestmentSettings,
    toPairingCenterInvestmentSettingsUpdateRequest
} from '@/models/pairing_center.ts';
import {
    EMPTY_USER_BASIC_INFO
} from '@/models/user.ts';

describe('Pairing Center investment settings helpers', () => {
    test('normalizePairingCenterInvestmentSettings extracts the dedicated settings subset from user info', () => {
        const normalized = normalizePairingCenterInvestmentSettings({
            ...EMPTY_USER_BASIC_INFO,
            importLearningEnabled: true,
            investmentPlatformKeywords: ['平台A'],
            investmentProductKeywords: ['产品A'],
            investmentExcludeKeywords: ['排除A']
        });

        expect(normalized).toStrictEqual({
            importLearningEnabled: true,
            investmentPlatformKeywords: ['平台A'],
            investmentProductKeywords: ['产品A'],
            investmentExcludeKeywords: ['排除A']
        });
    });

    test('buildPairingCenterInvestmentSettingsPayload keeps full normalized user info and detached settings arrays', () => {
        const payload = buildPairingCenterInvestmentSettingsPayload({
            username: 'pair-user',
            defaultAccountId: 88 as unknown as string,
            importLearningEnabled: true,
            investmentPlatformKeywords: ['平台B'],
            investmentProductKeywords: ['产品B'],
            investmentExcludeKeywords: ['排除B']
        });

        expect(payload.user.username).toBe('pair-user');
        expect(payload.user.defaultAccountId).toBe('88');
        expect(payload.settings).toStrictEqual({
            importLearningEnabled: true,
            investmentPlatformKeywords: ['平台B'],
            investmentProductKeywords: ['产品B'],
            investmentExcludeKeywords: ['排除B']
        });

        payload.user.investmentPlatformKeywords.push('不应共享');
        expect(payload.settings.investmentPlatformKeywords).toStrictEqual(['平台B']);
    });

    test('toPairingCenterInvestmentSettingsUpdateRequest emits only the dedicated update fields and clones arrays', () => {
        const settings = normalizePairingCenterInvestmentSettings({
            ...EMPTY_PAIRING_CENTER_INVESTMENT_SETTINGS,
            importLearningEnabled: true,
            investmentPlatformKeywords: ['平台C'],
            investmentProductKeywords: ['产品C'],
            investmentExcludeKeywords: ['排除C']
        });

        const request = toPairingCenterInvestmentSettingsUpdateRequest(settings);
        settings.investmentPlatformKeywords.push('变更后不应污染请求');

        expect(request).toStrictEqual({
            importLearningEnabled: true,
            investmentPlatformKeywords: ['平台C'],
            investmentProductKeywords: ['产品C'],
            investmentExcludeKeywords: ['排除C']
        });
    });
});
