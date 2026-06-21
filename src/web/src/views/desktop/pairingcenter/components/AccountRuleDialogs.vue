<template>
    <v-dialog
        v-model="showEditDialog"
        class="account-rule-edit-dialog"
        width="calc(100vw - 32px)"
        max-width="720"
        persistent
    >
        <v-card class="account-rule-edit-card pa-2 pa-sm-4 pa-md-8">
            <template #title>
                <div class="d-flex w-100 align-center justify-center">
                    <h4 class="text-h4">{{ editingRule ? tt('Edit Rule') : tt('Create Rule') }}</h4>
                </div>
            </template>
            <v-card-text class="account-rule-edit-dialog-content mt-md-4 pt-0">
                <v-form>
                    <v-row>
                        <v-col cols="12" md="7">
                            <v-select
                                v-model="ruleForm.accountId"
                                :items="accountOptions"
                                item-title="name"
                                item-value="id"
                                density="comfortable"
                                variant="outlined"
                                :label="tt('Account')"
                                :disabled="saving || isAccountLocked"
                            >
                                <template #item="{ props: itemProps, item }">
                                    <v-list-item v-bind="itemProps" :title="item.raw.name">
                                        <template #prepend>
                                            <ItemIcon icon-type="account" :icon-id="item.raw.icon" :color="item.raw.color" />
                                        </template>
                                    </v-list-item>
                                </template>
                            </v-select>
                        </v-col>
                        <v-col cols="12" md="5">
                            <v-text-field
                                v-model.number="ruleForm.priority"
                                type="number"
                                density="comfortable"
                                variant="outlined"
                                :label="tt('Priority')"
                                :disabled="saving"
                            />
                        </v-col>
                        <v-col cols="12">
                            <category-rule-builder-fields
                                v-model="ruleBuilderModel"
                                :auto-rule-name="autoRuleName"
                                :disabled="saving"
                                title="Account Matching"
                            />
                        </v-col>
                    </v-row>
                </v-form>
            </v-card-text>
            <v-card-text class="overflow-y-visible">
                <div class="account-rule-edit-dialog-actions w-100 d-flex justify-center mt-2 mt-sm-4 mt-md-6 gap-4">
                    <v-btn variant="tonal" :disabled="saving" @click="showEditDialog = false">
                        {{ tt('Cancel') }}
                    </v-btn>
                    <v-btn color="primary" :loading="saving" @click="$emit('save')">
                        {{ tt('Save') }}
                    </v-btn>
                </div>
            </v-card-text>
        </v-card>
    </v-dialog>

    <v-dialog v-model="showTestDialog" max-width="560">
        <v-card>
            <v-card-title>{{ tt('Test Rule') }}: {{ testRuleName }}</v-card-title>
            <v-card-text>
                <v-textarea
                    v-model="testText"
                    rows="3"
                    :label="tt('Text to test')"
                    :placeholder="tt('Enter parser, counterparty, payment method or description text')"
                />
                <v-alert v-if="testResult !== null" :type="testResult ? 'success' : 'warning'" class="mt-3">
                    {{ testResult ? tt('Match!') : tt('No match') }}
                </v-alert>
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="showTestDialog = false">{{ tt('Close') }}</v-btn>
                <v-btn color="primary" :loading="testing" @click="$emit('run-test')">{{ tt('Test') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <v-dialog v-model="showDeleteDialog" max-width="400">
        <v-card>
            <v-card-title>{{ tt('Delete Rule') }}</v-card-title>
            <v-card-text>
                {{ tt('Are you sure you want to delete rule') }} "{{ deletingRule?.name }}"?
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="showDeleteDialog = false">{{ tt('Cancel') }}</v-btn>
                <v-btn color="error" :loading="deleting" @click="$emit('delete')">{{ tt('Delete') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
</template>

<script setup lang="ts">
import CategoryRuleBuilderFields from '@/components/common/CategoryRuleBuilderFields.vue';
import ItemIcon from '@/components/desktop/ItemIcon.vue';
import { useI18n } from '@/locales/helpers.ts';
import type { AccountRuleForm, AccountRuleItem } from '@/models/account_rule.ts';

interface AccountRuleAccountOption {
    id: string;
    name: string;
    icon: string;
    color: string;
}

interface RuleBuilderModel {
    priority: number;
    ruleExpression: string;
    regexEnabled: boolean;
    enabled: boolean;
}

defineProps<{
    editingRule: AccountRuleItem | null;
    accountOptions: AccountRuleAccountOption[];
    autoRuleName: string;
    saving: boolean;
    isAccountLocked: boolean;
    testRuleName: string;
    testResult: boolean | null;
    testing: boolean;
    deletingRule: AccountRuleItem | null;
    deleting: boolean;
}>();

defineEmits<{
    save: [];
    'run-test': [];
    delete: [];
}>();

const showEditDialog = defineModel<boolean>('showEditDialog', { required: true });
const showTestDialog = defineModel<boolean>('showTestDialog', { required: true });
const showDeleteDialog = defineModel<boolean>('showDeleteDialog', { required: true });
const ruleForm = defineModel<AccountRuleForm>('ruleForm', { required: true });
const ruleBuilderModel = defineModel<RuleBuilderModel>('ruleBuilderModel', { required: true });
const testText = defineModel<string>('testText', { required: true });

const { tt } = useI18n();
</script>

<style scoped>
.account-rule-edit-dialog-content {
    max-height: min(64vh, 640px);
    overflow-y: auto;
}

.account-rule-edit-dialog-actions {
    align-items: center;
}
</style>
