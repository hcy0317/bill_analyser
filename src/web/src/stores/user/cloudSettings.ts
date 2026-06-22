import type { ApplicationCloudSetting } from '@/core/setting.ts';

import logger from '@/lib/logger.ts';
import services from '@/lib/services.ts';

type UserCloudSettingsStore = {
    createApplicationCloudSettings: (enabledSettingKeys: string[]) => ApplicationCloudSetting[];
    updateApplicationSyncSettingKeys: (enabledSettingKeys?: string[]) => void;
};

/** 中文说明：创建用户应用云同步设置动作，负责服务端配置读写和本地同步 key 刷新。 */
export function createUserCloudSettingsActions(settingsStore: UserCloudSettingsStore) {
    /** 中文说明：从服务端读取用户应用云同步设置，保留 false 结果表示服务端未启用或无配置。 */
    function getUserApplicationCloudSettings(): Promise<ApplicationCloudSetting[] | false> {
        return new Promise((resolve, reject) => {
            services.getUserApplicationCloudSettings().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    resolve(data.result);
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to load user synchronized application settings', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve user synchronized application settings' });
                } else {
                    reject(error);
                }
            });
        });
    }

    /** 中文说明：按当前选择的 setting key 全量覆盖服务端云同步配置，并刷新本地 synced key map。 */
    function fullUpdateUserApplicationCloudSettings(enabledSettingKeys: string[]): Promise<boolean> {
        const settings = settingsStore.createApplicationCloudSettings(enabledSettingKeys);

        return new Promise((resolve, reject) => {
            services.updateUserApplicationCloudSettings({
                settings: settings,
                fullUpdate: true
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to update user synchronized application settings' });
                    return;
                }

                settingsStore.updateApplicationSyncSettingKeys(enabledSettingKeys);
                resolve(data.result);
            }).catch(error => {
                logger.error('failed to update user synchronized application settings', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to update user synchronized application settings' });
                } else {
                    reject(error);
                }
            });
        });
    }

    /** 中文说明：禁用用户应用设置云同步，并清空本地 synced key map，避免后续本地变更继续写回。 */
    function disableUserApplicationCloudSettings(): Promise<boolean> {
        return new Promise((resolve, reject) => {
            services.disableUserApplicationCloudSettings().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to disable user synchronized application settings' });
                    return;
                }

                settingsStore.updateApplicationSyncSettingKeys(undefined);
                resolve(data.result);
            }).catch(error => {
                logger.error('failed to disable user synchronized application settings', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to disable user synchronized application settings' });
                } else {
                    reject(error);
                }
            });
        });
    }

    return {
        getUserApplicationCloudSettings,
        fullUpdateUserApplicationCloudSettings,
        disableUserApplicationCloudSettings
    };
}
