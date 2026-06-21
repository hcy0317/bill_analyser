import type {
    UserBasicInfo,
    UserProfileResponse,
    UserProfileUpdateResponse
} from '@/models/user.ts';

import { isObject } from '@/lib/common.ts';
import logger from '@/lib/logger.ts';
import services from '@/lib/services.ts';
import { normalizeUserBasicInfo } from '@/models/user.ts';

type StoreUserBasicInfo = (userInfo: UserBasicInfo) => void;

/** 中文说明：创建 profile、交易编辑范围和头像更新动作，统一在成功响应后刷新 basic info 缓存。 */
export function createUserProfileActions(storeUserBasicInfo: StoreUserBasicInfo) {
    function getCurrentUserProfile(): Promise<UserProfileResponse> {
        return new Promise((resolve, reject) => {
            services.getProfile().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to retrieve user profile' });
                    return;
                }

                resolve(normalizeUserBasicInfo(data.result) as UserProfileResponse);
            }).catch(error => {
                logger.error('failed to retrieve user profile', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve user profile' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function updateUserTransactionEditScope({ transactionEditScope }: { transactionEditScope: number }): Promise<UserProfileUpdateResponse> {
        return new Promise((resolve, reject) => {
            services.updateProfile({ transactionEditScope }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result || !data.result.user || !isObject(data.result.user)) {
                    reject({ message: 'Unable to update editable transaction range' });
                    return;
                }

                storeUserBasicInfo(data.result.user);

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to save editable transaction range', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to update editable transaction range' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function updateUserAvatar({ avatarFile }: { avatarFile: File }): Promise<UserProfileResponse> {
        return new Promise((resolve, reject) => {
            services.updateAvatar({ avatarFile }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to update user avatar' });
                    return;
                }

                storeUserBasicInfo(data.result);

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to update user avatar', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to update user avatar' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function removeUserAvatar(): Promise<UserProfileResponse> {
        return new Promise((resolve, reject) => {
            services.removeAvatar().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to remove user avatar' });
                    return;
                }

                storeUserBasicInfo(data.result);

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to remove user avatar', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to remove user avatar' });
                } else {
                    reject(error);
                }
            });
        });
    }

    return {
        getCurrentUserProfile,
        updateUserTransactionEditScope,
        updateUserAvatar,
        removeUserAvatar
    };
}
