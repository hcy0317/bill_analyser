import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import {
    ALL_ALLOWED_CLOUD_SYNC_APP_SETTING_KEY_TYPES,
    type ApplicationCloudSetting
} from '@/core/setting.ts';
import {
    ALL_APPLICATION_CLOUD_SETTINGS,
    useAppCloudSyncBase
} from '@/views/base/settings/AppCloudSyncPageBase.ts';

const mockSetApplicationSettingsFromCloudSettings = jest.fn<(settings?: ApplicationCloudSetting[]) => void>();
const mockSettingsStore = {
    syncedAppSettings: {} as Record<string, boolean>,
    enableApplicationCloudSync: false,
    setApplicationSettingsFromCloudSettings: mockSetApplicationSettingsFromCloudSettings
};

jest.mock('@/stores/setting.ts', () => ({
    __esModule: true,
    useSettingsStore: () => mockSettingsStore
}));

function allCloudSettingKeys(): string[] {
    return ALL_APPLICATION_CLOUD_SETTINGS.flatMap(category => category.items.map(item => item.settingKey));
}

describe('AppCloudSyncPageBase shared selection contracts', () => {
    beforeEach(() => {
        mockSettingsStore.syncedAppSettings = {};
        mockSettingsStore.enableApplicationCloudSync = false;
        mockSetApplicationSettingsFromCloudSettings.mockClear();
    });

    test('cloud setting catalog mirrors the allowed sync key contract', () => {
        const keys = allCloudSettingKeys();

        expect(new Set(keys).size).toBe(keys.length);
        expect([...keys].sort()).toStrictEqual(Object.keys(ALL_ALLOWED_CLOUD_SYNC_APP_SETTING_KEY_TYPES).sort());
        expect(keys).toContain('showAccountBalance');
        expect(keys).toContain('alwaysShowTransactionPicturesInMobileTransactionEditPage');

        const mobileOnlyPictureSetting = ALL_APPLICATION_CLOUD_SETTINGS
            .flatMap(category => category.items)
            .find(item => item.settingKey === 'alwaysShowTransactionPicturesInMobileTransactionEditPage');

        expect(mobileOnlyPictureSetting).toMatchObject({
            mobile: true,
            desktop: false
        });
    });

    test('initial state is copied from settings store synced keys', () => {
        mockSettingsStore.syncedAppSettings = {
            showAccountBalance: true,
            showAmountInHomePage: false,
            autoSaveTransactionDraft: true
        };
        mockSettingsStore.enableApplicationCloudSync = true;

        const base = useAppCloudSyncBase();

        expect(base.isEnableCloudSync.value).toBe(true);
        expect(base.enabledApplicationCloudSettings.value).not.toBe(mockSettingsStore.syncedAppSettings);
        expect(base.enabledApplicationCloudSettings.value).toStrictEqual(mockSettingsStore.syncedAppSettings);
        expect(base.hasEnabledApplicationCloudSettings.value).toBe(true);
        expect(base.enabledApplicationCloudSettingKeys.value).toStrictEqual([
            'showAccountBalance',
            'autoSaveTransactionDraft'
        ]);
    });

    test('selection helpers keep all, none, invert and partial category states stable', () => {
        const base = useAppCloudSyncBase();
        const overviewCategory = ALL_APPLICATION_CLOUD_SETTINGS.find(category => category.categoryName === 'Overview Page');

        expect(overviewCategory).toBeDefined();
        base.updateSettingsSelected(overviewCategory!, true);
        expect(base.isAllSettingsSelected(overviewCategory!)).toBe(true);
        expect(base.hasSettingSelectedButNotAllChecked(overviewCategory!)).toBe(false);

        base.enabledApplicationCloudSettings.value['showAmountInHomePage'] = false;
        expect(base.isAllSettingsSelected(overviewCategory!)).toBe(false);
        expect(base.hasSettingSelectedButNotAllChecked(overviewCategory!)).toBe(true);

        base.selectNoneSettings();
        expect(base.hasEnabledApplicationCloudSettings.value).toBe(false);
        expect(base.enabledApplicationCloudSettingKeys.value).toStrictEqual([]);

        base.selectInvertSettings();
        expect([...base.enabledApplicationCloudSettingKeys.value].sort()).toStrictEqual([...allCloudSettingKeys()].sort());

        base.selectAllSettings();
        expect([...base.enabledApplicationCloudSettingKeys.value].sort()).toStrictEqual([...allCloudSettingKeys()].sort());
    });

    test('server cloud settings are applied by key and false clears the local selection map', () => {
        const base = useAppCloudSyncBase();
        const cloudSettings: ApplicationCloudSetting[] = [
            { settingKey: 'showAccountBalance', settingValue: 'false' },
            { settingKey: 'statistics.defaultChartDataType', settingValue: '2' }
        ];

        base.setUserApplicationCloudSettings(cloudSettings);

        expect(mockSetApplicationSettingsFromCloudSettings).toHaveBeenCalledWith(cloudSettings);
        expect(base.enabledApplicationCloudSettings.value).toMatchObject({
            showAccountBalance: true,
            'statistics.defaultChartDataType': true
        });

        base.setUserApplicationCloudSettings(false);

        expect(mockSetApplicationSettingsFromCloudSettings).toHaveBeenLastCalledWith(undefined);
        expect(base.enabledApplicationCloudSettings.value).toStrictEqual({});
        expect(base.hasEnabledApplicationCloudSettings.value).toBe(false);
    });
});
