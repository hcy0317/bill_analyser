import services from '@/lib/services.ts';

export function generateOAuth2LoginUrl(platform: 'mobile' | 'desktop', clientSessionId: string): string {
    return services.generateOAuth2LoginUrl(platform, clientSessionId);
}

export function generateOAuth2LinkUrl(platform: 'mobile' | 'desktop', clientSessionId: string): string {
    return services.generateOAuth2LinkUrl(platform, clientSessionId);
}
