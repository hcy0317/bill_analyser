import { describe, expect, test } from '@jest/globals';

import {
    EMPTY_USER_BASIC_INFO,
    normalizeUserBasicInfo,
    User,
    type UserBasicInfo
} from '@/models/user.ts';

const SAMPLE_USER: UserBasicInfo = {
    ...EMPTY_USER_BASIC_INFO,
    id: 7,
    username: 'cyanslot',
    email: 'cyan@example.com',
    nickname: 'Cyan',
    avatar: 'avatar.png',
    defaultAccountId: '101',
    language: 'zh-CN',
    defaultCurrency: 'CNY',
    firstDayOfWeek: 1,
    cashAccountId: '202',
    cashTransferCategoryId: '303',
    investmentPlatformKeywords: ['基金平台'],
    investmentProductKeywords: ['指数基金'],
    investmentExcludeKeywords: ['体验金'],
    emailVerified: true
};

describe('User model helpers', () => {
    test('normalizeUserBasicInfo fills defaults and normalizes id-like fields to strings', () => {
        const normalized = normalizeUserBasicInfo({
            username: 'demo',
            defaultAccountId: 99 as unknown as string,
            cashAccountId: null as unknown as string,
            cashTransferCategoryId: undefined,
            investmentPlatformKeywords: ['平台A']
        });

        expect(normalized.username).toBe('demo');
        expect(normalized.defaultAccountId).toBe('99');
        expect(normalized.cashAccountId).toBe('');
        expect(normalized.cashTransferCategoryId).toBe('');
        expect(normalized.language).toBe(EMPTY_USER_BASIC_INFO.language);
        expect(normalized.investmentPlatformKeywords).toStrictEqual(['平台A']);
        expect(normalized.investmentProductKeywords).toStrictEqual([]);
        expect(normalized.investmentExcludeKeywords).toStrictEqual([]);
    });

    test('normalizeUserBasicInfo returns empty defaults for nullish input', () => {
        expect(normalizeUserBasicInfo()).toStrictEqual(EMPTY_USER_BASIC_INFO);
        expect(normalizeUserBasicInfo(null)).toStrictEqual(EMPTY_USER_BASIC_INFO);
    });

    test('User.of copies normalized values and clones keyword arrays', () => {
        const user = User.of(SAMPLE_USER);

        expect(user.username).toBe('cyanslot');
        expect(user.language).toBe('zh-CN');
        expect(user.defaultCurrency).toBe('CNY');
        expect(user.defaultAccountId).toBe('101');
        expect(user.cashAccountId).toBe('202');
        expect(user.cashTransferCategoryId).toBe('303');
        expect(user.investmentPlatformKeywords).toStrictEqual(['基金平台']);
        expect(user.investmentProductKeywords).toStrictEqual(['指数基金']);
        expect(user.investmentExcludeKeywords).toStrictEqual(['体验金']);

        SAMPLE_USER.investmentPlatformKeywords.push('不应共享');
        expect(user.investmentPlatformKeywords).toStrictEqual(['基金平台']);
        SAMPLE_USER.investmentPlatformKeywords.pop();
    });

    test('User.fillFrom copies profile data and keeps arrays detached', () => {
        const target = User.createNewUser('en-US', 'USD', 0);
        const source = User.of(SAMPLE_USER);

        target.fillFrom(source);
        source.investmentProductKeywords.push('新增关键字');

        expect(target.email).toBe('cyan@example.com');
        expect(target.nickname).toBe('Cyan');
        expect(target.language).toBe('zh-CN');
        expect(target.defaultCurrency).toBe('CNY');
        expect(target.investmentProductKeywords).toStrictEqual(['指数基金']);
    });

    test('User request converters preserve current editable state', () => {
        const user = User.createNewUser('zh-CN', 'CNY', 1);
        user.username = 'register-user';
        user.email = 'register@example.com';
        user.nickname = '注册用户';
        user.password = 'secret';
        user.defaultAccountId = '101';
        user.cashAccountId = '202';
        user.cashTransferCategoryId = '303';
        user.investmentPlatformKeywords = ['平台'];
        user.investmentProductKeywords = ['产品'];
        user.investmentExcludeKeywords = ['排除'];

        expect(user.toRegisterRequest()).toStrictEqual({
            username: 'register-user',
            email: 'register@example.com',
            nickname: '注册用户',
            password: 'secret',
            language: 'zh-CN',
            defaultCurrency: 'CNY',
            firstDayOfWeek: 1,
            categories: undefined
        });

        expect(user.toProfileUpdateRequest('old-secret')).toMatchObject({
            email: 'register@example.com',
            nickname: '注册用户',
            password: 'secret',
            oldPassword: 'old-secret',
            defaultAccountId: '101',
            cashAccountId: '202',
            cashTransferCategoryId: '303',
            investmentPlatformKeywords: ['平台'],
            investmentProductKeywords: ['产品'],
            investmentExcludeKeywords: ['排除']
        });
    });

    test('User.createNewUser seeds the requested locale defaults', () => {
        const user = User.createNewUser('ja-JP', 'JPY', 1);

        expect(user.language).toBe('ja-JP');
        expect(user.defaultCurrency).toBe('JPY');
        expect(user.firstDayOfWeek).toBe(1);
        expect(user.defaultAccountId).toBe(EMPTY_USER_BASIC_INFO.defaultAccountId);
        expect(user.importLearningEnabled).toBe(EMPTY_USER_BASIC_INFO.importLearningEnabled);
    });
});
