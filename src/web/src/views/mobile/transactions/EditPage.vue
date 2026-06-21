<template src="./edit-page/EditPage.template.html"></template>

<script setup lang="ts">import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import { ref, computed, useTemplateRef, watch } from 'vue';
import type { PhotoBrowser, Router } from 'framework7/types';

import { useI18n } from '@/locales/helpers.ts';
import { useI18nUIComponents, showLoading, hideLoading } from '@/lib/ui/mobile.ts';
import {
    TransactionEditPageMode,
    TransactionEditPageType,
    GeoLocationStatus,
    useTransactionEditPageBase
} from '@/views/base/transactions/TransactionEditPageBase.ts';
import MobileTransactionPicturesPanel from './components/MobileTransactionPicturesPanel.vue';
import { useSettingsStore } from '@/stores/setting.ts';
import { useEnvironmentsStore } from '@/stores/environment.ts';
import { useUserStore } from '@/stores/user.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useTransactionsStore } from '@/stores/transaction.ts';
import { useTransactionTemplatesStore } from '@/stores/transactionTemplate.ts';

import { CategoryType } from '@/core/category.ts';
import { TransactionEditScopeType, TransactionType } from '@/core/transaction.ts';
import { ScheduledTemplateFrequencyType, TemplateType } from '@/core/template.ts';
import { TRANSACTION_MAX_AMOUNT, TRANSACTION_MIN_AMOUNT } from '@/consts/transaction.ts';
import { KnownErrorCode } from '@/consts/api.ts';
import { SUPPORTED_IMAGE_EXTENSIONS } from '@/consts/file.ts';

import { TransactionTemplate } from '@/models/transaction_template.ts';
import type { TransactionPictureInfoBasicResponse } from '@/models/transaction_picture_info.ts';
import { Transaction } from '@/models/transaction.ts';
import type { RecognizedReceiptImageResponse, RecognizeReceiptImageError } from '@/models/large_language_model.ts';
import { RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD } from '@/models/large_language_model.ts';

import {
    getActualUnixTimeForStore,
    getBrowserTimezoneOffsetMinutes,
    getTimezoneOffset,
    getTimezoneOffsetMinutes
} from '@/lib/datetime.ts';
import { categorizedArrayToPlainArray } from '@/lib/common.ts';
import { formatCoordinate } from '@/lib/coordinate.ts';
import { generateRandomUUID } from '@/lib/misc.ts';
import {
    getTransactionPrimaryCategoryName,
    getTransactionSecondaryCategoryName,
    localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories
} from '@/lib/category.ts';
import { setTransactionModelByTransaction } from '@/lib/transaction.ts';
import {
    type ReceiptDraftCandidateHint,
    applyReceiptDraftAutoFillToTransaction,
    applyReceiptDraftFieldToTransaction,
    buildReceiptDraftCandidateHints,
    getReceiptDraftCandidateDisplayValue
} from '@/lib/receiptDraft.ts';
import {
    buildTransactionPictureItems,
    buildTransactionThumbs,
    getFontClassByAmount,
    parseStrictQueryCents
} from './edit-page/displayHelpers.ts';
import { getMapProvider, isTransactionPicturesEnabled } from '@/lib/server_settings.ts';
import logger from '@/lib/logger.ts';

const props = defineProps<{
    f7route: Router.Route;
    f7router: Router.Router;
}>();

const query = props.f7route.query;
const pageTypeAndMode = getPageTypeNameMode();

const {
    tt,
    getCurrentLanguageTag,
    getAllTransactionDefaultCategories,
    getMultiMonthdayShortNames,
    getMultiWeekdayLongNames,
    formatUnixTimeToLongDate,
    formatUnixTimeToLongTime,
    formatGregorianTextualYearMonthDayToLongDate
} = useI18n();
const { showAlert, showConfirm, showToast, routeBackOnError } = useI18nUIComponents();

const {
    mode,
    isSupportGeoLocation,
    editId,
    addByTemplateId,
    duplicateFromId,
    clientSessionId,
    loading,
    submitting,
    uploadingPicture,
    geoLocationStatus,
    setGeoLocationByClickMap,
    transaction,
    numeralSystem,
    currentTimezoneOffsetMinutes,
    defaultCurrency,
    defaultAccountId,
    firstDayOfWeek,
    coordinateDisplayType,
    allTimezones,
    allVisibleAccounts,
    allAccountsMap,
    allVisibleCategorizedAccounts,
    allCategories,
    allCategoriesMap,
    allTags,
    allTagsMap,
    firstVisibleAccountId,
    hasAvailableExpenseCategories,
    hasAvailableIncomeCategories,
    hasAvailableTransferCategories,
    hasAvailableInvestmentCategories,
    canAddTransactionPicture,
    title,
    saveButtonTitle,
    sourceAmountTitle,
    sourceAccountTitle,
    transferInAmountTitle,
    sourceAccountName,
    destinationAccountName,
    sourceAccountCurrency,
    destinationAccountCurrency,
    transactionDisplayTimezone,
    transactionTimezoneTimeDifference,
    geoLocationStatusInfo,
    inputEmptyProblemMessage,
    inputIsEmpty,
    swapTransactionData,
    getDisplayAmount,
    getTransactionPictureUrl
} = useTransactionEditPageBase(pageTypeAndMode?.type || TransactionEditPageType.Transaction, pageTypeAndMode?.mode, query['type'] ? parseInt(query['type']) : undefined);

const settingsStore = useSettingsStore();
const environmentsStore = useEnvironmentsStore();
const userStore = useUserStore();
const accountsStore = useAccountsStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const transactionTagsStore = useTransactionTagsStore();
const transactionsStore = useTransactionsStore();
const transactionTemplatesStore = useTransactionTemplatesStore();

const pictureBrowser = useTemplateRef<PhotoBrowser.PhotoBrowser>('pictureBrowser');
const pictureInput = useTemplateRef<HTMLInputElement>('pictureInput');

const loadingError = ref<unknown | null>(null);
const submitted = ref<boolean>(false);
const removingPictureId = ref<string | null>(null);
const transactionDateTimeSheetMode = ref<string>('time');
const showTimeInDefaultTimezone = ref<boolean>(false);
const showTimezonePopup = ref<boolean>(false);
const showGeoLocationActionSheet = ref<boolean>(false);
const showMoreActionSheet = ref<boolean>(false);
const showSourceAmountSheet = ref<boolean>(false);
const showDestinationAmountSheet = ref<boolean>(false);
const showCategorySheet = ref<boolean>(false);
const showSourceAccountSheet = ref<boolean>(false);
const showDestinationAccountSheet = ref<boolean>(false);
const showTransactionDateTimeSheet = ref<boolean>(false);
const showTransactionScheduledFrequencySheet = ref<boolean>(false);
const showScheduledStartDateSheet = ref<boolean>(false);
const showScheduledEndDateSheet = ref<boolean>(false);
const showGeoLocationMapSheet = ref<boolean>(false);
const showTransactionTagSheet = ref<boolean>(false);
const addingDefaultCategories = ref<boolean>(false);
const recognizingPicture = ref<boolean>(false);
const receiptDraftCandidateHints = ref<ReceiptDraftCandidateHint[]>([]);
const showTransactionPictures = ref<boolean>(pageTypeAndMode?.type === TransactionEditPageType.Transaction
    && (pageTypeAndMode?.mode === TransactionEditPageMode.Add || pageTypeAndMode?.mode === TransactionEditPageMode.Edit)
    && isTransactionPicturesEnabled());

const isDarkMode = computed<boolean>(() => environmentsStore.framework7DarkMode || false);

const sourceAmountClass = computed<Record<string, boolean>>(() => {
    const classes: Record<string, boolean> = {
        'readonly': mode.value === TransactionEditPageMode.View,
        'text-expense': transaction.value.type === TransactionType.Expense,
        'text-income': transaction.value.type === TransactionType.Income,
        'text-color-primary': transaction.value.type === TransactionType.Transfer
    };

    classes[getFontClassByAmount(transaction.value.sourceAmountCents)] = true;

    return classes;
});

const destinationAmountClass = computed<Record<string, boolean>>(() => {
    const classes: Record<string, boolean> = {
        'readonly': mode.value === TransactionEditPageMode.View
    };

    classes[getFontClassByAmount(transaction.value.destinationAmountCents)] = true;

    return classes;
});

const transactionDisplayDate = computed<string>(() => {
    if (mode.value !== TransactionEditPageMode.View || !showTimeInDefaultTimezone.value) {
        return formatUnixTimeToLongDate(getActualUnixTimeForStore(transaction.value.time, getTimezoneOffsetMinutes(), getBrowserTimezoneOffsetMinutes()));
    }

    return formatUnixTimeToLongDate(getActualUnixTimeForStore(transaction.value.time, transaction.value.utcOffset, getBrowserTimezoneOffsetMinutes()));
});

const transactionDisplayTime = computed<string>(() => {
    if (mode.value !== TransactionEditPageMode.View || !showTimeInDefaultTimezone.value) {
        return formatUnixTimeToLongTime(getActualUnixTimeForStore(transaction.value.time, getTimezoneOffsetMinutes(), getBrowserTimezoneOffsetMinutes()));
    }

    const utcOffset = numeralSystem.value.replaceWesternArabicDigitsToLocalizedDigits(getTimezoneOffset(settingsStore.appSettings.timeZone));
    return `${formatUnixTimeToLongTime(getActualUnixTimeForStore(transaction.value.time, transaction.value.utcOffset, getBrowserTimezoneOffsetMinutes()))} (UTC${utcOffset})`;
});

const transactionDisplayTimezoneName = computed<string>(() => {
    for (const timezone of allTimezones.value) {
        if (timezone.name === transaction.value.timeZone) {
            return timezone.displayName;
        }
    }

    return '';
});

const transactionPictures = computed<Record<string, string | undefined>[]>(() => buildTransactionPictureItems(transaction.value.pictures, getTransactionPictureUrl));

const transactionThumbs = computed<(string | undefined)[]>(() => buildTransactionThumbs(transaction.value.pictures, getTransactionPictureUrl));

const transactionDisplayScheduledFrequency = computed<string>(() => {
    if (pageTypeAndMode?.type !== TransactionEditPageType.Template) {
        return '';
    }

    const template = transaction.value as TransactionTemplate;

    if (template.scheduledFrequencyType === ScheduledTemplateFrequencyType.Disabled.type) {
        return tt('Disabled');
    }

    const items = (template.scheduledFrequency || '').split(',');
    const scheduledFrequencyValues: number[] = [];

    for (const item of items) {
        if (item) {
            scheduledFrequencyValues.push(parseInt(item));
        }
    }

    if (template.scheduledFrequencyType === ScheduledTemplateFrequencyType.Weekly.type) {
        if (scheduledFrequencyValues.length) {
            return tt('format.misc.everyMultiDaysOfWeek', {
                days: getMultiWeekdayLongNames(scheduledFrequencyValues, firstDayOfWeek.value)
            });
        } else {
            return tt('Weekly');
        }
    } else if (template.scheduledFrequencyType === ScheduledTemplateFrequencyType.Monthly.type) {
        if (scheduledFrequencyValues.length) {
            return tt('format.misc.everyMultiDaysOfMonth', {
                days: getMultiMonthdayShortNames(scheduledFrequencyValues)
            });
        } else {
            return tt('Monthly');
        }
    } else {
        return '';
    }
});

const transactionDisplayScheduledStartDate = computed<string>(() => {
    if (pageTypeAndMode?.type !== TransactionEditPageType.Template) {
        return '';
    }

    const template = transaction.value as TransactionTemplate;

    if (template.scheduledStartDate) {
        return formatGregorianTextualYearMonthDayToLongDate(template.scheduledStartDate);
    } else {
        return tt('No limit');
    }
});

const transactionDisplayScheduledEndDate = computed<string>(() => {
    if (pageTypeAndMode?.type !== TransactionEditPageType.Template) {
        return '';
    }

    const template = transaction.value as TransactionTemplate;

    if (template.scheduledEndDate) {
        return formatGregorianTextualYearMonthDayToLongDate(template.scheduledEndDate);
    } else {
        return tt('No limit');
    }
});

function getPageTypeNameMode(): { type: TransactionEditPageType, mode: TransactionEditPageMode } | null {
    if (props.f7route.path === '/transaction/add') {
        return {
            type: TransactionEditPageType.Transaction,
            mode: TransactionEditPageMode.Add
        };
    } else if (props.f7route.path === '/transaction/edit') {
        return {
            type: TransactionEditPageType.Transaction,
            mode: TransactionEditPageMode.Edit
        };
    } else if (props.f7route.path === '/transaction/detail') {
        return {
            type: TransactionEditPageType.Transaction,
            mode: TransactionEditPageMode.View
        };
    } else if (props.f7route.path === '/template/add') {
        return {
            type: TransactionEditPageType.Template,
            mode: TransactionEditPageMode.Add
        };
    } else if (props.f7route.path === '/template/edit') {
        return {
            type: TransactionEditPageType.Template,
            mode: TransactionEditPageMode.Edit
        };
    } else {
        return null;
    }
}

function getTagName(tagId: string): string {
    for (const tag of allTags.value) {
        if (tag.id === tagId) {
            return tag.name;
        }
    }

    return '';
}

function hasAvailableCategoriesForType(type: number): boolean {
    if (type === TransactionType.Expense) {
        return hasAvailableExpenseCategories.value;
    }

    if (type === TransactionType.Income) {
        return hasAvailableIncomeCategories.value;
    }

    if (type === TransactionType.Transfer) {
        return hasAvailableTransferCategories.value;
    }

    if (type === TransactionType.Investment) {
        return hasAvailableInvestmentCategories.value;
    }

    return false;
}

async function addDefaultCategoriesAndOpenSheet(type: number): Promise<void> {
    if (addingDefaultCategories.value) {
        return;
    }

    const allPresetCategories = getAllTransactionDefaultCategories(0, getCurrentLanguageTag());
    const presetCategoriesArray = categorizedArrayToPlainArray(allPresetCategories);
    const submitCategories = localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories(presetCategoriesArray);

    if (!submitCategories.length) {
        showToast('No available category');
        return;
    }

    addingDefaultCategories.value = true;
    showLoading(() => addingDefaultCategories.value);

    try {
        await transactionCategoriesStore.addPresetCategories({
            categories: submitCategories
        });

        await transactionCategoriesStore.loadAllCategories({ force: true });
        showToast('You have added preset categories');

        if (hasAvailableCategoriesForType(type)) {
            showCategorySheet.value = true;
        }
    } catch (error: any) {
        if (!error?.processed) {
            showToast(error?.message || error);
        }
    } finally {
        addingDefaultCategories.value = false;
        hideLoading();
    }
}

function handleCategoryItemClick(): void {
    if (mode.value === TransactionEditPageMode.View || loading.value || submitting.value || addingDefaultCategories.value) {
        return;
    }

    const currentType = transaction.value.type;

    if (hasAvailableCategoriesForType(currentType)) {
        showCategorySheet.value = true;
        return;
    }

    showConfirm(`${tt('No available category')}. ${tt('Add Default Categories')}?`, () => {
        void addDefaultCategoriesAndOpenSheet(currentType);
    });
}

function init(): void {
    if (!pageTypeAndMode) {
        showToast('Parameter Invalid');
        loadingError.value = 'Parameter Invalid';
        return;
    }

    loading.value = true;
    receiptDraftCandidateHints.value = [];

    const promises: Promise<unknown>[] = [
        accountsStore.loadAllAccounts({ force: false }),
        transactionCategoriesStore.loadAllCategories({ force: false }),
        transactionTagsStore.loadAllTags({ force: false }),
        transactionTemplatesStore.loadAllTemplates({ force: false, templateType: TemplateType.Normal.type })
    ];

    if (pageTypeAndMode.type === TransactionEditPageType.Transaction) {
        if (query['id']) {
            if (mode.value === TransactionEditPageMode.Edit) {
                editId.value = query['id'];
            } else if (mode.value === TransactionEditPageMode.Add) {
                duplicateFromId.value = query['id'];
            }

            promises.push(transactionsStore.getTransaction({ transactionId: query['id'], withPictures: mode.value !== TransactionEditPageMode.Add }));
        }
    } else if (pageTypeAndMode.type === TransactionEditPageType.Template) {
        const template = TransactionTemplate.createNewTransactionTemplate(transaction.value);
        template.name = '';

        if (query['templateType']) {
            template.templateType = parseInt(query['templateType']);
        }

        if (template.templateType === TemplateType.Schedule.type) {
            template.scheduledFrequencyType = ScheduledTemplateFrequencyType.Disabled.type;
            template.scheduledFrequency = '';
        }

        transaction.value = template;

        if (query['id']) {
            if (mode.value === TransactionEditPageMode.Edit) {
                editId.value = query['id'];
            }

            promises.push(transactionTemplatesStore.getTemplate({
                templateId: query['id'],
                templateType: (transaction.value as TransactionTemplate).templateType
            }));
        }
    }

    const queryType = query['type'] ? parseInt(query['type']) : 0;

    if (queryType &&
        queryType >= TransactionType.Income &&
        queryType <= TransactionType.Investment) {
        transaction.value.type = queryType;
    } else if (queryType === TransactionType.ModifyBalance &&
        pageTypeAndMode.type === TransactionEditPageType.Transaction &&
        mode.value === TransactionEditPageMode.View) {
        transaction.value.type = queryType;
    }

    if (mode.value === TransactionEditPageMode.Add) {
        clientSessionId.value = generateRandomUUID();
    }

    Promise.all(promises).then(function (responses) {
        if (query['id'] && !responses[4]) {
            if (pageTypeAndMode.type === TransactionEditPageType.Transaction) {
                showToast('Unable to retrieve transaction');
                loadingError.value = 'Unable to retrieve transaction';
            } else if (pageTypeAndMode.type === TransactionEditPageType.Template) {
                showToast('Unable to retrieve template');
                loadingError.value = 'Unable to retrieve template';
            }

            return;
        }

        let fromTransaction: Transaction | TransactionTemplate | null = null;

        if (pageTypeAndMode.type === TransactionEditPageType.Transaction) {
            if (query['id'] && responses[4] instanceof Transaction) {
                fromTransaction = responses[4];
            } else if (query['templateId'] && transactionTemplatesStore.allTransactionTemplatesMap && transactionTemplatesStore.allTransactionTemplatesMap[TemplateType.Normal.type]) {
                fromTransaction = (transactionTemplatesStore.allTransactionTemplatesMap[TemplateType.Normal.type] as Record<string, TransactionTemplate>)[query['templateId']] ?? null;

                if (fromTransaction) {
                    addByTemplateId.value = fromTransaction.id;
                }
            } else if (query['noTransactionDraft'] !== 'true' && (settingsStore.appSettings.autoSaveTransactionDraft === 'enabled' || settingsStore.appSettings.autoSaveTransactionDraft === 'confirmation') && transactionsStore.transactionDraft) {
                fromTransaction = Transaction.ofDraft(transactionsStore.transactionDraft);
            }
        } else if (pageTypeAndMode.type === TransactionEditPageType.Template && responses[4] instanceof TransactionTemplate) {
            if (query['id']) {
                fromTransaction = responses[4];
            }
        }

        setTransactionModelByTransaction(
            transaction.value,
            fromTransaction,
            allCategories.value,
            allCategoriesMap.value,
            allVisibleAccounts.value,
            allAccountsMap.value,
            allTagsMap.value,
            defaultAccountId.value,
            {
                time: query['time'] ? parseInt(query['time']) : undefined,
                type: queryType,
                categoryId: query['categoryId'],
                accountId: query['accountId'],
                destinationAccountId: query['destinationAccountId'],
                sourceAmountCents: parseStrictQueryCents(query['sourceAmountCents']),
                destinationAmountCents: parseStrictQueryCents(query['destinationAmountCents']),
                tagIds: query['tagIds'],
                comment: query['comment']
            },
            pageTypeAndMode.type === TransactionEditPageType.Transaction && (mode.value === TransactionEditPageMode.Edit || mode.value === TransactionEditPageMode.View),
            pageTypeAndMode.type === TransactionEditPageType.Transaction && (mode.value === TransactionEditPageMode.Edit || mode.value === TransactionEditPageMode.View)
        );

        if (pageTypeAndMode.type === TransactionEditPageType.Transaction && query['id'] && responses[4] instanceof Transaction) {
            if (fromTransaction && query['withTime'] && query['withTime'] === 'true') {
                transaction.value.time = fromTransaction.time;
                transaction.value.timeZone = fromTransaction.timeZone;
                transaction.value.utcOffset = fromTransaction.utcOffset;
            }

            if (fromTransaction && query['withGeoLocation'] && query['withGeoLocation'] === 'true') {
                transaction.value.setGeoLocation(fromTransaction.geoLocation);
            }
        } else if (pageTypeAndMode.type === TransactionEditPageType.Template && query['id'] && responses[4] instanceof TransactionTemplate) {
            const template = responses[4];
            transaction.value.id = template.id;

            if (!(transaction.value instanceof TransactionTemplate)) {
                transaction.value = TransactionTemplate.createNewTransactionTemplate(transaction.value);
            }

            (transaction.value as TransactionTemplate).fillFrom(template);
        }

        loading.value = false;
    }).catch(error => {
        logger.error('failed to load essential data for editing transaction', error);

        if (error.processed) {
            loading.value = false;
        } else {
            loadingError.value = error;
            showToast(error.message || error);
        }
    });
}

function save(): void {
    const router = props.f7router;

    if (mode.value === TransactionEditPageMode.View) {
        return;
    }

    const problemMessage = inputEmptyProblemMessage.value;

    if (problemMessage) {
        showAlert(problemMessage);
        return;
    }

    if (pageTypeAndMode?.type === TransactionEditPageType.Transaction && (mode.value === TransactionEditPageMode.Add || mode.value === TransactionEditPageMode.Edit)) {
        const doSubmit = function () {
            submitting.value = true;
            showLoading(() => submitting.value);

            transactionsStore.saveTransaction({
                transaction: transaction.value as Transaction,
                defaultCurrency: defaultCurrency.value,
                isEdit: mode.value === TransactionEditPageMode.Edit,
                clientSessionId: clientSessionId.value
            }).then(() => {
                submitting.value = false;
                hideLoading();

                if (mode.value === TransactionEditPageMode.Add) {
                    showToast('You have added a new transaction');
                } else if (mode.value === TransactionEditPageMode.Edit) {
                    showToast('You have saved this transaction');
                }

                if (mode.value === TransactionEditPageMode.Add && query['noTransactionDraft'] !== 'true' && !addByTemplateId.value && !duplicateFromId.value) {
                    transactionsStore.clearTransactionDraft();
                }

                submitted.value = true;
                router.back();
            }).catch(error => {
                submitting.value = false;
                hideLoading();

                if (error.error && (error.error.errorCode === KnownErrorCode.TransactionCannotCreateInThisTime || error.error.errorCode === KnownErrorCode.TransactionCannotModifyInThisTime)) {
                    showConfirm('You have set this time range to prevent editing transactions. Would you like to change the editable transaction range to All?', () => {
                        submitting.value = true;
                        showLoading(() => submitting.value);

                        userStore.updateUserTransactionEditScope({
                            transactionEditScope: TransactionEditScopeType.All.type
                        }).then(() => {
                            submitting.value = false;
                            hideLoading();

                            showToast('Your editable transaction range has been set to All');
                        }).catch(error => {
                            submitting.value = false;
                            hideLoading();

                            if (!error.processed) {
                                showToast(error.message || error);
                            }
                        });
                    });
                } else if (!error.processed) {
                    showToast(error.message || error);
                }
            });
        };

        if (transaction.value.sourceAmountCents === 0) {
            showConfirm('Are you sure you want to save this transaction with a zero amount?', () => {
                doSubmit();
            });
        } else {
            doSubmit();
        }
    } else if (pageTypeAndMode?.type === TransactionEditPageType.Template && (mode.value === TransactionEditPageMode.Add || mode.value === TransactionEditPageMode.Edit)) {
        submitting.value = true;
        showLoading(() => submitting.value);

        transactionTemplatesStore.saveTemplateContent({
            template: transaction.value as TransactionTemplate,
            isEdit: mode.value === TransactionEditPageMode.Edit,
            clientSessionId: clientSessionId.value
        }).then(() => {
            submitting.value = false;
            hideLoading();

            if (mode.value === TransactionEditPageMode.Add) {
                showToast('You have added a new template');
            } else if (mode.value === TransactionEditPageMode.Edit) {
                showToast('You have saved this template');
            }

            submitted.value = true;
            router.back();
        }).catch(error => {
            submitting.value = false;
            hideLoading();

            if (!error.processed) {
                showToast(error.message || error);
            }
        });
    }
}

function updateGeoLocation(forceUpdate: boolean): void {
    if (!isSupportGeoLocation) {
        logger.warn('this browser does not support geo location');

        if (forceUpdate) {
            showToast('Unable to retrieve current position');
        }
        return;
    }

    navigator.geolocation.getCurrentPosition(function (position) {
        if (!position || !position.coords) {
            logger.error('current position is null');
            geoLocationStatus.value = GeoLocationStatus.Error;

            if (forceUpdate) {
                showToast('Unable to retrieve current position');
            }

            return;
        }

        geoLocationStatus.value = GeoLocationStatus.Success;

        transaction.value.setLatitudeAndLongitude(position.coords.latitude, position.coords.longitude);
    }, function (err) {
        logger.error('cannot retrieve current position', err);
        geoLocationStatus.value = GeoLocationStatus.Error;

        if (forceUpdate) {
            showToast('Unable to retrieve current position');
        }
    });

    geoLocationStatus.value = GeoLocationStatus.Getting;
}

function clearGeoLocation(): void {
    geoLocationStatus.value = null;
    transaction.value.removeGeoLocation();
}

function showDateTimeDialog(sheetMode: string): void {
    if (mode.value === TransactionEditPageMode.View) {
        showTimeInDefaultTimezone.value = !showTimeInDefaultTimezone.value;
    } else {
        transactionDateTimeSheetMode.value = sheetMode;
        showTransactionDateTimeSheet.value = true;
    }
}

function showOpenPictureDialog(): void {
    if (!canAddTransactionPicture.value || submitting.value || recognizingPicture.value) {
        return;
    }

    pictureInput.value?.click();
}

function shouldRecognizeUploadedPicture(): boolean {
    return pageTypeAndMode?.type === TransactionEditPageType.Transaction
        && mode.value === TransactionEditPageMode.Add;
}

function applyReceiptDraftCandidate(candidate: ReceiptDraftCandidateHint): void {
    if (!applyReceiptDraftFieldToTransaction(transaction.value, candidate.key, candidate.field)) {
        return;
    }

    receiptDraftCandidateHints.value = receiptDraftCandidateHints.value.filter(item => item.id !== candidate.id);
}

function applyReceiptRecognitionResult(result: RecognizedReceiptImageResponse): void {
    receiptDraftCandidateHints.value = buildReceiptDraftCandidateHints(result.draft);
    applyReceiptDraftAutoFillToTransaction(transaction.value, result);

    if (result.confidence !== null && result.confidence < RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD) {
        showToast('Low confidence recognition, please verify');
    }
}

function showReceiptRecognitionError(error: RecognizeReceiptImageError | unknown): void {
    const typed = error as RecognizeReceiptImageError;
    const errorCode = typed && typeof typed.errorCode === 'string' ? typed.errorCode : 'unknown';

    if (errorCode === 'provider_unconfigured') {
        showToast('OCR recognition requires configuration in Rule Center');
        return;
    }
    if (errorCode === 'timeout') {
        showToast('Recognition timed out, please try again');
        return;
    }
    if (errorCode === 'parse_error') {
        showToast('Could not parse this image, please try a clearer one');
        return;
    }
    if (errorCode === 'rate_limited') {
        showToast('Too many requests, please wait a moment');
        return;
    }

    showToast('Unable to recognize image');
}

async function recognizeUploadedPicture(pictureFile: File): Promise<void> {
    if (!shouldRecognizeUploadedPicture()) {
        return;
    }

    recognizingPicture.value = true;
    try {
        const result = await transactionsStore.recognizeReceiptImage({ imageFile: pictureFile });
        applyReceiptRecognitionResult(result);
        showToast('Image recognized and filled');
    } catch (error) {
        showReceiptRecognitionError(error);
    } finally {
        recognizingPicture.value = false;
    }
}

async function uploadPicture(event: Event): Promise<void> {
    if (!event || !event.target) {
        return;
    }

    const el = event.target as HTMLInputElement;

    if (!el.files || !el.files.length) {
        return;
    }

    const pictureFile = el.files[0] as File;

    el.value = '';

    uploadingPicture.value = true;
    submitting.value = true;

    try {
        const response = await transactionsStore.uploadTransactionPicture({
            pictureFile,
            clientSessionId: clientSessionId.value
        });
        transaction.value.addPicture(response);
        uploadingPicture.value = false;
        await recognizeUploadedPicture(pictureFile);
    } catch (error: unknown) {
        const processed = !!(error && typeof error === 'object' && (error as { processed?: boolean }).processed);
        if (!processed) {
            const message = error && typeof error === 'object' && 'message' in error
                ? String((error as { message?: string }).message || '')
                : '';
            showToast(message || 'Unable to upload transaction picture');
        }
    } finally {
        uploadingPicture.value = false;
        submitting.value = false;
        recognizingPicture.value = false;
    }
}

function viewOrRemovePicture(pictureInfo: TransactionPictureInfoBasicResponse): void {
    if (mode.value !== TransactionEditPageMode.Add && mode.value !== TransactionEditPageMode.Edit && transaction.value.pictures && transaction.value.pictures.length) {
        pictureBrowser.value?.open();
        return;
    }

    showConfirm('Are you sure you want to remove this transaction picture?', () => {
        removingPictureId.value = pictureInfo.pictureId;
        submitting.value = true;

        transactionsStore.removeUnusedTransactionPicture({ pictureInfo }).then(response => {
            if (response) {
                transaction.value.removePicture(pictureInfo);
            }

            removingPictureId.value = '';
            submitting.value = false;
        }).catch(error => {
            if (error.error && error.error.errorCode === KnownErrorCode.TransactionPictureNotFound) {
                transaction.value.removePicture(pictureInfo);
            } else if (!error.processed) {
                showToast(error.message || error);
            }

            removingPictureId.value = '';
            submitting.value = false;
        });
    });
}

function duplicate(withTime?: boolean, withGeoLocation?: boolean): void {
    props.f7router.navigate(`/transaction/add?id=${transaction.value.id}&type=${transaction.value.type}&withTime=${withTime ?? false}&withGeoLocation=${withGeoLocation ?? false}`);
}

function onPageAfterIn(): void {
    routeBackOnError(props.f7router, loadingError);

    if (settingsStore.appSettings.autoGetCurrentGeoLocation && mode.value === TransactionEditPageMode.Add
        && !geoLocationStatus.value && !transaction.value.geoLocation) {
        updateGeoLocation(false);
    }
}

function onPageBeforeOut(): void {
    if (submitted.value || pageTypeAndMode?.type !== TransactionEditPageType.Transaction || mode.value !== TransactionEditPageMode.Add || query['noTransactionDraft'] === 'true' || addByTemplateId.value || duplicateFromId.value) {
        return;
    }

    const initAmount: number | undefined = parseStrictQueryCents(query['sourceAmountCents']);

    if (settingsStore.appSettings.autoSaveTransactionDraft === 'confirmation') {
        if (transactionsStore.isTransactionDraftModified(transaction.value, initAmount, query['categoryId'], query['accountId'], query['tagIds'], firstVisibleAccountId.value)) {
            showConfirm('Do you want to save this transaction draft?', () => {
                transactionsStore.saveTransactionDraft(transaction.value, initAmount, query['categoryId'], query['accountId'], query['tagIds'], firstVisibleAccountId.value);
            }, () => {
                transactionsStore.clearTransactionDraft();
            });
        } else {
            transactionsStore.clearTransactionDraft();
        }
    } else if (settingsStore.appSettings.autoSaveTransactionDraft === 'enabled') {
        transactionsStore.saveTransactionDraft(transaction.value, initAmount, query['categoryId'], query['accountId'], query['tagIds'], firstVisibleAccountId.value);
    }
}

// 监听type变化：切换交易类型时重置分类选择
watch(() => transaction.value.type, (newType, oldType) => {
    if (oldType !== undefined && newType !== oldType) {
        // 清空所有分类ID
        transaction.value.expenseCategoryId = '';
        transaction.value.incomeCategoryId = '';
        transaction.value.transferCategoryId = '';
        transaction.value.investmentCategoryId = '';

        logger.info(`交易类型从 ${oldType} 切换到 ${newType}，已重置分类选择`);
    }
});

init();
useExternalTemplateBindings(accountsStore, addByTemplateId, addDefaultCategoriesAndOpenSheet, addingDefaultCategories, allAccountsMap, allCategories, allCategoriesMap, allTags, allTagsMap, allTimezones, allVisibleAccounts, allVisibleCategorizedAccounts, applyReceiptDraftAutoFillToTransaction, applyReceiptDraftCandidate, applyReceiptDraftFieldToTransaction, applyReceiptRecognitionResult, buildReceiptDraftCandidateHints, buildTransactionPictureItems, buildTransactionThumbs, canAddTransactionPicture, categorizedArrayToPlainArray, CategoryType, clearGeoLocation, clientSessionId, computed, coordinateDisplayType, currentTimezoneOffsetMinutes, defaultAccountId, defaultCurrency, destinationAccountCurrency, destinationAccountName, destinationAmountClass, duplicate, duplicateFromId, editId, environmentsStore, firstDayOfWeek, firstVisibleAccountId, formatCoordinate, formatGregorianTextualYearMonthDayToLongDate, formatUnixTimeToLongDate, formatUnixTimeToLongTime, generateRandomUUID, geoLocationStatus, GeoLocationStatus, geoLocationStatusInfo, getActualUnixTimeForStore, getAllTransactionDefaultCategories, getBrowserTimezoneOffsetMinutes, getCurrentLanguageTag, getDisplayAmount, getFontClassByAmount, getMapProvider, getMultiMonthdayShortNames, getMultiWeekdayLongNames, getPageTypeNameMode, getReceiptDraftCandidateDisplayValue, getTagName, getTimezoneOffset, getTimezoneOffsetMinutes, getTransactionPictureUrl, getTransactionPrimaryCategoryName, getTransactionSecondaryCategoryName, handleCategoryItemClick, hasAvailableCategoriesForType, hasAvailableExpenseCategories, hasAvailableIncomeCategories, hasAvailableInvestmentCategories, hasAvailableTransferCategories, hideLoading, init, inputEmptyProblemMessage, inputIsEmpty, isDarkMode, isSupportGeoLocation, isTransactionPicturesEnabled, KnownErrorCode, loading, loadingError, localizedPresetCategoriesToTransactionCategoryCreateWithSubCategories, logger, MobileTransactionPicturesPanel, mode, numeralSystem, onPageAfterIn, onPageBeforeOut, pageTypeAndMode, parseStrictQueryCents, pictureBrowser, pictureInput, props, query, RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD, receiptDraftCandidateHints, recognizeUploadedPicture, recognizingPicture, ref, removingPictureId, routeBackOnError, save, saveButtonTitle, ScheduledTemplateFrequencyType, setGeoLocationByClickMap, settingsStore, setTransactionModelByTransaction, shouldRecognizeUploadedPicture, showAlert, showCategorySheet, showConfirm, showDateTimeDialog, showDestinationAccountSheet, showDestinationAmountSheet, showGeoLocationActionSheet, showGeoLocationMapSheet, showLoading, showMoreActionSheet, showOpenPictureDialog, showReceiptRecognitionError, showScheduledEndDateSheet, showScheduledStartDateSheet, showSourceAccountSheet, showSourceAmountSheet, showTimeInDefaultTimezone, showTimezonePopup, showToast, showTransactionDateTimeSheet, showTransactionPictures, showTransactionScheduledFrequencySheet, showTransactionTagSheet, sourceAccountCurrency, sourceAccountName, sourceAccountTitle, sourceAmountClass, sourceAmountTitle, submitted, submitting, SUPPORTED_IMAGE_EXTENSIONS, swapTransactionData, TemplateType, title, transaction, Transaction, TRANSACTION_MAX_AMOUNT, TRANSACTION_MIN_AMOUNT, transactionCategoriesStore, transactionDateTimeSheetMode, transactionDisplayDate, transactionDisplayScheduledEndDate, transactionDisplayScheduledFrequency, transactionDisplayScheduledStartDate, transactionDisplayTime, transactionDisplayTimezone, transactionDisplayTimezoneName, TransactionEditPageMode, TransactionEditPageType, TransactionEditScopeType, transactionPictures, transactionsStore, transactionTagsStore, TransactionTemplate, transactionTemplatesStore, transactionThumbs, transactionTimezoneTimeDifference, TransactionType, transferInAmountTitle, tt, updateGeoLocation, uploadingPicture, uploadPicture, useAccountsStore, useEnvironmentsStore, useI18n, useI18nUIComponents, userStore, useSettingsStore, useTemplateRef, useTransactionCategoriesStore, useTransactionEditPageBase, useTransactionsStore, useTransactionTagsStore, useTransactionTemplatesStore, useUserStore, viewOrRemovePicture, watch);
</script>

<style src="./edit-page/EditPage.css"></style>
