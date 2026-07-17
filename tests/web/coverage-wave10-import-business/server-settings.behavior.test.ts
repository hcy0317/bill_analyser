import { beforeEach, describe, expect, test } from '@jest/globals';

import {
    getAmapApiExternalProxyUrl,
    getAmapApplicationKey,
    getAmapApplicationSecret,
    getAmapSecurityVerificationMethod,
    getBaiduMapAK,
    getCustomMapAnnotationLayerUrl,
    getCustomMapDefaultZoomLevel,
    getCustomMapMaxZoomLevel,
    getCustomMapMinZoomLevel,
    getCustomMapTileLayerUrl,
    getExchangeRatesRequestTimeout,
    getGoogleMapAPIKey,
    getLoginPageTips,
    getMapProvider,
    getOAuth2Provider,
    getOIDCCustomDisplayNames,
    getTianDiTuMapAPIKey,
    getTomTomMapAPIKey,
    isAPITokenEnabled,
    isCustomMapAnnotationLayerDataFetchProxyEnabled,
    isDataExportingEnabled,
    isDataImportingEnabled,
    isInternalAuthEnabled,
    isMapDataFetchProxyEnabled,
    isMCPServerEnabled,
    isOAuth2Enabled,
    isTransactionFromAIImageRecognitionEnabled,
    isTransactionPicturesEnabled,
    isUserForgetPasswordEnabled,
    isUserRegistrationEnabled,
    isUserScheduledTransactionEnabled,
    isUserVerifyEmailEnabled
} from '@/lib/server_settings.ts';

type ServerSettings = Record<string, string | number | boolean | Record<string, string> | null | undefined>;

function installSettings(settings: ServerSettings): void {
    (window as unknown as { bill_analyser_SERVER_SETTINGS?: ServerSettings }).bill_analyser_SERVER_SETTINGS = settings;
}

describe('server setting accessors', () => {
    beforeEach(() => {
        delete (window as unknown as { bill_analyser_SERVER_SETTINGS?: ServerSettings }).bill_analyser_SERVER_SETTINGS;
    });

    test('uses secure feature defaults and documented map zoom defaults when settings are absent', () => {
        expect(isInternalAuthEnabled()).toBe(true);
        expect([
            isOAuth2Enabled(),
            isUserRegistrationEnabled(),
            isUserForgetPasswordEnabled(),
            isAPITokenEnabled(),
            isUserVerifyEmailEnabled(),
            isTransactionPicturesEnabled(),
            isUserScheduledTransactionEnabled(),
            isDataExportingEnabled(),
            isDataImportingEnabled(),
            isMCPServerEnabled(),
            isTransactionFromAIImageRecognitionEnabled(),
            isMapDataFetchProxyEnabled(),
            isCustomMapAnnotationLayerDataFetchProxyEnabled()
        ]).toEqual(new Array(13).fill(false));
        expect(getCustomMapMinZoomLevel()).toBe(1);
        expect(getCustomMapMaxZoomLevel()).toBe(18);
        expect(getCustomMapDefaultZoomLevel()).toBe(14);
    });

    test('projects enabled capabilities, identity labels and provider credentials without rewriting values', () => {
        const oidcNames = { google: 'Google Workspace', custom: 'Corporate SSO' };
        const loginTips = { en: 'Use your work account', zh: '使用工作账号' };
        installSettings({
            a: 0,
            o: 1,
            r: 1,
            f: 1,
            t: 1,
            v: 1,
            p: 1,
            s: 1,
            e: 1,
            i: 1,
            mcp: 1,
            llmt: 1,
            op: 'oidc',
            ocn: oidcNames,
            lpt: loginTips,
            m: 'amap',
            mp: 1,
            cmsu: 'https://tiles.example/{z}/{x}/{y}',
            cmau: 'https://annotations.example/data.json',
            cmap: 1,
            cmzl: '3-19-12',
            tmak: 'tomtom-key',
            tdak: 'tianditu-key',
            gmak: 'google-key',
            bmak: 'baidu-key',
            amak: 'amap-key',
            amsv: 'secret',
            amep: 'https://proxy.example/amap',
            amas: 'amap-secret',
            errt: 4500
        });

        expect(isInternalAuthEnabled()).toBe(false);
        expect([
            isOAuth2Enabled(),
            isUserRegistrationEnabled(),
            isUserForgetPasswordEnabled(),
            isAPITokenEnabled(),
            isUserVerifyEmailEnabled(),
            isTransactionPicturesEnabled(),
            isUserScheduledTransactionEnabled(),
            isDataExportingEnabled(),
            isDataImportingEnabled(),
            isMCPServerEnabled(),
            isTransactionFromAIImageRecognitionEnabled(),
            isMapDataFetchProxyEnabled(),
            isCustomMapAnnotationLayerDataFetchProxyEnabled()
        ]).toEqual(new Array(13).fill(true));
        expect(getOAuth2Provider()).toBe('oidc');
        expect(getOIDCCustomDisplayNames()).toBe(oidcNames);
        expect(getLoginPageTips()).toBe(loginTips);
        expect(getMapProvider()).toBe('amap');
        expect(getCustomMapTileLayerUrl()).toBe('https://tiles.example/{z}/{x}/{y}');
        expect(getCustomMapAnnotationLayerUrl()).toBe('https://annotations.example/data.json');
        expect([
            getCustomMapMinZoomLevel(),
            getCustomMapMaxZoomLevel(),
            getCustomMapDefaultZoomLevel()
        ]).toEqual([3, 19, 12]);
        expect([
            getTomTomMapAPIKey(),
            getTianDiTuMapAPIKey(),
            getGoogleMapAPIKey(),
            getBaiduMapAK(),
            getAmapApplicationKey(),
            getAmapSecurityVerificationMethod(),
            getAmapApiExternalProxyUrl(),
            getAmapApplicationSecret()
        ]).toEqual([
            'tomtom-key',
            'tianditu-key',
            'google-key',
            'baidu-key',
            'amap-key',
            'secret',
            'https://proxy.example/amap',
            'amap-secret'
        ]);
        expect(getExchangeRatesRequestTimeout()).toBe(4500);
    });

    test('accepts the legacy AI-recognition flag and fills only missing zoom slots', () => {
        installSettings({ air: 1, cmzl: '5--' });

        expect(isTransactionFromAIImageRecognitionEnabled()).toBe(true);
        expect(getCustomMapMinZoomLevel()).toBe(5);
        expect(getCustomMapMaxZoomLevel()).toBe(18);
        expect(getCustomMapDefaultZoomLevel()).toBe(14);
    });
});
