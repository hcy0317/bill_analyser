import { ref, type ComputedRef, type Ref } from 'vue';
import type { Router } from 'framework7/types';

import type { ApplicationCloudSetting } from '@/core/setting.ts';
import { useI18nUIComponents, showLoading, hideLoading } from '@/lib/ui/mobile.ts';
import { useUserStore } from '@/stores/user.ts';

export interface MobileAppCloudSyncActionState {
    readonly loading: Ref<boolean>;
    readonly enabling: Ref<boolean>;
    readonly disabling: Ref<boolean>;
    readonly enabledApplicationCloudSettings: Ref<Record<string, boolean>>;
    readonly enabledApplicationCloudSettingKeys: ComputedRef<string[]>;
    readonly f7router: Router.Router;
    setUserApplicationCloudSettings: (settings: ApplicationCloudSetting[] | false) => void;
}

/** 中文说明：封装移动端应用设置云同步动作，集中处理 loading、toast、routeBackOnError 和本地选择清空。 */
export function useMobileAppCloudSyncActions(state: MobileAppCloudSyncActionState) {
    const userStore = useUserStore();
    const { showToast, routeBackOnError } = useI18nUIComponents();
    const loadingError = ref<unknown | null>(null);
    const showMoreActionSheet = ref<boolean>(false);

    function init(): void {
        state.loading.value = true;

        userStore.getUserApplicationCloudSettings().then(response => {
            state.setUserApplicationCloudSettings(response);
            state.loading.value = false;
        }).catch(error => {
            if (error.processed) {
                state.loading.value = false;
            } else {
                loadingError.value = error;
                showToast(error.message || error);
            }
        });
    }

    function enable(update: boolean): void {
        state.enabling.value = true;
        showLoading(() => state.enabling.value);

        userStore.fullUpdateUserApplicationCloudSettings(state.enabledApplicationCloudSettingKeys.value).then(() => {
            state.enabling.value = false;
            hideLoading();

            if (!update) {
                showToast('Settings sync has been enabled');
            } else {
                showToast('Synchronized settings have been updated');
            }
        }).catch(error => {
            state.enabling.value = false;
            hideLoading();

            if (!error.processed) {
                showToast(error.message || error);
            }
        });
    }

    function disable(): void {
        state.disabling.value = true;
        showLoading(() => state.disabling.value);

        userStore.disableUserApplicationCloudSettings().then(() => {
            state.enabledApplicationCloudSettings.value = {};
            state.disabling.value = false;
            hideLoading();
            showToast('Settings sync has been disabled');
        }).catch(error => {
            state.disabling.value = false;
            hideLoading();

            if (!error.processed) {
                showToast(error.message || error);
            }
        });
    }

    function onPageAfterIn(): void {
        routeBackOnError(state.f7router, loadingError);
    }

    return {
        loadingError,
        showMoreActionSheet,
        init,
        enable,
        disable,
        onPageAfterIn
    };
}
