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

const mockGetProfile = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockUpdateProfile = jest.fn<(payload: unknown) => Promise<ApiResponse<unknown>>>();
const mockUpdateAvatar = jest.fn<(payload: unknown) => Promise<ApiResponse<unknown>>>();
const mockRemoveAvatar = jest.fn<() => Promise<ApiResponse<unknown>>>();
const mockGetUserDataStatistics = jest.fn<() => Promise<ApiResponse<Record<string, string | number | undefined>>>>();
const mockGetExportedUserData = jest.fn<(fileType: string, req?: unknown) => Promise<BlobResponse>>();
const mockGetExportedSettingsBundle = jest.fn<() => Promise<BlobResponse>>();
const mockGetExportedSettingsBundleSection = jest.fn<(sectionKey: string, auth?: unknown) => Promise<BlobResponse>>();
const mockPreviewImportSettingsBundle = jest.fn<(bundle: unknown) => Promise<ApiResponse<SettingsBundleImportResult>>>();
const mockPreviewImportSettingsBundleSection = jest.fn<(sectionKey: string, bundle: unknown) => Promise<ApiResponse<SettingsBundleImportResult>>>();
const mockImportSettingsBundle = jest.fn<(bundle: unknown) => Promise<ApiResponse<SettingsBundleImportResult>>>();
const mockImportSettingsBundleSection = jest.fn<(sectionKey: string, bundle: unknown) => Promise<ApiResponse<SettingsBundleImportResult>>>();
const mockGetInternalAvatarUrlWithToken = jest.fn<(avatarUrl: string, disableBrowserCache: boolean | string) => string>();
const mockGetUserApplicationCloudSettings = jest.fn<() => Promise<ApiResponse<false | Array<{ settingKey: string; settingValue: string }>>>>();
const mockUpdateUserApplicationCloudSettings = jest.fn<(payload: unknown) => Promise<ApiResponse<boolean>>>();
const mockDisableUserApplicationCloudSettings = jest.fn<() => Promise<ApiResponse<boolean>>>();

jest.mock('@/stores/setting.ts', () => ({
    __esModule: true,
    useSettingsStore: () => mockSettingsStore
}));

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getProfile: () => mockGetProfile(),
        updateProfile: (payload: unknown) => mockUpdateProfile(payload),
        updateAvatar: (payload: unknown) => mockUpdateAvatar(payload),
        removeAvatar: () => mockRemoveAvatar(),
        getUserDataStatistics: () => mockGetUserDataStatistics(),
        getExportedUserData: (fileType: string, req?: unknown) => mockGetExportedUserData(fileType, req),
        getExportedSettingsBundle: () => mockGetExportedSettingsBundle(),
        getExportedSettingsBundleSection: (sectionKey: string, auth?: unknown) => mockGetExportedSettingsBundleSection(sectionKey, auth),
        previewImportSettingsBundle: (bundle: unknown) => mockPreviewImportSettingsBundle(bundle),
        previewImportSettingsBundleSection: (sectionKey: string, bundle: unknown) => mockPreviewImportSettingsBundleSection(sectionKey, bundle),
        importSettingsBundle: (bundle: unknown) => mockImportSettingsBundle(bundle),
        importSettingsBundleSection: (sectionKey: string, bundle: unknown) => mockImportSettingsBundleSection(sectionKey, bundle),
        getInternalAvatarUrlWithToken: (avatarUrl: string, disableBrowserCache: boolean | string) => mockGetInternalAvatarUrlWithToken(avatarUrl, disableBrowserCache),
        getUserApplicationCloudSettings: () => mockGetUserApplicationCloudSettings(),
        updateUserApplicationCloudSettings: (payload: unknown) => mockUpdateUserApplicationCloudSettings(payload),
        disableUserApplicationCloudSettings: () => mockDisableUserApplicationCloudSettings()
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
        mockUpdateAvatar.mockReset();
        mockRemoveAvatar.mockReset();
        mockGetUserDataStatistics.mockReset();
        mockGetExportedUserData.mockReset();
        mockGetExportedSettingsBundle.mockReset();
        mockGetExportedSettingsBundleSection.mockReset();
        mockPreviewImportSettingsBundle.mockReset();
        mockPreviewImportSettingsBundleSection.mockReset();
        mockImportSettingsBundle.mockReset();
        mockImportSettingsBundleSection.mockReset();
        mockGetInternalAvatarUrlWithToken.mockReset();
        mockGetUserApplicationCloudSettings.mockReset();
        mockUpdateUserApplicationCloudSettings.mockReset();
        mockDisableUserApplicationCloudSettings.mockReset();
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

    test('basic info getters expose cached preferences and generate locale-aware users', () => {
        mockGetInternalAvatarUrlWithToken.mockReturnValue('/avatar/profile.png?token=current');
        const store = useUserStore();
        const profile = userProfile({
            avatar: 'profile.png',
            defaultAccountId: 'account-1',
            language: 'zh-Hans',
            defaultCurrency: 'USD',
            firstDayOfWeek: 1,
            fiscalYearStart: EMPTY_USER_BASIC_INFO.fiscalYearStart,
            calendarDisplayType: 2,
            dateDisplayType: 3,
            longDateFormat: 4,
            shortDateFormat: 5,
            longTimeFormat: 6,
            shortTimeFormat: 7,
            fiscalYearFormat: 8,
            currencyDisplayType: 9,
            numeralSystem: 10,
            decimalSeparator: 11,
            digitGroupingSymbol: 12,
            digitGrouping: 13,
            coordinateDisplayType: 14,
            expenseAmountColor: 15,
            incomeAmountColor: 16,
            cashAccountId: 'cash-account',
            cashTransferCategoryId: 'cash-transfer'
        });

        store.storeUserBasicInfo(profile);

        expect({
            nickname: store.currentUserNickname,
            avatar: store.currentUserAvatar,
            defaultAccountId: store.currentUserDefaultAccountId,
            language: store.currentUserLanguage,
            defaultCurrency: store.currentUserDefaultCurrency,
            firstDayOfWeek: store.currentUserFirstDayOfWeek,
            fiscalYearStart: store.currentUserFiscalYearStart,
            calendarDisplayType: store.currentUserCalendarDisplayType,
            dateDisplayType: store.currentUserDateDisplayType,
            longDateFormat: store.currentUserLongDateFormat,
            shortDateFormat: store.currentUserShortDateFormat,
            longTimeFormat: store.currentUserLongTimeFormat,
            shortTimeFormat: store.currentUserShortTimeFormat,
            fiscalYearFormat: store.currentUserFiscalYearFormat,
            currencyDisplayType: store.currentUserCurrencyDisplayType,
            numeralSystem: store.currentUserNumeralSystem,
            decimalSeparator: store.currentUserDecimalSeparator,
            digitGroupingSymbol: store.currentUserDigitGroupingSymbol,
            digitGrouping: store.currentUserDigitGrouping,
            coordinateDisplayType: store.currentUserCoordinateDisplayType,
            expenseAmountColor: store.currentUserExpenseAmountColor,
            incomeAmountColor: store.currentUserIncomeAmountColor,
            cashAccountId: store.currentUserCashAccountId,
            cashTransferCategoryId: store.currentUserCashTransferCategoryId
        }).toStrictEqual({
            nickname: 'Auth User',
            avatar: '/avatar/profile.png?token=current',
            defaultAccountId: 'account-1',
            language: 'zh-Hans',
            defaultCurrency: 'USD',
            firstDayOfWeek: 1,
            fiscalYearStart: EMPTY_USER_BASIC_INFO.fiscalYearStart,
            calendarDisplayType: 2,
            dateDisplayType: 3,
            longDateFormat: 4,
            shortDateFormat: 5,
            longTimeFormat: 6,
            shortTimeFormat: 7,
            fiscalYearFormat: 8,
            currencyDisplayType: 9,
            numeralSystem: 10,
            decimalSeparator: 11,
            digitGroupingSymbol: 12,
            digitGrouping: 13,
            coordinateDisplayType: 14,
            expenseAmountColor: 15,
            incomeAmountColor: 16,
            cashAccountId: 'cash-account',
            cashTransferCategoryId: 'cash-transfer'
        });
        expect(mockGetInternalAvatarUrlWithToken).toHaveBeenCalledWith('profile.png', false);

        const newUser = store.generateNewUserModel('en');
        expect(newUser).toMatchObject({
            language: 'en',
            defaultCurrency: 'CNY',
            firstDayOfWeek: 1
        });
    });

    test('basic info getters fall back for empty and invalid cached preferences', () => {
        const store = useUserStore();

        expect({
            nickname: store.currentUserNickname,
            avatar: store.currentUserAvatar,
            defaultAccountId: store.currentUserDefaultAccountId,
            language: store.currentUserLanguage,
            defaultCurrency: store.currentUserDefaultCurrency,
            firstDayOfWeek: store.currentUserFirstDayOfWeek,
            fiscalYearStart: store.currentUserFiscalYearStart,
            calendarDisplayType: store.currentUserCalendarDisplayType,
            dateDisplayType: store.currentUserDateDisplayType,
            longDateFormat: store.currentUserLongDateFormat,
            shortDateFormat: store.currentUserShortDateFormat,
            longTimeFormat: store.currentUserLongTimeFormat,
            shortTimeFormat: store.currentUserShortTimeFormat,
            fiscalYearFormat: store.currentUserFiscalYearFormat,
            currencyDisplayType: store.currentUserCurrencyDisplayType,
            numeralSystem: store.currentUserNumeralSystem,
            decimalSeparator: store.currentUserDecimalSeparator,
            digitGroupingSymbol: store.currentUserDigitGroupingSymbol,
            digitGrouping: store.currentUserDigitGrouping,
            coordinateDisplayType: store.currentUserCoordinateDisplayType,
            expenseAmountColor: store.currentUserExpenseAmountColor,
            incomeAmountColor: store.currentUserIncomeAmountColor,
            cashAccountId: store.currentUserCashAccountId,
            cashTransferCategoryId: store.currentUserCashTransferCategoryId
        }).toStrictEqual({
            nickname: null,
            avatar: null,
            defaultAccountId: EMPTY_USER_BASIC_INFO.defaultAccountId,
            language: EMPTY_USER_BASIC_INFO.language,
            defaultCurrency: 'CNY',
            firstDayOfWeek: 1,
            fiscalYearStart: EMPTY_USER_BASIC_INFO.fiscalYearStart,
            calendarDisplayType: EMPTY_USER_BASIC_INFO.calendarDisplayType,
            dateDisplayType: EMPTY_USER_BASIC_INFO.dateDisplayType,
            longDateFormat: EMPTY_USER_BASIC_INFO.longDateFormat,
            shortDateFormat: EMPTY_USER_BASIC_INFO.shortDateFormat,
            longTimeFormat: EMPTY_USER_BASIC_INFO.longTimeFormat,
            shortTimeFormat: EMPTY_USER_BASIC_INFO.shortTimeFormat,
            fiscalYearFormat: EMPTY_USER_BASIC_INFO.fiscalYearFormat,
            currencyDisplayType: EMPTY_USER_BASIC_INFO.currencyDisplayType,
            numeralSystem: EMPTY_USER_BASIC_INFO.numeralSystem,
            decimalSeparator: EMPTY_USER_BASIC_INFO.decimalSeparator,
            digitGroupingSymbol: EMPTY_USER_BASIC_INFO.digitGroupingSymbol,
            digitGrouping: EMPTY_USER_BASIC_INFO.digitGrouping,
            coordinateDisplayType: EMPTY_USER_BASIC_INFO.coordinateDisplayType,
            expenseAmountColor: EMPTY_USER_BASIC_INFO.expenseAmountColor,
            incomeAmountColor: EMPTY_USER_BASIC_INFO.incomeAmountColor,
            cashAccountId: EMPTY_USER_BASIC_INFO.cashAccountId,
            cashTransferCategoryId: EMPTY_USER_BASIC_INFO.cashTransferCategoryId
        });

        store.storeUserBasicInfo(userProfile({
            nickname: '',
            username: 'fallback-name',
            defaultCurrency: '',
            firstDayOfWeek: 99,
            fiscalYearStart: -999
        }));
        expect(store.currentUserNickname).toBe('fallback-name');
        expect(store.currentUserDefaultCurrency).toBe('CNY');
        expect(store.currentUserFirstDayOfWeek).toBe(1);
        expect(store.currentUserFiscalYearStart).toBe(EMPTY_USER_BASIC_INFO.fiscalYearStart);

        store.storeUserBasicInfo(userProfile({
            firstDayOfWeek: 'invalid' as unknown as number,
            fiscalYearStart: 'invalid' as unknown as number
        }));
        expect(store.currentUserFirstDayOfWeek).toBe(1);
        expect(store.currentUserFiscalYearStart).toBe(EMPTY_USER_BASIC_INFO.fiscalYearStart);
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

    test('avatar update and removal refresh the cached basic info', async () => {
        const avatarFile = { name: 'avatar.png' } as File;
        mockUpdateAvatar.mockResolvedValue(apiResponse(userProfile({
            username: 'avatar-user',
            avatar: 'avatar.png'
        })));
        mockRemoveAvatar.mockResolvedValue(apiResponse(userProfile({
            username: 'avatar-user',
            avatar: ''
        })));
        const store = useUserStore();

        await expect(store.updateUserAvatar({ avatarFile })).resolves.toMatchObject({
            avatar: 'avatar.png'
        });
        expect(mockUpdateAvatar).toHaveBeenCalledWith({ avatarFile });
        expect(store.currentUserBasicInfo?.avatar).toBe('avatar.png');

        await expect(store.removeUserAvatar()).resolves.toMatchObject({ avatar: '' });
        expect(mockRemoveAvatar).toHaveBeenCalledTimes(1);
        expect(store.currentUserBasicInfo?.avatar).toBe('');
    });

    test('profile actions reject malformed success envelopes', async () => {
        const malformed = {
            data: {
                success: false,
                result: null
            }
        } as ApiResponse<unknown>;
        const malformedUpdate = apiResponse({ user: 'not-an-object' });
        const store = useUserStore();

        mockGetProfile.mockResolvedValueOnce(malformed);
        await expect(store.getCurrentUserProfile()).rejects.toStrictEqual({
            message: 'Unable to retrieve user profile'
        });

        mockUpdateProfile.mockResolvedValueOnce(malformedUpdate);
        await expect(store.updateUserTransactionEditScope({ transactionEditScope: 7 })).rejects.toStrictEqual({
            message: 'Unable to update editable transaction range'
        });

        mockUpdateAvatar.mockResolvedValueOnce(malformed);
        await expect(store.updateUserAvatar({ avatarFile: { name: 'avatar.png' } as File })).rejects.toStrictEqual({
            message: 'Unable to update user avatar'
        });

        mockRemoveAvatar.mockResolvedValueOnce(malformed);
        await expect(store.removeUserAvatar()).rejects.toStrictEqual({
            message: 'Unable to remove user avatar'
        });
    });

    test('profile actions route backend, unprocessed and processed failures distinctly', async () => {
        const avatarFile = { name: 'avatar.png' } as File;
        const store = useUserStore();
        const actions = [
            {
                reject: (error: unknown) => mockGetProfile.mockRejectedValueOnce(error),
                invoke: () => store.getCurrentUserProfile(),
                fallbackMessage: 'Unable to retrieve user profile'
            },
            {
                reject: (error: unknown) => mockUpdateProfile.mockRejectedValueOnce(error),
                invoke: () => store.updateUserTransactionEditScope({ transactionEditScope: 14 }),
                fallbackMessage: 'Unable to update editable transaction range'
            },
            {
                reject: (error: unknown) => mockUpdateAvatar.mockRejectedValueOnce(error),
                invoke: () => store.updateUserAvatar({ avatarFile }),
                fallbackMessage: 'Unable to update user avatar'
            },
            {
                reject: (error: unknown) => mockRemoveAvatar.mockRejectedValueOnce(error),
                invoke: () => store.removeUserAvatar(),
                fallbackMessage: 'Unable to remove user avatar'
            }
        ];

        for (const [index, action] of actions.entries()) {
            const backendData = { message: `backend-${index}` };
            action.reject({ response: { data: backendData } });
            await expect(action.invoke()).rejects.toStrictEqual({ error: backendData });

            action.reject({ processed: false });
            await expect(action.invoke()).rejects.toStrictEqual({
                message: action.fallbackMessage
            });

            const processedError = { processed: true, marker: `processed-${index}` };
            action.reject(processedError);
            await expect(action.invoke()).rejects.toBe(processedError);
        }
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

    test('settings bundle full and section actions preserve blobs and import results', async () => {
        const importResult: SettingsBundleImportResult = {
            dryRun: false,
            schemaVersion: 1,
            sections: {
                accounts: { created: 1, updated: 2, skipped: 3 }
            },
            warnings: []
        };
        mockGetExportedSettingsBundle.mockResolvedValue({
            data: '{"accounts":[]}',
            headers: {}
        });
        mockPreviewImportSettingsBundle.mockResolvedValue(apiResponse(importResult));
        mockImportSettingsBundle.mockResolvedValue(apiResponse(importResult));
        mockImportSettingsBundleSection.mockResolvedValue(apiResponse(importResult));
        const store = useUserStore();
        const bundle = { accounts: [] };

        const blob = await store.getExportedSettingsBundle();
        await expect(store.previewImportSettingsBundle(bundle)).resolves.toBe(importResult);
        await expect(store.importSettingsBundle(bundle)).resolves.toBe(importResult);
        await expect(store.importSettingsBundleSection('accounts', bundle)).resolves.toBe(importResult);

        expect(blob.type).toBe('application/json');
        expect(mockPreviewImportSettingsBundle).toHaveBeenCalledWith(bundle);
        expect(mockImportSettingsBundle).toHaveBeenCalledWith(bundle);
        expect(mockImportSettingsBundleSection).toHaveBeenCalledWith('accounts', bundle);
    });

    test('settings bundle import actions reject malformed success envelopes', async () => {
        const malformed = {
            data: {
                success: false,
                result: null
            }
        } as unknown as ApiResponse<SettingsBundleImportResult>;
        const store = useUserStore();

        mockPreviewImportSettingsBundle.mockResolvedValueOnce(malformed);
        await expect(store.previewImportSettingsBundle({})).rejects.toStrictEqual({
            message: 'Unable to preview settings bundle import'
        });

        mockPreviewImportSettingsBundleSection.mockResolvedValueOnce(malformed);
        await expect(store.previewImportSettingsBundleSection('accounts', {})).rejects.toStrictEqual({
            message: 'Unable to preview settings bundle import'
        });

        mockImportSettingsBundle.mockResolvedValueOnce(malformed);
        await expect(store.importSettingsBundle({})).rejects.toStrictEqual({
            message: 'Unable to import settings bundle'
        });

        mockImportSettingsBundleSection.mockResolvedValueOnce(malformed);
        await expect(store.importSettingsBundleSection('accounts', {})).rejects.toStrictEqual({
            message: 'Unable to import settings bundle'
        });
    });

    test('settings bundle actions convert unprocessed failures and preserve processed failures', async () => {
        const store = useUserStore();
        const actions = [
            {
                reject: (error: unknown) => mockGetExportedSettingsBundle.mockRejectedValueOnce(error),
                invoke: () => store.getExportedSettingsBundle(),
                fallbackMessage: 'Unable to retrieve exported settings bundle'
            },
            {
                reject: (error: unknown) => mockGetExportedSettingsBundleSection.mockRejectedValueOnce(error),
                invoke: () => store.getExportedSettingsBundleSection('accounts'),
                fallbackMessage: 'Unable to retrieve exported settings bundle'
            },
            {
                reject: (error: unknown) => mockPreviewImportSettingsBundle.mockRejectedValueOnce(error),
                invoke: () => store.previewImportSettingsBundle({}),
                fallbackMessage: 'Unable to preview settings bundle import'
            },
            {
                reject: (error: unknown) => mockPreviewImportSettingsBundleSection.mockRejectedValueOnce(error),
                invoke: () => store.previewImportSettingsBundleSection('accounts', {}),
                fallbackMessage: 'Unable to preview settings bundle import'
            },
            {
                reject: (error: unknown) => mockImportSettingsBundle.mockRejectedValueOnce(error),
                invoke: () => store.importSettingsBundle({}),
                fallbackMessage: 'Unable to import settings bundle'
            },
            {
                reject: (error: unknown) => mockImportSettingsBundleSection.mockRejectedValueOnce(error),
                invoke: () => store.importSettingsBundleSection('accounts', {}),
                fallbackMessage: 'Unable to import settings bundle'
            }
        ];

        for (const [index, action] of actions.entries()) {
            action.reject({ processed: false });
            await expect(action.invoke()).rejects.toStrictEqual({
                message: action.fallbackMessage
            });

            const processedError = { processed: true, marker: `processed-${index}` };
            action.reject(processedError);
            await expect(action.invoke()).rejects.toBe(processedError);
        }
    });

    test('getUserAvatarUrl delegates tokenized avatar URLs and returns null for empty avatar', () => {
        mockGetInternalAvatarUrlWithToken.mockReturnValue('/avatar/avatar.png?token=demo');
        const store = useUserStore();

        expect(store.getUserAvatarUrl(userProfile({ avatar: 'avatar.png' }), true)).toBe('/avatar/avatar.png?token=demo');
        expect(mockGetInternalAvatarUrlWithToken).toHaveBeenCalledWith('avatar.png', true);
        expect(store.getUserAvatarUrl(userProfile({ avatar: '' }), true)).toBeNull();
    });

    test('cloud settings load returns false or server settings without local mutation', async () => {
        mockGetUserApplicationCloudSettings.mockResolvedValueOnce(apiResponse(false));
        const store = useUserStore();

        await expect(store.getUserApplicationCloudSettings()).resolves.toBe(false);
        expect(mockSettingsStore.updateApplicationSyncSettingKeys).not.toHaveBeenCalled();

        const cloudSettings = [
            { settingKey: 'showAccountBalance', settingValue: 'false' },
            { settingKey: 'autoSaveTransactionDraft', settingValue: 'enabled' }
        ];
        mockGetUserApplicationCloudSettings.mockResolvedValueOnce(apiResponse(cloudSettings));

        await expect(store.getUserApplicationCloudSettings()).resolves.toStrictEqual(cloudSettings);
        expect(mockSettingsStore.updateApplicationSyncSettingKeys).not.toHaveBeenCalled();
    });

    test('full cloud settings update sends enabled keys and refreshes local synced keys', async () => {
        mockUpdateUserApplicationCloudSettings.mockResolvedValue(apiResponse(true));
        const store = useUserStore();
        const enabledKeys = ['showAccountBalance', 'autoSaveTransactionDraft'];

        await expect(store.fullUpdateUserApplicationCloudSettings(enabledKeys)).resolves.toBe(true);

        expect(mockSettingsStore.createApplicationCloudSettings).toHaveBeenCalledWith(enabledKeys);
        expect(mockUpdateUserApplicationCloudSettings).toHaveBeenCalledWith({
            settings: [
                { settingKey: 'showAccountBalance', settingValue: 'true' },
                { settingKey: 'autoSaveTransactionDraft', settingValue: 'true' }
            ],
            fullUpdate: true
        });
        expect(mockSettingsStore.updateApplicationSyncSettingKeys).toHaveBeenCalledWith(enabledKeys);
    });

    test('disable cloud settings clears local synced keys after server success', async () => {
        mockDisableUserApplicationCloudSettings.mockResolvedValue(apiResponse(true));
        const store = useUserStore();

        await expect(store.disableUserApplicationCloudSettings()).resolves.toBe(true);

        expect(mockDisableUserApplicationCloudSettings).toHaveBeenCalledTimes(1);
        expect(mockSettingsStore.updateApplicationSyncSettingKeys).toHaveBeenCalledWith(undefined);
    });
});
