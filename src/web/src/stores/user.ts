import { defineStore } from 'pinia';

import { useSettingsStore } from './setting.ts';
import { getUserAvatarUrl } from './user/avatar.ts';
import { createUserBasicInfoState } from './user/basicInfo.ts';
import { createUserCloudSettingsActions } from './user/cloudSettings.ts';
import { createUserDataManagementActions } from './user/dataManagement.ts';
import { createUserProfileActions } from './user/profileActions.ts';
import { createUserSettingsBundleActions } from './user/settingsBundle.ts';

/** 用户 store facade，保持登录态、资料、云同步、数据管理和设置包动作的既有导出合同。 */
export const useUserStore = defineStore('user', () => {
    const settingsStore = useSettingsStore();

    const basicInfoState = createUserBasicInfoState(settingsStore, getUserAvatarUrl);
    const profileActions = createUserProfileActions(basicInfoState.storeUserBasicInfo);
    const cloudSettingsActions = createUserCloudSettingsActions(settingsStore);
    const dataManagementActions = createUserDataManagementActions();
    const settingsBundleActions = createUserSettingsBundleActions();

    return {
        // 状态与计算状态
        ...basicInfoState,
        // 资料与头像
        ...profileActions,
        // 云同步设置
        ...cloudSettingsActions,
        // 用户数据导出与统计
        ...dataManagementActions,
        // 设置包导入导出
        ...settingsBundleActions,
        getUserAvatarUrl
    };
});
