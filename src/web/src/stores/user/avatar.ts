import type { UserBasicInfo } from '@/models/user.ts';

import {
    isObject,
    isString
} from '@/lib/common.ts';
import services from '@/lib/services.ts';

export function getUserAvatarUrl(userInfoOrAvatarUrl: UserBasicInfo | string | null, disableBrowserCache: boolean | string): string | null {
    let avatarUrl = '';

    if (isObject(userInfoOrAvatarUrl)) {
        avatarUrl = userInfoOrAvatarUrl.avatar;
    } else if (isString(userInfoOrAvatarUrl)) {
        avatarUrl = userInfoOrAvatarUrl;
    }

    if (!avatarUrl) {
        return null;
    }

    return services.getInternalAvatarUrlWithToken(avatarUrl, disableBrowserCache);
}
