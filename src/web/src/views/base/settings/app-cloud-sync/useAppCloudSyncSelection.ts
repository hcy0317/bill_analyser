import { computed, ref } from 'vue';

import { useSettingsStore } from '@/stores/setting.ts';

import { keysIfValueEquals } from '@/core/base.ts';

import {
    ALL_APPLICATION_CLOUD_SETTINGS,
    type CategorizedApplicationCloudSettingItems,
    type UserApplicationCloudSettingsPayload
} from './cloudSettingCatalog.ts';

/** 中文说明：创建应用设置云同步选择状态，统一桌面/移动页面的全选、反选、半选和服务端 settings 应用逻辑。 */
export function useAppCloudSyncSelection() {
    const settingsStore = useSettingsStore();

    const loading = ref<boolean>(false);
    const enabling = ref<boolean>(false);
    const disabling = ref<boolean>(false);
    const enabledApplicationCloudSettings = ref<Record<string, boolean>>(Object.assign({}, settingsStore.syncedAppSettings));

    const isEnableCloudSync = computed<boolean>(() => settingsStore.enableApplicationCloudSync);

    const hasEnabledApplicationCloudSettings = computed<boolean>(() => {
        for (const _ of keysIfValueEquals(enabledApplicationCloudSettings.value, true)) {
            return true;
        }

        return false;
    });

    const enabledApplicationCloudSettingKeys = computed<string[]>(() => {
        const keys: string[] = [];

        for (const key of keysIfValueEquals(enabledApplicationCloudSettings.value, true)) {
            keys.push(key);
        }

        return keys;
    });

    function isAllSettingsSelected(categorizedItems: CategorizedApplicationCloudSettingItems): boolean {
        for (const item of categorizedItems.items) {
            if (!enabledApplicationCloudSettings.value[item.settingKey]) {
                return false;
            }
        }

        return true;
    }

    function hasSettingSelectedButNotAllChecked(categorizedItems: CategorizedApplicationCloudSettingItems): boolean {
        let checkedCount = 0;

        for (const item of categorizedItems.items) {
            if (!enabledApplicationCloudSettings.value[item.settingKey]) {
                checkedCount++;
            }
        }

        return checkedCount > 0 && checkedCount < categorizedItems.items.length;
    }

    function updateSettingsSelected(categorizedItems: CategorizedApplicationCloudSettingItems, value: boolean): void {
        for (const item of categorizedItems.items) {
            enabledApplicationCloudSettings.value[item.settingKey] = value;
        }
    }

    function selectAllSettings(): void {
        for (const categorizedItems of ALL_APPLICATION_CLOUD_SETTINGS) {
            for (const item of categorizedItems.items) {
                enabledApplicationCloudSettings.value[item.settingKey] = true;
            }
        }
    }

    function selectNoneSettings(): void {
        for (const categorizedItems of ALL_APPLICATION_CLOUD_SETTINGS) {
            for (const item of categorizedItems.items) {
                enabledApplicationCloudSettings.value[item.settingKey] = false;
            }
        }
    }

    function selectInvertSettings(): void {
        for (const categorizedItems of ALL_APPLICATION_CLOUD_SETTINGS) {
            for (const item of categorizedItems.items) {
                enabledApplicationCloudSettings.value[item.settingKey] = !enabledApplicationCloudSettings.value[item.settingKey];
            }
        }
    }

    /** 中文说明：应用服务端返回的云同步 settings，成功时同步本地 settings store，false/空数组时清空选择。 */
    function setUserApplicationCloudSettings(settings: UserApplicationCloudSettingsPayload) {
        if (settings && settings.length > 0) {
            settingsStore.setApplicationSettingsFromCloudSettings(settings);

            for (const setting of settings) {
                if (setting && setting.settingKey) {
                    enabledApplicationCloudSettings.value[setting.settingKey] = true;
                }
            }
        } else {
            settingsStore.setApplicationSettingsFromCloudSettings(undefined);
            enabledApplicationCloudSettings.value = {};
        }
    }

    return {
        // 常量
        ALL_APPLICATION_CLOUD_SETTINGS,
        // 状态
        loading,
        enabling,
        disabling,
        enabledApplicationCloudSettings,
        // 计算状态
        isEnableCloudSync,
        hasEnabledApplicationCloudSettings,
        enabledApplicationCloudSettingKeys,
        // 函数
        isAllSettingsSelected,
        hasSettingSelectedButNotAllChecked,
        updateSettingsSelected,
        selectAllSettings,
        selectNoneSettings,
        selectInvertSettings,
        setUserApplicationCloudSettings
    };
}
