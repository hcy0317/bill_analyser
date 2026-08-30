<template>
    <f7-block class="combination-list-wrapper margin-vertical" :class="{ 'no-accordion-toggle': pageType !== TransactionListPageType.List.type }">
        <f7-accordion-item :opened="monthList.opened"
                           @accordion:open="emit('collapseMonth', monthList, false)"
                           @accordion:opened="emit('collapseStateChanged')"
                           @accordion:close="emit('collapseMonth', monthList, true)"
                           @accordion:closed="emit('collapseStateChanged')">
            <f7-block-title :id="getTransactionMonthTitleDomId(monthList.yearDashMonth)" v-if="pageType === TransactionListPageType.List.type">
                <f7-accordion-toggle>
                    <f7-list strong inset dividers media-list
                             class="transaction-amount-list combination-list-header"
                             :class="monthList.opened ? 'combination-list-opened' : 'combination-list-closed'">
                        <f7-list-item>
                            <template #title>
                                <small>
                                    <span>{{ getDisplayLongYearMonth(monthList) }}</span>
                                </small>
                                <small class="transaction-amount-statistics" v-if="showTotalAmount && monthList.totalAmountCents">
                                    <span class="text-income">
                                        {{ getDisplayMonthTotalAmount(monthList.totalAmountCents.incomeCents, defaultCurrency, '+', monthList.totalAmountCents.incompleteIncome) }}
                                    </span>
                                    <span class="text-expense">
                                        {{ getDisplayMonthTotalAmount(monthList.totalAmountCents.expenseCents, defaultCurrency, '-', monthList.totalAmountCents.incompleteExpense) }}
                                    </span>
                                </small>
                                <f7-icon class="combination-list-chevron-icon" :f7="monthList.opened ? 'chevron_up' : 'chevron_down'"></f7-icon>
                            </template>
                        </f7-list-item>
                    </f7-list>
                </f7-accordion-toggle>
            </f7-block-title>
            <f7-accordion-content>
                <f7-block :style="{ height: getTransactionMonthListHeight(monthList) }"
                          v-if="isTransactionMonthListInvisible(monthList)" />
                <f7-list strong inset dividers media-list accordion-list
                         class="transaction-info-list transaction-month-list combination-list-content"
                         :id="getTransactionMonthListDomId(monthList.yearDashMonth)"
                         v-if="!isTransactionMonthListInvisible(monthList)"
                >
                    <f7-list-item swipeout chevron-center accordion-item
                                  class="transaction-info"
                                  :id="getTransactionDomId(transaction)"
                                  :link="`/transaction/detail?id=${transaction.id}&type=${transaction.type}`"
                                  :key="transaction.id"
                                  v-for="(transaction, idx) in monthList.items"
                    >
                        <template #media>
                            <div class="display-flex flex-direction-column transaction-date" :style="getTransactionDateStyle(transaction, idx > 0 ? monthList.items[idx - 1] : undefined)">
                                <span class="transaction-day full-line flex-direction-column">
                                    {{ getCalendarDisplayDayOfMonthFromUnixTime(transaction.time) }}
                                </span>
                                <span class="transaction-day-of-week full-line flex-direction-column" v-if="transaction.getDisplayDayOfWeekObject()">
                                    {{ getWeekdayShortName(transaction.getDisplayDayOfWeekObject()!) }}
                                </span>
                            </div>
                        </template>
                        <template #inner>
                            <div class="display-flex no-padding-horizontal">
                                <div class="item-media">
                                    <div class="transaction-icon display-flex align-items-center">
                                        <ItemIcon icon-type="category"
                                                  :icon-id="transaction.category.icon"
                                                  :color="transaction.category.color"
                                                  v-if="transaction.category && transaction.category.color"></ItemIcon>
                                        <f7-icon v-else-if="!transaction.category || !transaction.category.color"
                                                 f7="pencil_ellipsis_rectangle">
                                        </f7-icon>
                                    </div>
                                </div>
                                <div class="actual-item-inner">
                                    <div class="item-title-row">
                                        <div class="item-title">
                                            <div class="transaction-category-name no-padding">
                                                <span v-if="transaction.type === TransactionType.ModifyBalance">
                                                    {{ tt('Modify Balance') }}
                                                </span>
                                                <span v-else-if="transaction.type !== TransactionType.ModifyBalance && transaction.category">
                                                    {{ transaction.category.name }}
                                                </span>
                                                <span v-else-if="transaction.type !== TransactionType.ModifyBalance && !transaction.category">
                                                    {{ getTransactionTypeName(transaction.type, 'Transaction') }}
                                                </span>
                                            </div>
                                        </div>
                                        <div class="item-after">
                                            <div class="transaction-amount" v-if="transaction.sourceAccount"
                                                 :class="{ 'text-expense': transaction.type === TransactionType.Expense, 'text-income': transaction.type === TransactionType.Income }">
                                                <span>{{ getDisplayAmount(transaction) }}</span>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="item-text">
                                        <div class="transaction-description" v-if="transaction.comment">
                                            <span>{{ transaction.comment }}</span>
                                        </div>
                                    </div>
                                    <div class="item-footer">
                                        <div class="transaction-tags" v-if="showTags && transaction.tagIds && transaction.tagIds.length">
                                            <f7-chip media-text-color="var(--f7-chip-text-color)" class="transaction-tag"
                                                     :text="allTransactionTags[tagId]?.name"
                                                     :title="allTransactionTags[tagId]?.name || ''"
                                                     :aria-label="allTransactionTags[tagId]?.name || ''"
                                                     :key="tagId"
                                                     v-for="tagId in transaction.tagIds">
                                                <template #media>
                                                    <f7-icon f7="number"></f7-icon>
                                                </template>
                                            </f7-chip>
                                        </div>
                                        <div class="transaction-footer">
                                            <span>{{ getDisplayTime(transaction) }}</span>
                                            <span v-if="transaction.utcOffset !== currentTimezoneOffsetMinutes">{{ `(${getDisplayTimezone(transaction)})` }}</span>
                                            <span v-if="transaction.sourceAccount">·</span>
                                            <span v-if="transaction.sourceAccount">{{ transaction.sourceAccount.name }}</span>
                                            <f7-icon class="transaction-account-arrow icon-with-direction" f7="arrow_right" v-if="transaction.sourceAccount && (transaction.type === TransactionType.Transfer || transaction.type === TransactionType.Investment) && transaction.destinationAccount && transaction.sourceAccount.id !== transaction.destinationAccount.id"></f7-icon>
                                            <span v-if="transaction.sourceAccount && (transaction.type === TransactionType.Transfer || transaction.type === TransactionType.Investment) && transaction.destinationAccount && transaction.sourceAccount.id !== transaction.destinationAccount.id">{{ transaction.destinationAccount.name }}</span>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </template>
                        <f7-swipeout-actions :left="textDirection === TextDirection.RTL"
                                             :right="textDirection === TextDirection.LTR">
                            <f7-swipeout-button color="primary" close
                                                :text="tt('Duplicate')"
                                                v-if="transaction.type !== TransactionType.ModifyBalance"
                                                @click="emit('duplicate', transaction)"></f7-swipeout-button>
                            <f7-swipeout-button color="orange" close
                                                :text="tt('Edit')"
                                                v-if="transaction.editable"
                                                @click="emit('edit', transaction)"></f7-swipeout-button>
                            <f7-swipeout-button color="red" class="padding-horizontal"
                                                v-if="transaction.editable"
                                                @click="emit('remove', transaction, false)">
                                <f7-icon f7="trash"></f7-icon>
                            </f7-swipeout-button>
                        </f7-swipeout-actions>
                    </f7-list-item>
                </f7-list>
            </f7-accordion-content>
        </f7-accordion-item>
    </f7-block>
</template>

<script setup lang="ts">
import { useI18n } from '@/locales/helpers.ts';
import { TransactionListPageType } from '@/views/base/transactions/TransactionListPageBase.ts';

import { TextDirection } from '@/core/text.ts';
import { TransactionType } from '@/core/transaction.ts';
import type { TextualYearMonth } from '@/core/datetime.ts';
import type { TransactionMonthList } from '@/stores/transaction.ts';
import type { Transaction } from '@/models/transaction.ts';

defineProps<{
    monthList: TransactionMonthList;
    pageType: number;
    showTotalAmount: boolean;
    showTags: boolean;
    defaultCurrency: string;
    currentTimezoneOffsetMinutes: number;
    textDirection: TextDirection;
    allTransactionTags: Record<string, { name: string } | undefined>;
    getDisplayLongYearMonth: (monthList: TransactionMonthList) => string;
    getDisplayMonthTotalAmount: (amount: number, currency: string | false, symbol: string, incomplete: boolean) => string;
    getTransactionMonthTitleDomId: (yearMonth: TextualYearMonth) => string;
    getTransactionMonthListDomId: (yearMonth: TextualYearMonth) => string;
    getTransactionMonthListHeight: (monthList: TransactionMonthList) => string;
    isTransactionMonthListInvisible: (monthList: TransactionMonthList) => boolean;
    getTransactionDomId: (transaction: Transaction) => string;
    getTransactionDateStyle: (transaction: Transaction, previousTransaction: Transaction | undefined) => Record<string, string>;
    getTransactionTypeName: (type: number | null, defaultName: string) => string;
    getDisplayAmount: (transaction: Transaction) => string;
    getDisplayTime: (transaction: Transaction) => string;
    getDisplayTimezone: (transaction: Transaction) => string;
}>();

const emit = defineEmits<{
    collapseMonth: [monthList: TransactionMonthList, collapse: boolean];
    collapseStateChanged: [];
    duplicate: [transaction: Transaction];
    edit: [transaction: Transaction];
    remove: [transaction: Transaction, confirm: boolean];
}>();

const {
    tt,
    getWeekdayShortName,
    getCalendarDisplayDayOfMonthFromUnixTime
} = useI18n();
</script>
