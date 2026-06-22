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

    /** 中文说明：判断某个设置分组是否已全选，供桌面/移动复用 checkbox 状态。 */
    function isAllSettingsSelected(categorizedItems: CategorizedApplicationCloudSettingItems): boolean {
        for (const item of categorizedItems.items) {
            if (!enabledApplicationCloudSettings.value[item.settingKey]) {
                return false;
            }
        }

        return true;
    }

    /** 中文说明：判断某个设置分组是否处于半选状态，保持分组选择 UI 一致。 */
    function hasSettingSelectedButNotAllChecked(categorizedItems: CategorizedApplicationCloudSettingItems): boolean {
        let checkedCount = 0;

        for (const item of categorizedItems.items) {
            if (!enabledApplicationCloudSettings.value[item.settingKey]) {
                checkedCount++;
            }
        }

        return checkedCount > 0 && checkedCount < categorizedItems.items.length;
    }

    /** 中文说明：批量切换某个设置分组的选择状态，供全选/清空分组操作复用。 */
    function updateSettingsSelected(categorizedItems: CategorizedApplicationCloudSettingItems, value: boolean): void {
        for (const item of categorizedItems.items) {
            enabledApplicationCloudSettings.value[item.settingKey] = value;
        }
    }

    /** 中文说明：选择全部可同步应用设置，生成后续 full update 所需的本地选择状态。 */
    function selectAllSettings(): void {
        for (const categorizedItems of ALL_APPLICATION_CLOUD_SETTINGS) {
            for (const item of categorizedItems.items) {
                enabledApplicationCloudSettings.value[item.settingKey] = true;
            }
        }
    }

    /** 中文说明：取消选择全部应用设置，保留云同步开关状态但清空待同步 key。 */
    function selectNoneSettings(): void {
        for (const categorizedItems of ALL_APPLICATION_CLOUD_SETTINGS) {
            for (const item of categorizedItems.items) {
                enabledApplicationCloudSettings.value[item.settingKey] = false;
            }
        }
    }

    /** 中文说明：反选全部应用设置，用于批量调整同步 key 时快速切换选择状态。 */
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
