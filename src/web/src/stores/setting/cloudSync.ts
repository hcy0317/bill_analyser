import type { Ref } from 'vue';

import { values } from '@/core/base.ts';
import {
    type ApplicationSettingValue,
    type ApplicationSettingSubValue,
    type ApplicationSettings,
    type ApplicationCloudSetting,
    UserApplicationCloudSettingType,
    ALL_ALLOWED_CLOUD_SYNC_APP_SETTING_KEY_TYPES
} from '@/core/setting.ts';
import {
    isObject,
    isString,
    isBoolean,
    getObjectOwnFieldCount,
    arrayItemToObjectField
} from '@/lib/common.ts';
import {
    updateApplicationSettingsValue,
    updateApplicationSettingsSubValue
} from '@/lib/settings.ts';
import logger from '@/lib/logger.ts';
import services from '@/lib/services.ts';

export interface SettingCloudSyncRuntime {
    readonly appSettings: Ref<ApplicationSettings>;
    readonly syncedAppSettings: Ref<Record<string, boolean>>;
}

/** 中文说明：判断应用设置云同步是否启用，避免页面和 store 重复理解 synced key map 的启用语义。 */
export function hasEnabledApplicationCloudSync(syncedAppSettings: Record<string, boolean>): boolean {
    return getObjectOwnFieldCount(syncedAppSettings) > 0;
}

/** 中文说明：创建应用设置云同步 helper，集中处理设置序列化、服务端增量写回和云端 setting 反序列化。 */
export function createSettingCloudSyncActions(runtime: SettingCloudSyncRuntime) {
    /** 中文说明：把云端 setting value 写回本地 settings storage 与 Pinia appSettings，兼容一级 key 和二级 key。 */
    function updateApplicationSettingsValueAndAppSettingsFromCloudSetting(key: string, value: string | number | boolean | Record<string, boolean>): void {
        const keyItems = key.split('.');
        const keyFirstPart = keyItems[0] as string;

        if (keyItems.length === 1) {
            updateApplicationSettingsValue(keyFirstPart, value);
            runtime.appSettings.value[keyFirstPart] = value;
        } else if (keyItems.length === 2) {
            const subKey = keyItems[1] as string;
            updateApplicationSettingsSubValue(keyFirstPart, subKey, value);
            (runtime.appSettings.value[keyFirstPart] as Record<string, ApplicationSettingSubValue>)[subKey] = value;
        } else {
            logger.warn(`cannot load application cloud setting "${key}", because it has invalid key format`);
        }
    }

    function createUserApplicationCloudSetting(key: string): ApplicationCloudSetting | null {
        const settingType = ALL_ALLOWED_CLOUD_SYNC_APP_SETTING_KEY_TYPES[key];

        if (!settingType) {
            logger.warn(`cannot get application cloud setting "${key}", because it is not supported to sync`);
            return null;
        }

        const keyItems = key.split('.');
        let value: ApplicationSettingValue | ApplicationSettingSubValue = runtime.appSettings.value[key] as (ApplicationSettingValue | ApplicationSettingSubValue);

        if (keyItems.length === 2) {
            const primaryKey = keyItems[0] as string;
            const subKey = keyItems[1] as string;
            value = (runtime.appSettings.value[primaryKey] as Record<string, ApplicationSettingSubValue>)[subKey] as ApplicationSettingSubValue;
        } else if (keyItems.length > 2) {
            logger.warn(`cannot get application cloud setting "${key}", because it has invalid key format`);
            return null;
        }

        let settingValue = '';

        if (settingType === UserApplicationCloudSettingType.String) {
            if (!value) {
                settingValue = '';
            } else {
                settingValue = value.toString();
            }
        } else {
            settingValue = JSON.stringify(value);
        }

        return {
            settingKey: key,
            settingValue: settingValue
        };
    }

    /** 中文说明：当已同步设置在本地变化时向服务端发送单项增量写回，未启用或不支持的 key 直接跳过。 */
    function updateUserApplicationCloudSettingValue(key: string, value: string | number | boolean | Record<string, boolean>): void {
        if (!runtime.syncedAppSettings.value || !runtime.syncedAppSettings.value[key]) {
            return;
        }

        const settingType = ALL_ALLOWED_CLOUD_SYNC_APP_SETTING_KEY_TYPES[key];

        if (!settingType) {
            return;
        }

        const settingValue = isString(value) ? value : JSON.stringify(value);

        services.updateUserApplicationCloudSettings({
            settings: [{
                settingKey: key,
                settingValue: settingValue
            }],
            fullUpdate: false
        }).then(response => {
            const data = response.data;

            if (!data || !data.success || !data.result) {
                logger.debug(`failed to update user application cloud setting "${key}" with value "${settingValue}"`);
                return;
            }

            logger.debug(`update user application cloud setting "${key}" with value "${settingValue}" successfully`);
        }).catch(error => {
            logger.debug(`failed to update user application cloud setting "${key}" with value "${settingValue}"`, error);
        });
    }

    /** 中文说明：按选中的 setting key 生成 full update payload，过滤掉不支持或格式非法的 key。 */
    function createApplicationCloudSettings(applicationSettingKeys: string[]): ApplicationCloudSetting[] {
        if (!applicationSettingKeys || applicationSettingKeys.length < 1) {
            return [];
        }

        const settings: ApplicationCloudSetting[] = [];

        for (const settingKey of applicationSettingKeys) {
            const cloudSetting = createUserApplicationCloudSetting(settingKey);

            if (cloudSetting) {
                settings.push(cloudSetting);
            }
        }

        return settings;
    }

    /** 中文说明：应用服务端返回的云端设置，按声明的 setting type 解析并写回本地设置，同时刷新 synced key map。 */
    function setApplicationSettingsFromCloudSettings(cloudSettings?: ApplicationCloudSetting[]): void {
        if (!cloudSettings || cloudSettings.length < 1) {
            runtime.syncedAppSettings.value = {};
            return;
        }

        runtime.syncedAppSettings.value = arrayItemToObjectField(cloudSettings.map(item => item.settingKey), true);

        for (const setting of cloudSettings) {
            if (!setting || !setting.settingKey) {
                continue;
            }

            const settingType = ALL_ALLOWED_CLOUD_SYNC_APP_SETTING_KEY_TYPES[setting.settingKey];

            if (!settingType) {
                logger.warn(`cannot load application cloud setting "${setting.settingKey}", because it is not supported to sync`);
                continue;
            }

            if (settingType === UserApplicationCloudSettingType.String) {
                updateApplicationSettingsValueAndAppSettingsFromCloudSetting(setting.settingKey, setting.settingValue);
            } else if (settingType === UserApplicationCloudSettingType.Number) {
                const value = parseFloat(setting.settingValue);

                if (isNaN(value)) {
                    logger.warn(`cannot load application cloud setting "${setting.settingKey}", because it has invalid number value`);
                    continue;
                }

                updateApplicationSettingsValueAndAppSettingsFromCloudSetting(setting.settingKey, value);
            } else if (settingType === UserApplicationCloudSettingType.Boolean) {
                if (setting.settingValue !== 'true' && setting.settingValue !== 'false') {
                    logger.warn(`cannot load application cloud setting "${setting.settingKey}", because it has invalid boolean value`);
                    continue;
                }

                updateApplicationSettingsValueAndAppSettingsFromCloudSetting(setting.settingKey, setting.settingValue === 'true');
            } else if (settingType === UserApplicationCloudSettingType.StringBooleanMap) {
                try {
                    const map = JSON.parse(setting.settingValue);
                    let isValid = isObject(map);

                    if (isValid) {
                        for (const value of values(map)) {
                            if (!isBoolean(value)) {
                                isValid = false;
                                break;
                            }
                        }
                    }

                    if (!isValid) {
                        logger.warn(`cannot load application cloud setting "${setting.settingKey}", because it has invalid map value`);
                        continue;
                    }

                    updateApplicationSettingsValueAndAppSettingsFromCloudSetting(setting.settingKey, map as Record<string, boolean>);
                } catch (error) {
                    logger.warn(`cannot load application cloud setting "${setting.settingKey}", because cannot parse JSON (${error})`);
                }
            } else {
                logger.warn(`cannot load application cloud setting "${setting.settingKey}", because it has unknown type "${settingType}"`);
            }
        }
    }

    function updateApplicationSyncSettingKeys(settingKeys?: string[]): void {
        if (!settingKeys || settingKeys.length < 1) {
            runtime.syncedAppSettings.value = {};
        } else {
            runtime.syncedAppSettings.value = arrayItemToObjectField(settingKeys, true);
        }
    }

    return {
        updateUserApplicationCloudSettingValue,
        createApplicationCloudSettings,
        setApplicationSettingsFromCloudSettings,
        updateApplicationSyncSettingKeys
    };
}
