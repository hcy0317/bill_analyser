<template>
    <f7-page @page:afterin="onPageAfterIn">
        <f7-navbar>
            <f7-nav-left :back-link="tt('Back')"></f7-nav-left>
            <f7-nav-title :title="tt('Settings Sync')"></f7-nav-title>
            <f7-nav-right>
                <f7-link icon-f7="ellipsis" :class="{ 'disabled': loading || enabling || disabling }" @click="showMoreActionSheet = true"></f7-link>
            </f7-nav-right>
        </f7-navbar>

        <f7-list strong inset dividers class="margin-vertical skeleton-text" v-if="loading">
            <f7-list-item title="Status" after="Unknown"></f7-list-item>
        </f7-list>

        <f7-list strong inset dividers class="margin-vertical" v-else-if="!loading">
            <f7-list-item :title="tt('Status')" :after="tt(isEnableCloudSync ? 'Enabled' : 'Disabled')"></f7-list-item>
        </f7-list>

        <f7-list strong inset dividers class="margin-vertical synchronized-settings-list"
                 :class="{ 'disabled': loading || enabling || disabling }">
            <f7-list-item group-title :sortable="false">
                <small>{{ tt('Synchronized Settings') }}</small>
            </f7-list-item>
            <f7-list-item class="has-child-list-item" checkbox
                          :disabled="loading || enabling || disabling"
                          :value="categorizedItems.categoryName"
                          :checked="isAllSettingsSelected(categorizedItems)"
                          :indeterminate="hasSettingSelectedButNotAllChecked(categorizedItems)"
                          :key="categorizedItems.categoryName"
                          v-for="categorizedItems in ALL_APPLICATION_CLOUD_SETTINGS"
                          @change="updateSettingsSelected(categorizedItems, $event.target.checked)">
                <template #root>
                    <ul class="padding-inline-start">
                        <f7-list-item checkbox
                                      :disabled="loading || enabling || disabling"
                                      :title="tt(settingItem.settingName)"
                                      :value="settingItem.settingKey"
                                      :checked="enabledApplicationCloudSettings[settingItem.settingKey]"
                                      :key="settingItem.settingKey"
                                      v-for="settingItem in categorizedItems.items"
                                      @change="enabledApplicationCloudSettings[settingItem.settingKey] = $event.target.checked">
                            <template #after>
                                <f7-icon class="synchronized-settings-device-icon" f7="device_phone_portrait" v-if="settingItem.mobile"></f7-icon>
                                <f7-icon class="synchronized-settings-device-icon" f7="device_desktop" v-if="settingItem.desktop"></f7-icon>
                            </template>
                        </f7-list-item>
                    </ul>
                </template>

                <template #title>
                    <span>{{ tt(categorizedItems.categoryName) }}</span>
                    <span class="margin-horizontal-half" v-if="categorizedItems.categorySubName">/</span>
                    <span v-if="categorizedItems.categorySubName">{{ tt(categorizedItems.categorySubName) }}</span>
                </template>
            </f7-list-item>
        </f7-list>

        <f7-list strong inset dividers class="margin-vertical skeleton-text" v-if="loading">
            <f7-list-button class="disabled">Operate</f7-list-button>
        </f7-list>

        <f7-list strong inset dividers class="margin-vertical" v-else-if="!loading">
            <f7-list-button :class="{ 'disabled': loading || enabling || disabling || !hasEnabledApplicationCloudSettings }"
                            v-if="!isEnableCloudSync"
                            @click="enable(false)">{{ tt('Enable Settings Sync') }}</f7-list-button>
            <f7-list-button :class="{ 'disabled': loading || enabling || disabling || !hasEnabledApplicationCloudSettings }"
                            v-if="isEnableCloudSync"
                            @click="enable(true)">{{ tt('Update Synchronized Settings') }}</f7-list-button>
            <f7-list-button :class="{ 'disabled': loading || enabling || disabling }"
                            v-if="isEnableCloudSync"
                            @click="disable">{{ tt('Disable') }}</f7-list-button>
        </f7-list>

        <f7-actions close-by-outside-click close-on-escape :opened="showMoreActionSheet" @actions:closed="showMoreActionSheet = false">
            <f7-actions-group>
                <f7-actions-button :class="{ 'disabled': loading || enabling || disabling }"
                                   @click="selectAllSettings">{{ tt('Select All') }}</f7-actions-button>
                <f7-actions-button :class="{ 'disabled': loading || enabling || disabling }"
                                   @click="selectNoneSettings">{{ tt('Select None') }}</f7-actions-button>
                <f7-actions-button :class="{ 'disabled': loading || enabling || disabling }"
                                   @click="selectInvertSettings">{{ tt('Invert Selection') }}</f7-actions-button>
            </f7-actions-group>
            <f7-actions-group>
                <f7-actions-button bold close>{{ tt('Cancel') }}</f7-actions-button>
            </f7-actions-group>
        </f7-actions>
    </f7-page>
</template>

<script setup lang="ts">
import type { Router } from 'framework7/types';

import { useI18n } from '@/locales/helpers.ts';
import { useAppCloudSyncBase } from '@/views/base/settings/AppCloudSyncPageBase.ts';
import { useMobileAppCloudSyncActions } from './app-cloud-sync/useMobileAppCloudSyncActions.ts';

const props = defineProps<{
    f7router: Router.Router;
}>();

const { tt } = useI18n();
const {
    ALL_APPLICATION_CLOUD_SETTINGS,
    loading,
    enabling,
    disabling,
    enabledApplicationCloudSettings,
    isEnableCloudSync,
    hasEnabledApplicationCloudSettings,
    enabledApplicationCloudSettingKeys,
    isAllSettingsSelected,
    hasSettingSelectedButNotAllChecked,
    updateSettingsSelected,
    selectAllSettings,
    selectNoneSettings,
    selectInvertSettings,
    setUserApplicationCloudSettings
} = useAppCloudSyncBase();

const {
    showMoreActionSheet,
    init,
    enable,
    disable,
    onPageAfterIn
} = useMobileAppCloudSyncActions({
    loading,
    enabling,
    disabling,
    enabledApplicationCloudSettings,
    enabledApplicationCloudSettingKeys,
    f7router: props.f7router,
    setUserApplicationCloudSettings
});

init();
</script>

<style>
.synchronized-settings-device-icon {
    font-size: var(--ebk-synchronized-settings-list-device-icon-font-size);
}
</style>
