import type { ApplicationCloudSetting } from '@/core/setting.ts';

import type { UserBasicInfo } from './user.ts';

export interface AuthResponse {
    readonly token: string;
    readonly refreshToken?: string;
    readonly need2FA: boolean;
    readonly user?: UserBasicInfo;
    readonly applicationCloudSettings?: ApplicationCloudSetting[];
    readonly notificationContent?: string;
}

export interface RegisterResponse extends AuthResponse {
    readonly needVerifyEmail: boolean;
    readonly presetCategoriesSaved: boolean;
    readonly presetAccountsSaved?: boolean;
    readonly defaultSeed?: RegisterDefaultSeedSummary;
}

export interface RegisterDefaultSeedSummary {
    readonly package: string;
    readonly categoriesCreated: number;
    readonly categoriesSkipped: number;
    readonly categoryRulesCreated: number;
    readonly categoryRulesSkipped: number;
    readonly accountsCreated: number;
    readonly accountsSkipped: number;
    readonly accountRulesCreated: number;
    readonly accountRulesSkipped: number;
    readonly rulesMissingTargets: number;
}
