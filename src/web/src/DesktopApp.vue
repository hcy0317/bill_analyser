<template>
    <v-app>
        <router-view />
    </v-app>
    <v-snackbar class="cursor-pointer" color="notification-background" location="top"
                :multi-line="true" :timeout="-1" :close-on-content-click="true" v-model="showNotification">
        <v-tooltip activator="parent">{{ tt('Click to close') }}</v-tooltip>
        <div class="d-inline-flex">
            <img alt="logo" class="notification-logo" :src="APPLICATION_LOGO_PATH" />
            <span class="ms-2">{{ tt('global.app.title') }}</span>
        </div>
        <div>
            {{ currentNotificationContent }}
        </div>
    </v-snackbar>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';

import { useTheme } from 'vuetify';
import { register } from 'register-service-worker';

import { useI18n } from '@/locales/helpers.ts';

import { useRootStore } from '@/stores/index.ts';
import { useSettingsStore } from '@/stores/setting.ts';
import { useUserStore } from '@/stores/user.ts';
import { useExchangeRatesStore } from '@/stores/exchangeRates.ts';

import { APPLICATION_LOGO_PATH } from '@/consts/asset.ts';
import { SYSTEM_THEME_PREFERENCE, ThemeType, resolveThemePreference } from '@/core/theme.ts';
import { isProduction } from '@/lib/version.ts';
import { initMapProvider } from '@/lib/map/index.ts';
import { isUserLogined, isUserUnlocked } from '@/lib/userstate.ts';
import { getSystemTheme, setExpenseAndIncomeAmountColor } from '@/lib/ui/common.ts';
import logger from '@/lib/logger.ts';

const { tt, getCurrentLanguageInfo, initLocale } = useI18n();

const theme = useTheme();

const rootStore = useRootStore();
const settingsStore = useSettingsStore();
const userStore = useUserStore();
const exchangeRatesStore = useExchangeRatesStore();

const initialRoutePath: string = (() => {
    if (!window.location.hash) {
        return '/';
    }

    const hash = window.location.hash;
    const hashIndex = hash.indexOf('#/');

    if (hashIndex < 0) {
        return '/';
    }

    const routePath = hash.substring(hashIndex + 1);
    const queryIndex = routePath.indexOf('?');

    if (queryIndex < 0) {
        return routePath;
    }

    return routePath.substring(0, queryIndex);
})();

const showNotification = ref<boolean>(false);

const currentNotificationContent = computed<string | null>(() => rootStore.currentNotification);

onMounted(() => {
    document.addEventListener('DOMContentLoaded', () => {
        const languageInfo = getCurrentLanguageInfo();
        initMapProvider(languageInfo?.alternativeLanguageTag);
    });
});

watch(currentNotificationContent, (newValue) => {
    showNotification.value = !!newValue;
});

theme.change(resolveThemePreference(settingsStore.appSettings.theme, getSystemTheme()));

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function (e) {
    if (settingsStore.appSettings.theme === SYSTEM_THEME_PREFERENCE) {
        if (e.matches) {
            theme.change(ThemeType.Dark);
        } else {
            theme.change(ThemeType.Light);
        }
    }
});

const localeDefaultSettings = initLocale(userStore.currentUserLanguage, settingsStore.appSettings.timeZone);
settingsStore.updateLocalizedDefaultSettings(localeDefaultSettings);

setExpenseAndIncomeAmountColor(userStore.currentUserExpenseAmountColor, userStore.currentUserIncomeAmountColor);

if (isUserLogined() && initialRoutePath !== '/verify_email' && initialRoutePath !== '/resetpassword' && initialRoutePath !== '/oauth2_callback') {
    if (!settingsStore.appSettings.applicationLock || isUserUnlocked()) {
        // 禁用自动刷新令牌 - 令牌有效期为 7 天，启动时不可能过期
        // 如果真的过期，接口请求会返回 401，由响应拦截器处理
        logger.info('[DesktopApp] Skipping auto token refresh on app startup - token valid for 7 days');

        // 自动刷新汇率数据
        if (settingsStore.appSettings.autoUpdateExchangeRatesData) {
            exchangeRatesStore.getLatestExchangeRates({ silent: true, force: false });
        }
    }
}

if (isProduction()) {
    register('./sw.js', {
        registrationOptions: {
            scope: './'
        }
    });
}
</script>

<style>
.notification-logo {
    width: 1.2rem;
    height: 1.2rem;
}
</style>
