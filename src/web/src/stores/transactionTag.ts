import { ref, computed } from 'vue';
import { defineStore } from 'pinia';

import { type BeforeResolveFunction, itemAndIndex } from '@/core/base.ts';

import {
    type TransactionTagCreateBatchRequest,
    type TransactionTagInfoResponse,
    type TransactionTagNewDisplayOrderRequest,
    TransactionTag
} from '@/models/transaction_tag.ts';

import logger from '@/lib/logger.ts';
import services, { type ApiResponsePromise } from '@/lib/services.ts';

export const useTransactionTagsStore = defineStore('transactionTags', () => {
    const allTransactionTags = ref<TransactionTag[]>([]);
    const allTransactionTagsMap = ref<Record<string, TransactionTag>>({});
    const transactionTagListStateInvalid = ref<boolean>(true);

    const allVisibleTags = computed<TransactionTag[]>(() => {
        const visibleTags: TransactionTag[] = [];

        for (const tag of allTransactionTags.value) {
            if (!tag.hidden) {
                visibleTags.push(tag);
            }
        }

        return visibleTags;
    });

    const allAvailableTagsCount = computed<number>(() => allTransactionTags.value.length);
    const allVisibleTagsCount = computed<number>(() => allVisibleTags.value.length);

    function loadTransactionTagList(tags: TransactionTag[]): void {
        allTransactionTags.value = tags;
        allTransactionTagsMap.value = {};

        for (const tag of tags) {
            allTransactionTagsMap.value[tag.id] = tag;
        }
    }

    function addTagToTransactionTagList(tag: TransactionTag): void {
        allTransactionTags.value.push(tag);
        allTransactionTagsMap.value[tag.id] = tag;
    }

    function updateTagInTransactionTagList(currentTag: TransactionTag): void {
        for (const [transactionTag, index] of itemAndIndex(allTransactionTags.value)) {
            if (transactionTag.id === currentTag.id) {
                allTransactionTags.value.splice(index, 1, currentTag);
                break;
            }
        }

        allTransactionTagsMap.value[currentTag.id] = currentTag;
    }

    function updateTagDisplayOrderInTransactionTagList({ from, to }: { from: number, to: number }): void {
        allTransactionTags.value.splice(to, 0, allTransactionTags.value.splice(from, 1)[0] as TransactionTag);
    }

    function updateTagVisibilityInTransactionTagList({ tag, hidden }: { tag: TransactionTag, hidden: boolean }): void {
        if (allTransactionTagsMap.value[tag.id]) {
            allTransactionTagsMap.value[tag.id]!.hidden = hidden;
        }
    }

    function removeTagFromTransactionTagList(currentTag: TransactionTag): void {
        for (const [transactionTag, index] of itemAndIndex(allTransactionTags.value)) {
            if (transactionTag.id === currentTag.id) {
                allTransactionTags.value.splice(index, 1);
                break;
            }
        }

        if (allTransactionTagsMap.value[currentTag.id]) {
            delete allTransactionTagsMap.value[currentTag.id];
        }
    }

    function updateTransactionTagListInvalidState(invalidState: boolean): void {
        transactionTagListStateInvalid.value = invalidState;
    }

    function resetTransactionTags(): void {
        allTransactionTags.value = [];
        allTransactionTagsMap.value = {};
        transactionTagListStateInvalid.value = true;
    }

    function loadAllTags({ force }: { force?: boolean }): Promise<TransactionTag[]> {
        if (!force && !transactionTagListStateInvalid.value) {
            return new Promise((resolve) => {
                resolve(allTransactionTags.value);
            });
        }

        return new Promise((resolve, reject) => {
            services.getAllTransactionTags().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to retrieve tag list' });
                    return;
                }

                if (transactionTagListStateInvalid.value) {
                    updateTransactionTagListInvalidState(false);
                }

                const transactionTags = TransactionTag.ofMulti(data.result);

                // 即使数据相同，也应该resolve而不是reject，因为这不是错误
                // 修复：删除了导致Promise.all失败的reject逻辑
                // if (force && data.result && isEquals(allTransactionTags.value, transactionTags)) {
                //     reject({ message: 'Tag list is up to date', isUpToDate: true });
                //     return;
                // }

                loadTransactionTagList(transactionTags);

                resolve(transactionTags);
            }).catch(error => {
                if (force) {
                    logger.error('failed to force load tag list', error);
                } else {
                    logger.error('failed to load tag list', error);
                }

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve tag list' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function saveTag({ tag }: { tag: TransactionTag }): Promise<TransactionTag> {
        return new Promise((resolve, reject) => {
            let promise: ApiResponsePromise<TransactionTagInfoResponse>;

            if (!tag.id) {
                promise = services.addTransactionTag(tag.toCreateRequest());
            } else {
                promise = services.modifyTransactionTag(tag.toModifyRequest());
            }

            promise.then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    if (!tag.id) {
                        reject({ message: 'Unable to add tag' });
                    } else {
                        reject({ message: 'Unable to save tag' });
                    }
                    return;
                }

                const transactionTag = TransactionTag.of(data.result);

                if (!tag.id) {
                    addTagToTransactionTagList(transactionTag);
                } else {
                    updateTagInTransactionTagList(transactionTag);
                }

                resolve(transactionTag);
            }).catch(error => {
                logger.error('failed to save tag', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    if (!tag.id) {
                        reject({ message: 'Unable to add tag' });
                    } else {
                        reject({ message: 'Unable to save tag' });
                    }
                } else {
                    reject(error);
                }
            });
        });
    }

    function addTags(req: TransactionTagCreateBatchRequest): Promise<TransactionTag[]> {
        return new Promise((resolve, reject) => {
            services.addTransactionTagBatch(req).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to add tag' });
                    return;
                }

                if (!transactionTagListStateInvalid.value) {
                    updateTransactionTagListInvalidState(true);
                }

                const transactionTags = TransactionTag.ofMulti(data.result);

                resolve(transactionTags);
            }).catch(error => {
                logger.error('failed to add tags', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to add tag' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function changeTagDisplayOrder({ tagId, from, to }: { tagId: string, from: number, to: number }): Promise<void> {
        return new Promise((resolve, reject) => {
            let currentTag: TransactionTag | null = null;

            for (const transactionTag of allTransactionTags.value) {
                if (transactionTag.id === tagId) {
                    currentTag = transactionTag;
                    break;
                }
            }

            if (!currentTag || !allTransactionTags.value[to]) {
                reject({ message: 'Unable to move tag' });
                return;
            }

            if (!transactionTagListStateInvalid.value) {
                updateTransactionTagListInvalidState(true);
            }

            updateTagDisplayOrderInTransactionTagList({ from, to });

            resolve();
        });
    }

    function updateTagDisplayOrders(): Promise<boolean> {
        const newDisplayOrders: TransactionTagNewDisplayOrderRequest[] = [];

        for (const [transactionTag, index] of itemAndIndex(allTransactionTags.value)) {
            newDisplayOrders.push({
                id: transactionTag.id,
                displayOrder: index + 1
            });
        }

        return new Promise((resolve, reject) => {
            services.moveTransactionTag({
                newDisplayOrders: newDisplayOrders
            }).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to move tag' });
                    return;
                }

                if (transactionTagListStateInvalid.value) {
                    updateTransactionTagListInvalidState(false);
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to save tags display order', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to move tag' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function hideTag({ tag, hidden }: { tag: TransactionTag, hidden: boolean }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            console.log(`[TransactionTagsStore] hideTag called for ${tag.id}, hidden=${hidden}`);

            services.hideTransactionTag({ id: tag.id, hidden: hidden }).then(response => {
                const data = response.data;

                if (!data || !data.success) {
                    console.error(`[TransactionTagsStore] hideTag failed for ${tag.id}`, data);
                    reject({ message: 'Unable to update tag visibility' });
                    return;
                }

                console.log(`[TransactionTagsStore] hideTag success for ${tag.id}`);
                updateTagVisibilityInTransactionTagList({ tag, hidden });

                resolve(true);
            }).catch(error => {
                console.error(`[TransactionTagsStore] hideTag error for ${tag.id}`, error);
                logger.error('failed to update tag visibility', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to update tag visibility' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function deleteTag({ tag, beforeResolve }: { tag: TransactionTag, beforeResolve?: BeforeResolveFunction }): Promise<boolean> {
        return new Promise((resolve, reject) => {
            console.log(`[TransactionTagsStore] deleteTag called for ${tag.id}`);
            services.deleteTransactionTag({ id: tag.id }).then(response => {
                const data = response.data;

                if (!data || !data.success) {
                    console.error(`[TransactionTagsStore] deleteTag failed for ${tag.id}`, data);
                    reject({ message: 'Unable to delete this tag' });
                    return;
                }

                console.log(`[TransactionTagsStore] deleteTag success for ${tag.id}`);

                if (beforeResolve) {
                    beforeResolve(() => {
                        removeTagFromTransactionTagList(tag);
                        resolve(true);
                    });
                } else {
                    removeTagFromTransactionTagList(tag);
                    resolve(true);
                }
            }).catch(error => {
                console.error(`[TransactionTagsStore] deleteTag error for ${tag.id}`, error);
                logger.error('failed to delete tag', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to delete this tag' });
                } else {
                    reject(error);
                }
            });
        });
    }

    return {
        // 状态
        allTransactionTags,
        allTransactionTagsMap,
        transactionTagListStateInvalid,
        // 计算状态
        allVisibleTags,
        allAvailableTagsCount,
        allVisibleTagsCount,
        // 函数
        updateTransactionTagListInvalidState,
        resetTransactionTags,
        loadAllTags,
        saveTag,
        addTags,
        changeTagDisplayOrder,
        updateTagDisplayOrders,
        hideTag,
        deleteTag
    }
});
