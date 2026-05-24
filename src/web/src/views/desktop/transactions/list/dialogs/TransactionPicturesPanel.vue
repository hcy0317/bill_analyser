<template>
    <v-row class="transaction-pictures align-content-start" :class="{ 'readonly': isReadonly }">
        <v-col :key="picIdx" cols="6" md="3" v-for="(pictureInfo, picIdx) in visiblePictures">
            <v-avatar rounded="lg" variant="tonal" size="160"
                      class="cursor-pointer transaction-picture"
                      color="rgba(0,0,0,0)" @click="emit('view-or-remove', pictureInfo)">
                <v-img :src="getPictureUrl(pictureInfo)">
                    <template #placeholder>
                        <div class="d-flex align-center justify-center fill-height bg-light-primary">
                            <v-progress-circular color="grey-500" indeterminate size="48"></v-progress-circular>
                        </div>
                    </template>
                    <template #error>
                        <div class="d-flex align-center justify-center fill-height bg-light-primary">
                            <span class="text-body-1">{{ tt('Failed to load image, please check whether the config "domain" and "root_url" are set correctly.') }}</span>
                        </div>
                    </template>
                </v-img>
                <div class="picture-control-icon" :class="{ 'show-control-icon': pictureInfo.pictureId === removingPictureId }">
                    <v-icon size="64" :icon="mdiTrashCanOutline" v-if="editingPictures && pictureInfo.pictureId !== removingPictureId"/>
                    <v-progress-circular color="grey-500" indeterminate size="48" v-if="editingPictures && pictureInfo.pictureId === removingPictureId"></v-progress-circular>
                    <v-icon size="64" :icon="mdiFullscreen" v-if="!editingPictures"/>
                </div>
            </v-avatar>
        </v-col>
        <v-col cols="6" md="3" v-if="canAddPicture">
            <v-avatar rounded="lg" variant="tonal" size="160"
                      class="transaction-picture transaction-picture-add"
                      :class="{ 'enabled': !submitting, 'cursor-pointer': !submitting }"
                      color="rgba(0,0,0,0)" @click="showOpenPictureDialog">
                <v-tooltip activator="parent" v-if="!submitting">{{ tt('Upload Receipt Image') }}</v-tooltip>
                <v-icon class="transaction-picture-add-icon" size="56" :icon="mdiImagePlusOutline" v-if="!uploadingPicture && !recognizingPicture"/>
                <v-progress-circular color="grey-500" indeterminate size="48" v-if="uploadingPicture || recognizingPicture"></v-progress-circular>
            </v-avatar>
        </v-col>
    </v-row>

    <input ref="pictureInput" type="file" style="display: none" :accept="SUPPORTED_IMAGE_EXTENSIONS" @change="emit('upload', $event)" />
</template>

<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';

import { useI18n } from '@/locales/helpers.ts';
import { SUPPORTED_IMAGE_EXTENSIONS } from '@/consts/file.ts';
import { TransactionEditPageMode } from '@/views/base/transactions/TransactionEditPageBase.ts';

import type { TransactionPictureInfoBasicResponse } from '@/models/transaction_picture_info.ts';

import {
    mdiFullscreen,
    mdiImagePlusOutline,
    mdiTrashCanOutline
} from '@mdi/js';

const props = defineProps<{
    pictures?: TransactionPictureInfoBasicResponse[];
    mode: TransactionEditPageMode;
    submitting: boolean;
    uploadingPicture: boolean;
    recognizingPicture: boolean;
    removingPictureId: string;
    canAddPicture: boolean;
    getPictureUrl: (pictureInfo?: TransactionPictureInfoBasicResponse | null) => string | undefined;
}>();

const emit = defineEmits<{
    upload: [event: Event];
    'view-or-remove': [pictureInfo: TransactionPictureInfoBasicResponse];
}>();

const { tt } = useI18n();
const pictureInput = useTemplateRef<HTMLInputElement>('pictureInput');

const visiblePictures = computed<TransactionPictureInfoBasicResponse[]>(() => props.pictures ?? []);
const editingPictures = computed<boolean>(() => props.mode === TransactionEditPageMode.Add || props.mode === TransactionEditPageMode.Edit);
const isReadonly = computed<boolean>(() => props.submitting || props.uploadingPicture || props.recognizingPicture || !!props.removingPictureId);

function showOpenPictureDialog(): void {
    if (!props.canAddPicture || props.submitting || props.recognizingPicture) {
        return;
    }

    pictureInput.value?.click();
}
</script>

<style>
@media (min-height: 630px) {
    @media (min-width: 960px) {
        .transaction-pictures {
            min-height: 300px;
        }
    }
}

@media (min-height: 700px) {
    @media (min-width: 960px) {
        .transaction-pictures {
            min-height: 350px;
        }
    }
}

@media (min-height: 800px) {
    @media (min-width: 960px) {
        .transaction-pictures {
            min-height: 450px;
        }
    }
}

@media (min-height: 900px) {
    @media (min-width: 960px) {
        .transaction-pictures {
            min-height: 550px;
        }
    }
}

.transaction-picture .picture-control-icon {
    display: none;
    position: absolute;
    width: 100% !important;
    height: 100% !important;
    background-color: rgba(0, 0, 0, 0.4);
}

.transaction-picture .picture-control-icon > i.v-icon {
    background-color: transparent;
    color: rgba(255, 255, 255, 0.8);
}

.transaction-picture:hover .picture-control-icon,
.transaction-picture .picture-control-icon.show-control-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    vertical-align: middle;
}

.transaction-picture:hover .transaction-picture-placeholder {
    display: none;
}

.transaction-picture-add {
    border: 2px dashed rgba(var(--v-theme-grey-500));

    .transaction-picture-add-icon {
        color: rgba(var(--v-theme-grey-500));
    }
}

.transaction-picture-add.enabled:hover {
    border: 2px dashed rgba(var(--v-theme-grey-700));

    .transaction-picture-add-icon {
        color: rgba(var(--v-theme-grey-700));
    }
}
</style>
