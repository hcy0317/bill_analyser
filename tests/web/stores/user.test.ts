import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { EMPTY_USER_BASIC_INFO, type UserBasicInfo } from '@/models/user.ts';
import type {
    DataStatisticsResponse,
    SettingsBundleImportResult
} from '@/models/data_management.ts';
import { useUserStore } from '@/stores/user.ts';

const memStore = new Map<string, string>();
(globalThis as unknown as { localStorage: Storage }).localStorage = {
    getItem: (key: string) => memStore.get(key) ?? null,
    setItem: (key: string, value: string) => { memStore.set(key, value); },
    removeItem: (key: string) => { memStore.delete(key); },
    clear: () => { memStore.clear(); },
    key: (index: number) => Array.from(memStore.keys())[index] ?? null,
    get length() { return memStore.size; }
} as unknown as Storage;
(globalThis as unknown as { window: { location: { pathname: string; origin: string } } }).window = {
    location: { pathname: '/', origin: 'http://localhost' }
};

type ApiResponse<T> = {
    data: {
        success: boolean;
        result: T;
    };
};

type BlobResponse = {
    data: BlobPart;
    headers: Record<string, string>;
};

const mockSettingsStore = {
    localeDefaultSettings: {
        currency: 'CNY',
        firstDayOfWeek: 1
    },
    createApplicationCloudSettings: jest.fn((enabledSettingKeys: string[]) => enabledSettingKeys.map(settingKey => ({
        settingKey,
        settingValue: 'true'
    }))),
    updateApplicationSyncSettingKeys: jest.fn()
};

const mockGetProfile = jest.fn<() => Promise<ApiResponse<UserBasicInfo>>>();
const mockUpdateProfile = jest.fn<(payload: unknown) => Promise<ApiResponse<{ user: UserBasicInfo }>>>();
const mockGetUserDataStatistics = jest.fn<() => Promise<ApiResponse<Record<string, string | number | undefined>>>>();
const mockGetExportedUserData = jest.fn<(fileType: string, req?: unknown) => Promise<BlobResponse>>();
const mockGetExportedSettingsBundleSection = jest.fn<(sectionKey: string, auth?: unknown) => Promise<BlobResponse>>();
const mockPreviewImportSettingsBundleSection = jest.fn<(sectionKey: string, bundle: unknown) => Promise<ApiResponse<SettingsBundleImportResult>>>();
const mockGetInternalAvatarUrlWithToken = jest.fn<(avatarUrl: string, disableBrowserCache: boolean | string) => string>();

jest.mock('@/stores/setting.ts', () => ({
    __esModule: true,
    useSettingsStore: () => mockSettingsStore
}));

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getProfile: () => mockGetProfile(),
        updateProfile: (payload: unknown) => mockUpdateProfile(payload),
        getUserDataStatistics: () => mockGetUserDataStatistics(),
        getExportedUserData: (fileType: string, req?: unknown) => mockGetExportedUserData(fileType, req),
        getExportedSettingsBundleSection: (sectionKey: string, auth?: unknown) => mockGetExportedSettingsBundleSection(sectionKey, auth),
        previewImportSettingsBundleSection: (sectionKey: string, bundle: unknown) => mockPreviewImportSettingsBundleSection(sectionKey, bundle),
        getInternalAvatarUrlWithToken: (avatarUrl: string, disableBrowserCache: boolean | string) => mockGetInternalAvatarUrlWithToken(avatarUrl, disableBrowserCache)
    }
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: jest.fn(),
        error: jest.fn()
    }
}));

function userProfile(overrides: Partial<UserBasicInfo> = {}): UserBasicInfo {
    return {
        ...EMPTY_USER_BASIC_INFO,
        id: 42,
        username: 'auth-user',
        email: 'auth@example.test',
        nickname: 'Auth User',
        defaultCurrency: 'CNY',
        firstDayOfWeek: 1,
        ...overrides
    };
}

function apiResponse<T>(result: T): ApiResponse<T> {
    return {
        data: {
            success: true,
            result
        }
    };
}

describe('user store auth/profile/data-management contracts', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        memStore.clear();
        mockSettingsStore.createApplicationCloudSettings.mockClear();
        mockSettingsStore.updateApplicationSyncSettingKeys.mockClear();
        mockGetProfile.mockReset();
        mockUpdateProfile.mockReset();
        mockGetUserDataStatistics.mockReset();
        mockGetExportedUserData.mockReset();
        mockGetExportedSettingsBundleSection.mockReset();
        mockPreviewImportSettingsBundleSection.mockReset();
        mockGetInternalAvatarUrlWithToken.mockReset();
    });

    test('storeUserBasicInfo normalizes profile fields and preserves localStorage contract', () => {
        const store = useUserStore();

        store.storeUserBasicInfo(userProfile({
            defaultAccountId: 101 as unknown as string,
            cashAccountId: null as unknown as string,
            avatar: 'avatar.png'
        }));

        expect(store.currentUserBasicInfo?.defaultAccountId).toBe('101');
        expect(store.currentUserBasicInfo?.cashAccountId).toBe('');
        expect(JSON.parse(memStore.get('ebk_user_info') || '{}')).toMatchObject({
            username: 'auth-user',
            defaultAccountId: '101',
            cashAccountId: ''
        });

        store.resetUserBasicInfo();
        expect(store.currentUserBasicInfo).toBeNull();
        expect(memStore.has('ebk_user_info')).toBe(false);
    });

    test('getCurrentUserProfile returns normalized profile without mutating cached basic info', async () => {
        mockGetProfile.mockResolvedValue(apiResponse(userProfile({
            username: 'profile-user',
            defaultAccountId: 202 as unknown as string
        })));
        const store = useUserStore();

        const profile = await store.getCurrentUserProfile();

        expect(profile.username).toBe('profile-user');
        expect(profile.defaultAccountId).toBe('202');
        expect(store.currentUserBasicInfo).toBeNull();
    });

    test('updateUserTransactionEditScope stores the returned profile payload', async () => {
        mockUpdateProfile.mockResolvedValue(apiResponse({
            user: userProfile({
                username: 'updated-user',
                transactionEditScope: 30
            })
        }));
        const store = useUserStore();

        await expect(store.updateUserTransactionEditScope({ transactionEditScope: 30 })).resolves.toMatchObject({
            user: expect.objectContaining({
                username: 'updated-user'
            })
        });

        expect(mockUpdateProfile).toHaveBeenCalledWith({ transactionEditScope: 30 });
        expect(store.currentUserBasicInfo?.username).toBe('updated-user');
        expect(JSON.parse(memStore.get('ebk_user_info') || '{}').transactionEditScope).toBe(30);
    });

    test('getUserDataStatistics normalizes numeric, string and legacy fallback counters', async () => {
        mockGetUserDataStatistics.mockResolvedValue(apiResponse({
            billCount: 17,
            accountCount: ' 5 ',
            categoryCount: 0,
            totalTransactionTagCount: '9',
            totalTransactionPictureCount: undefined,
            templateCount: 3,
            scheduledTransactionCount: ' 2 '
        }));
        const store = useUserStore();

        const expectedStatistics: DataStatisticsResponse = {
            totalTransactionCount: '17',
            totalAccountCount: '5',
            totalTransactionCategoryCount: '0',
            totalTransactionTagCount: '9',
            totalTransactionPictureCount: '0',
            totalTransactionTemplateCount: '3',
            totalScheduledTransactionCount: '2'
        };

        await expect(store.getUserDataStatistics()).resolves.toStrictEqual(expectedStatistics);
    });

    test('getExportedUserData rejects mismatched file content types before creating blob', async () => {
        mockGetExportedUserData.mockResolvedValue({
            data: 'csv,data',
            headers: {
                'content-type': 'text/plain'
            }
        });
        const store = useUserStore();

        await expect(store.getExportedUserData('csv')).rejects.toStrictEqual({
            message: 'Unable to retrieve exported user data'
        });
    });

    test('settings bundle section export and preview preserve section key and auth payload', async () => {
        const auth = { password: 'current-password' };
        const importResult: SettingsBundleImportResult = {
            dryRun: true,
            schemaVersion: 1,
            sections: {
                accounts: { created: 1, updated: 0, skipped: 0 }
            },
            warnings: []
        };
        mockGetExportedSettingsBundleSection.mockResolvedValue({
            data: '{"accounts":[]}',
            headers: {
                'content-type': 'application/json'
            }
        });
        mockPreviewImportSettingsBundleSection.mockResolvedValue(apiResponse(importResult));
        const store = useUserStore();

        const blob = await store.getExportedSettingsBundleSection('accounts', auth);
        await expect(store.previewImportSettingsBundleSection('accounts', { accounts: [] })).resolves.toBe(importResult);

        expect(blob.type).toBe('application/json');
        expect(mockGetExportedSettingsBundleSection).toHaveBeenCalledWith('accounts', auth);
        expect(mockPreviewImportSettingsBundleSection).toHaveBeenCalledWith('accounts', { accounts: [] });
    });

    test('getUserAvatarUrl delegates tokenized avatar URLs and returns null for empty avatar', () => {
        mockGetInternalAvatarUrlWithToken.mockReturnValue('/avatar/avatar.png?token=demo');
        const store = useUserStore();

        expect(store.getUserAvatarUrl(userProfile({ avatar: 'avatar.png' }), true)).toBe('/avatar/avatar.png?token=demo');
        expect(mockGetInternalAvatarUrlWithToken).toHaveBeenCalledWith('avatar.png', true);
        expect(store.getUserAvatarUrl(userProfile({ avatar: '' }), true)).toBeNull();
    });
});
