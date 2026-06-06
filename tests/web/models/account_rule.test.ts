import { describe, expect, test } from '@jest/globals';

import { AccountCategory } from '@/core/account.ts';
import {
    buildAccountRuleGroups,
    buildAccountRulePayload,
    buildAccountRuleTestContext,
    normalizeAccountRuleItem,
    type AccountRuleGroupingAccount,
} from '@/models/account_rule.ts';

describe('account rule model', () => {
    test('builds current expression-only payloads and test contexts', () => {
        const payload = buildAccountRulePayload({
            accountId: '12',
            name: '招商银行规则',
            priority: 5,
            ruleExpression: 'counterparty contains "招商"',
            regexEnabled: false,
            enabled: true,
        }, 'Fallback');
        const testContext = buildAccountRuleTestContext(' 招商银行快捷支付 ');

        expect(payload).toEqual({
            account_id: 12,
            name: '招商银行规则',
            priority: 5,
            rule_expression: 'counterparty contains "招商"',
            regex_enabled: false,
            enabled: true,
        });
        expect(payload).not.toHaveProperty('account_role_scope');
        expect(payload).not.toHaveProperty('transaction_type_scope');
        expect(payload).not.toHaveProperty('field_scope');
        expect(testContext).toEqual({
            context: {
                text: '招商银行快捷支付',
                counterparty: '招商银行快捷支付',
                paymentMethod: '招商银行快捷支付',
                description: '招商银行快捷支付',
                parserId: '招商银行快捷支付',
                parserLabel: '招商银行快捷支付',
            },
        });
        expect(testContext).not.toHaveProperty('account_role_scope');
        expect(testContext).not.toHaveProperty('transaction_type_scope');
        expect(testContext).not.toHaveProperty('field_scope');
    });

    test('normalizes old API rows without leaking legacy scope fields', () => {
        const item = normalizeAccountRuleItem({
            id: 1,
            account_id: 12,
            account_name: '招商储蓄卡',
            account_type: AccountCategory.CheckingAccount.type,
            name: '旧规则',
            priority: 20,
            rule_expression: '招商',
            regex_enabled: true,
            enabled: true,
            applied_count: 2,
            match_count: 3,
            account_role_scope: 'source',
            transaction_type_scope: 'income',
            field_scope: ['counterparty'],
        });

        expect(item.accountId).toBe(12);
        expect(item.ruleExpression).toBe('招商');
        expect(item.regexEnabled).toBe(true);
        expect(item.matchCount).toBe(3);
        expect(item).not.toHaveProperty('accountRoleScope');
        expect(item).not.toHaveProperty('transactionTypeScope');
        expect(item).not.toHaveProperty('fieldScope');
    });

    test('groups multiple expressions under primary and secondary accounts', () => {
        const accounts: AccountRuleGroupingAccount[] = [
            {
                id: '10',
                name: '银行',
                parentId: '0',
                category: AccountCategory.CheckingAccount.type,
                displayOrder: 1,
                icon: '100',
                color: '#409eff',
            },
            {
                id: '11',
                name: '储蓄卡',
                parentId: '10',
                category: AccountCategory.CheckingAccount.type,
                displayOrder: 2,
                icon: '100',
                color: '#409eff',
            },
            {
                id: '20',
                name: '现金',
                parentId: '0',
                category: AccountCategory.Cash.type,
                displayOrder: 1,
                icon: '1',
                color: '#67c23a',
            },
        ];
        const rules = [
            normalizeAccountRuleItem({
                id: 3,
                account_id: 11,
                account_name: '储蓄卡',
                account_type: AccountCategory.CheckingAccount.type,
                name: '工资',
                priority: 20,
                rule_expression: '工资',
                match_count: 2,
            }),
            normalizeAccountRuleItem({
                id: 2,
                account_id: 11,
                account_name: '储蓄卡',
                account_type: AccountCategory.CheckingAccount.type,
                name: '招商',
                priority: 10,
                rule_expression: '招商',
                match_count: 3,
            }),
            normalizeAccountRuleItem({
                id: 1,
                account_id: 20,
                account_name: '现金',
                account_type: AccountCategory.Cash.type,
                name: '现金',
                priority: 5,
                rule_expression: '现金',
                match_count: 1,
            }),
        ];

        const groups = buildAccountRuleGroups(rules, accounts, key => key);

        expect(groups.map(group => group.categoryName)).toEqual(['Cash', 'Checking Account']);
        expect(groups.map(group => group.categoryIcon)).toEqual([
            AccountCategory.Cash.defaultAccountIconId,
            AccountCategory.CheckingAccount.defaultAccountIconId,
        ]);
        expect(groups[0]?.accounts[0]?.displayName).toBe('现金');
        expect(groups[1]?.accounts[0]?.displayName).toBe('银行 / 储蓄卡');
        expect(groups[1]?.accounts[0]?.rules.map(rule => rule.name)).toEqual(['招商', '工资']);
        expect(groups[1]?.accounts[0]?.ruleCount).toBe(2);
        expect(groups[1]?.accounts[0]?.matchCount).toBe(5);
        expect(groups[1]?.ruleCount).toBe(2);
        expect(groups[1]?.matchCount).toBe(5);
    });
});
