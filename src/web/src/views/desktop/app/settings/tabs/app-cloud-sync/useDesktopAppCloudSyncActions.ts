import type { ComputedRef, Ref } from 'vue';

import type { ErrorResponse } from '@/core/api.ts';
import type { ApplicationCloudSetting } from '@/core/setting.ts';
import { useUserStore } from '@/stores/user.ts';

type DesktopAppCloudSyncError = string | { message: string } | { error: ErrorResponse };

interface DesktopAppCloudSyncMessageTarget {
    showError: (error: DesktopAppCloudSyncError) => void;
    showMessage: (message: string) => void;
}

export interface DesktopAppCloudSyncActionState {
    readonly loading: Ref<boolean>;
    readonly enabling: Ref<boolean>;
    readonly disabling: Ref<boolean>;
    readonly enabledApplicationCloudSettings: Ref<Record<string, boolean>>;
    readonly enabledApplicationCloudSettingKeys: ComputedRef<string[]>;
    readonly snackbar: Readonly<Ref<DesktopAppCloudSyncMessageTarget | null>>;
    setUserApplicationCloudSettings: (settings: ApplicationCloudSetting[] | false) => void;
}

/** 中文说明：封装桌面应用设置云同步的加载、启用/更新和禁用动作，保留 snackbar 错误与成功提示合同。 */
export function useDesktopAppCloudSyncActions(state: DesktopAppCloudSyncActionState) {
    const userStore = useUserStore();

    /** 中文说明：初始化桌面端云同步设置，加载服务端配置并同步本地选择状态。 */
    function init(): void {
        state.loading.value = true;

        userStore.getUserApplicationCloudSettings().then(response => {
            state.setUserApplicationCloudSettings(response);
            state.loading.value = false;
        }).catch(error => {
            state.loading.value = false;

            if (!error.processed) {
                state.snackbar.value?.showError(error as DesktopAppCloudSyncError);
            }
        });
    }

    /** 中文说明：桌面端启用或更新云同步配置，提交当前选择并展示 snackbar 反馈。 */
    function enable(update: boolean): void {
        state.enabling.value = true;

        userStore.fullUpdateUserApplicationCloudSettings(state.enabledApplicationCloudSettingKeys.value).then(() => {
            state.enabling.value = false;

            if (!update) {
                state.snackbar.value?.showMessage('Settings sync has been enabled');
            } else {
                state.snackbar.value?.showMessage('Synchronized settings have been updated');
            }
        }).catch(error => {
            state.enabling.value = false;

            if (!error.processed) {
                state.snackbar.value?.showError(error as DesktopAppCloudSyncError);
            }
        });
    }

    /** 中文说明：桌面端禁用云同步配置，成功后清空本地选择状态并展示反馈。 */
    function disable(): void {
        state.disabling.value = true;

        userStore.disableUserApplicationCloudSettings().then(() => {
            state.enabledApplicationCloudSettings.value = {};
            state.disabling.value = false;
            state.snackbar.value?.showMessage('Settings sync has been disabled');
        }).catch(error => {
            state.disabling.value = false;

            if (!error.processed) {
                state.snackbar.value?.showError(error as DesktopAppCloudSyncError);
            }
        });
    }

    return {
        init,
        enable,
        disable
    };
}
