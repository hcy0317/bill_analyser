<template>
    <f7-list-item
        link="#" no-chevron
        :header="tt('Pictures')"
        v-if="show"
    >
        <template #footer>
            <f7-block class="margin-top-half no-padding no-margin" :class="{ 'readonly': submitting || uploadingPicture || recognizingPicture || removingPictureId }">
                <swiper-container
                    :pagination="false"
                    :space-between="10"
                    :slides-per-view="'auto'"
                    class="transaction-pictures"
                >
                    <swiper-slide class="transaction-picture-container" :key="picIdx"
                                  v-for="(pictureInfo, picIdx) in pictures"
                                  @click="emit('viewOrRemovePicture', pictureInfo)">
                        <div class="transaction-picture">
                            <div class="display-flex justify-content-center align-items-center transaction-picture-control-backdrop"
                                 v-if="mode === TransactionEditPageMode.Add || mode === TransactionEditPageMode.Edit">
                                <f7-icon class="picture-control-icon picture-remove-icon" f7="trash" v-if="pictureInfo.pictureId !== removingPictureId"></f7-icon>
                                <f7-preloader color="white" :size="28" v-if="pictureInfo.pictureId === removingPictureId" />
                            </div>
                            <img alt="picture" :src="getTransactionPictureUrl(pictureInfo) || ''"/>
                        </div>
                    </swiper-slide>
                    <swiper-slide @click="emit('openPictureDialog')" v-if="canAddTransactionPicture">
                        <div class="display-flex justify-content-center align-items-center transaction-picture transaction-picture-add">
                            <f7-icon class="picture-control-icon" f7="plus" v-if="!uploadingPicture && !recognizingPicture"></f7-icon>
                            <f7-preloader :size="28" v-if="uploadingPicture || recognizingPicture" />
                        </div>
                    </swiper-slide>
                </swiper-container>
            </f7-block>
        </template>
    </f7-list-item>
</template>

<script setup lang="ts">
import { TransactionEditPageMode } from '@/views/base/transactions/TransactionEditPageBase.ts';
import type { TransactionPictureInfoBasicResponse } from '@/models/transaction_picture_info.ts';

defineProps<{
    show: boolean;
    pictures: TransactionPictureInfoBasicResponse[];
    mode: TransactionEditPageMode;
    submitting: boolean;
    uploadingPicture: boolean;
    recognizingPicture: boolean;
    removingPictureId: string | null;
    canAddTransactionPicture: boolean;
    tt: (key: string) => string;
    getTransactionPictureUrl: (pictureInfo?: TransactionPictureInfoBasicResponse | null) => string | undefined;
}>();

const emit = defineEmits<{
    viewOrRemovePicture: [pictureInfo: TransactionPictureInfoBasicResponse];
    openPictureDialog: [];
}>();
</script>
