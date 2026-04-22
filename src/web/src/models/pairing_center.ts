import {
    normalizeUserBasicInfo,
    type UserBasicInfo
} from '@/models/user.ts';

export interface PairingCenterInvestmentSettings {
    readonly importLearningEnabled: boolean;
    readonly investmentPlatformKeywords: string[];
    readonly investmentProductKeywords: string[];
    readonly investmentExcludeKeywords: string[];
}

export interface PairingCenterInvestmentSettingsUpdateRequest {
    readonly importLearningEnabled: boolean;
    readonly investmentPlatformKeywords: string[];
    readonly investmentProductKeywords: string[];
    readonly investmentExcludeKeywords: string[];
}

export interface PairingCenterInvestmentSettingsPayload {
    readonly user: UserBasicInfo;
    readonly settings: PairingCenterInvestmentSettings;
}

export const EMPTY_PAIRING_CENTER_INVESTMENT_SETTINGS: PairingCenterInvestmentSettings = {
    importLearningEnabled: false,
    investmentPlatformKeywords: [],
    investmentProductKeywords: [],
    investmentExcludeKeywords: []
};

export function normalizePairingCenterInvestmentSettings(
    value?: Partial<PairingCenterInvestmentSettings> | Partial<UserBasicInfo> | null
): PairingCenterInvestmentSettings {
    const normalizedUserInfo = normalizeUserBasicInfo(value as Partial<UserBasicInfo> | null);

    return {
        importLearningEnabled: !!normalizedUserInfo.importLearningEnabled,
        investmentPlatformKeywords: [...normalizedUserInfo.investmentPlatformKeywords],
        investmentProductKeywords: [...normalizedUserInfo.investmentProductKeywords],
        investmentExcludeKeywords: [...normalizedUserInfo.investmentExcludeKeywords]
    };
}

export function buildPairingCenterInvestmentSettingsPayload(
    userInfo?: Partial<UserBasicInfo> | null
): PairingCenterInvestmentSettingsPayload {
    const normalizedUserInfo = normalizeUserBasicInfo(userInfo);

    return {
        user: normalizedUserInfo,
        settings: normalizePairingCenterInvestmentSettings(normalizedUserInfo)
    };
}

export function toPairingCenterInvestmentSettingsUpdateRequest(
    settings: PairingCenterInvestmentSettings
): PairingCenterInvestmentSettingsUpdateRequest {
    const normalizedSettings = normalizePairingCenterInvestmentSettings(settings);

    return {
        importLearningEnabled: normalizedSettings.importLearningEnabled,
        investmentPlatformKeywords: [...normalizedSettings.investmentPlatformKeywords],
        investmentProductKeywords: [...normalizedSettings.investmentProductKeywords],
        investmentExcludeKeywords: [...normalizedSettings.investmentExcludeKeywords]
    };
}
