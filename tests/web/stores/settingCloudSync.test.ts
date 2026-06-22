import { beforeEach, describe, expect, jest, test } from '@jest/globals';

import type { ApplicationSettings } from '@/core/setting.ts';
import {
    createSettingCloudSyncActions,
    hasEnabledApplicationCloudSync
} from '@/stores/setting/cloudSync.ts';

const mockUpdateApplicationSettingsValue = jest.fn<(key: string, value: unknown) => void>();
const mockUpdateApplicationSettingsSubValue = jest.fn<(key: string, subKey: string, value: unknown) => void>();
const mockUpdateUserApplicationCloudSettings = jest.fn<(payload: unknown) => Promise<{ data: { success: boolean; result: boolean } }>>();
const mockWarn = jest.fn();
const mockDebug = jest.fn();

jest.mock('@/lib/settings.ts', () => ({
    __esModule: true,
    updateApplicationSettingsValue: (key: string, value: unknown) => mockUpdateApplicationSettingsValue(key, value),
    updateApplicationSettingsSubValue: (key: string, subKey: string, value: unknown) => mockUpdateApplicationSettingsSubValue(key, subKey, value)
}));

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        updateUserApplicationCloudSettings: (payload: unknown) => mockUpdateUserApplicationCloudSettings(payload)
    }
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        warn: (message: unknown) => mockWarn(message),
        debug: (message: unknown, error?: unknown) => mockDebug(message, error)
    }
}));

function settingsFixture(): ApplicationSettings {
    return {
        showAccountBalance: true,
        autoSaveTransactionDraft: 'always',
        timezoneUsedForStatisticsInHomePage: 8,
        overviewAccountFilterInHomePage: { '1': true },
        statistics: {
            defaultChartDataType: 2,
            defaultAccountFilter: {}
        }
    } as unknown as ApplicationSettings;
}

function createCloudSyncTestRuntime(overrides: Partial<ApplicationSettings> = {}) {
    const appSettings = {
        value: {
            ...settingsFixture(),
            ...overrides
        }
    };
    const syncedAppSettings = {
        value: {} as Record<string, boolean>
    };
    const runtime = { appSettings, syncedAppSettings } as unknown as Parameters<typeof createSettingCloudSyncActions>[0];

    return {
        appSettings,
        syncedAppSettings,
        actions: createSettingCloudSyncActions(runtime)
    };
}

describe('setting cloud sync helper contracts', () => {
    beforeEach(() => {
        mockUpdateApplicationSettingsValue.mockClear();
        mockUpdateApplicationSettingsSubValue.mockClear();
        mockUpdateUserApplicationCloudSettings.mockReset();
        mockWarn.mockClear();
        mockDebug.mockClear();
    });

    test('cloud sync enabled state follows synced key map emptiness', () => {
        expect(hasEnabledApplicationCloudSync({})).toBe(false);
        expect(hasEnabledApplicationCloudSync({ showAccountBalance: true })).toBe(true);
    });

    test('createApplicationCloudSettings serializes supported root and nested setting keys', () => {
        const { actions } = createCloudSyncTestRuntime();

        expect(actions.createApplicationCloudSettings([
            'showAccountBalance',
            'autoSaveTransactionDraft',
            'statistics.defaultChartDataType',
            'unsupported.key'
        ])).toStrictEqual([
            { settingKey: 'showAccountBalance', settingValue: 'true' },
            { settingKey: 'autoSaveTransactionDraft', settingValue: 'always' },
            { settingKey: 'statistics.defaultChartDataType', settingValue: '2' }
        ]);
        expect(mockWarn).toHaveBeenCalledWith(expect.stringContaining('unsupported.key'));
    });

    test('setApplicationSettingsFromCloudSettings applies typed values and keeps synced keys', () => {
        const { appSettings, syncedAppSettings, actions } = createCloudSyncTestRuntime();

        actions.setApplicationSettingsFromCloudSettings([
            { settingKey: 'showAccountBalance', settingValue: 'false' },
            { settingKey: 'timezoneUsedForStatisticsInHomePage', settingValue: '9' },
            { settingKey: 'overviewAccountFilterInHomePage', settingValue: '{"1":false,"2":true}' },
            { settingKey: 'statistics.defaultChartDataType', settingValue: '3' }
        ]);

        expect(syncedAppSettings.value).toStrictEqual({
            showAccountBalance: true,
            timezoneUsedForStatisticsInHomePage: true,
            overviewAccountFilterInHomePage: true,
            'statistics.defaultChartDataType': true
        });
        expect(mockUpdateApplicationSettingsValue).toHaveBeenCalledWith('showAccountBalance', false);
        expect(mockUpdateApplicationSettingsValue).toHaveBeenCalledWith('timezoneUsedForStatisticsInHomePage', 9);
        expect(mockUpdateApplicationSettingsValue).toHaveBeenCalledWith('overviewAccountFilterInHomePage', { '1': false, '2': true });
        expect(mockUpdateApplicationSettingsSubValue).toHaveBeenCalledWith('statistics', 'defaultChartDataType', 3);
        expect(appSettings.value.showAccountBalance).toBe(false);
        expect(appSettings.value.statistics.defaultChartDataType).toBe(3);
    });

    test('updateUserApplicationCloudSettingValue writes only enabled supported keys', async () => {
        mockUpdateUserApplicationCloudSettings.mockResolvedValue({ data: { success: true, result: true } });
        const { syncedAppSettings, actions } = createCloudSyncTestRuntime();
        syncedAppSettings.value = { showAccountBalance: true };

        actions.updateUserApplicationCloudSettingValue('autoSaveTransactionDraft', 'always');
        expect(mockUpdateUserApplicationCloudSettings).not.toHaveBeenCalled();

        actions.updateUserApplicationCloudSettingValue('showAccountBalance', false);
        await Promise.resolve();

        expect(mockUpdateUserApplicationCloudSettings).toHaveBeenCalledWith({
            settings: [{ settingKey: 'showAccountBalance', settingValue: 'false' }],
            fullUpdate: false
        });
    });
});
