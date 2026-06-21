import type { UserBasicInfo } from '@/models/user.ts';

import {
    isObject,
    isString
} from '@/lib/common.ts';
import services from '@/lib/services.ts';

/** 中文说明：把用户头像路径转换成带认证 token 的内部头像 URL，并按需附加缓存破坏参数。 */
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
