import services from '@/lib/services.ts';

/**
 * 中文说明：生成 OAuth2 登录跳转地址，保留 platform 和 clientSessionId 查询字段供回调校验。
 */
export function generateOAuth2LoginUrl(platform: 'mobile' | 'desktop', clientSessionId: string): string {
    return services.generateOAuth2LoginUrl(platform, clientSessionId);
}

/**
 * 中文说明：生成 OAuth2 账号绑定跳转地址，与登录地址共享 session 校验字段但使用 link 场景。
 */
export function generateOAuth2LinkUrl(platform: 'mobile' | 'desktop', clientSessionId: string): string {
    return services.generateOAuth2LinkUrl(platform, clientSessionId);
}
