<template>
    <div data-testid="desktop.user-settings.page">
        <v-tabs show-arrows v-model="activeTab">
            <v-tab value="basicSetting" @click="pushRouter('basicSetting')">
                <v-icon size="20" start :icon="mdiAccountOutline"/>
                {{ tt('Basic') }}
            </v-tab>
            <v-tab value="securitySetting" @click="pushRouter('securitySetting')">
                <v-icon size="20" start :icon="mdiLockOpenOutline"/>
                {{ tt('Security') }}
            </v-tab>
            <v-tab value="twoFactorSetting" @click="pushRouter('twoFactorSetting')">
                <v-icon size="20" start :icon="mdiOnepassword"/>
                {{ tt('Two-Factor Authentication') }}
            </v-tab>
            <v-tab value="dataManagementSetting" @click="pushRouter('dataManagementSetting')">
                <v-icon size="20" start :icon="mdiDatabaseCogOutline"/>
                {{ tt('Data Management') }}
            </v-tab>
        </v-tabs>

        <div class="mt-4">
            <user-basic-setting-tab v-if="activeTab === 'basicSetting'" />
            <user-security-setting-tab v-else-if="activeTab === 'securitySetting'" />
            <user-two-factor-auth-setting-tab v-else-if="activeTab === 'twoFactorSetting'" />
            <user-data-management-setting-tab v-else-if="activeTab === 'dataManagementSetting'" />
        </div>
    </div>
</template>

<script setup lang="ts">
import UserBasicSettingTab from './settings/tabs/UserBasicSettingTab.vue';
import UserSecuritySettingTab from './settings/tabs/UserSecuritySettingTab.vue';
import UserTwoFactorAuthSettingTab from './settings/tabs/UserTwoFactorAuthSettingTab.vue';
import UserDataManagementSettingTab from './settings/tabs/UserDataManagementSettingTab.vue';

import { ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';

import { useI18n } from '@/locales/helpers.ts';

import {
    mdiAccountOutline,
    mdiLockOpenOutline,
    mdiOnepassword,
    mdiDatabaseCogOutline
} from '@mdi/js';

const props = defineProps<{
    initTab?: string;
}>();

const route = useRoute();
const router = useRouter();

const { tt } = useI18n();

const ALL_TABS: string[] = [
    'basicSetting',
    'securitySetting',
    'twoFactorSetting',
    'dataManagementSetting'
];

function normalizeTab(value?: string): string {
    return value && ALL_TABS.indexOf(value) >= 0 ? value : 'basicSetting';
}

const activeTab = ref<string>(normalizeTab(String(route.query['tab'] || props.initTab || 'basicSetting')));

const pushRouter = (tab: string) => {
    const normalizedTab = normalizeTab(tab);

    if (activeTab.value !== normalizedTab) {
        activeTab.value = normalizedTab;
    }

    if (route.query['tab'] !== normalizedTab) {
        router.replace(`/user/settings?tab=${normalizedTab}`);
    }
};

watch(() => route.query['tab'], (tabValue) => {
    const nextTab = normalizeTab(typeof tabValue === 'string' ? tabValue : props.initTab);

    if (activeTab.value !== nextTab) {
        activeTab.value = nextTab;
    }
}, { immediate: true });
</script>
