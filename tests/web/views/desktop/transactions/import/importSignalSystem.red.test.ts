import { describe, expect, test } from '@jest/globals';
import { previewStateSnapshot } from '../../../../helpers/importPreviewState.ts';

import { TransactionType } from '@/core/transaction.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';
import * as signalContract from '@/views/desktop/transactions/import/checkDataMatching.ts';
import {
    buildImportPreviewSignalViewModel,
    isImportPreviewCanonicalTruthy,
    matchesImportPreviewSignalFilter,
    normalizeImportPreviewSignalStatusAlias,
    type ImportPreviewSignalState,
    type ImportPreviewSignalStatus,
    type ImportPreviewVisibleSignalFilterValue
} from '@/views/desktop/transactions/import/checkDataMatching.ts';
import {
    buildImportPreviewIndexSignalViewModel,
    mapImportPreviewIndexResponseItem
} from '@/views/desktop/transactions/import/importPreviewIndex.ts';
import {
    buildImportTransactionFromPreviewRecord,
    type ImportPreviewTransactionDraft
} from '@/views/desktop/transactions/import/importPreviewTransaction.ts';
import { buildImportPreviewUpdateFromTransaction } from '@/views/desktop/transactions/import/importPreviewUpdates.ts';
import type { ImportPreviewRecord } from '@/views/desktop/transactions/import/importPreview.ts';

const SIX_FAMILIES = [
    'parser',
    'platform_duplicate',
    'transfer',
    'history',
    'learning',
    'llm'
] as const satisfies readonly ImportPreviewVisibleSignalFilterValue[];

const LLM_FIXTURE = {
    suggested_type: '支出',
    suggested_category_id: 3,
    suggested_main_category: '餐饮',
    suggested_sub_category: '咖啡',
    suggested_source_account: '招商银行',
    suggested_destination_account: '支付宝',
    confidence: 0.87,
    reason: '根据历史记忆推荐',
    review_status: 'pending',
    suppressed: false
};

function makeDraft(): ImportPreviewTransactionDraft {
    const transaction = ImportTransaction.of({
        type: TransactionType.Expense,
        categoryId: '',
        originalCategoryName: '',
        time: 0,
        utcOffset: 480,
        sourceAccountId: '101',
        originalSourceAccountName: '招商银行',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: '',
        sourceAmountCents: 1234,
        destinationAmountCents: 0,
        tagIds: [],
        originalTagNames: [],
        comment: '',
        selected: true
    }, 0) as ImportPreviewTransactionDraft;
    transaction._previewId = 7;
    return transaction;
}

describe('import signal system RED contracts', () => {
    test('publishes the canonical six-family TypeScript checked mirror in backend order', () => {
        expect((signalContract as unknown as Record<string, unknown>)['IMPORT_PREVIEW_VISIBLE_SIGNAL_FILTERS'])
            .toStrictEqual(SIX_FAMILIES);
    });

    test.each([
        ['empty learning', { learningStatus: 'pending' }, 'learning'],
        ['none learning', { learningStatus: 'none', learningSummary: '支出 | 餐饮' }, 'learning'],
        ['suppressed learning', { learningStatus: 'pending', learningSummary: '支出 | 餐饮', learningSuppressed: true }, 'learning'],
        ['empty llm', { llmStatus: 'pending' }, 'llm'],
        ['none llm', { llmStatus: 'none', llmCategoryPath: '餐饮/咖啡' }, 'llm'],
        ['suppressed llm', { llmStatus: 'pending', llmCategoryPath: '餐饮/咖啡', llmSuppressed: true }, 'llm']
    ])('does not create a visible/filter family from %s', (_name, rawState, family) => {
        const view = buildImportPreviewSignalViewModel(rawState as unknown as ImportPreviewSignalState);

        expect(view[family as 'learning' | 'llm']).toBeNull();
        expect(matchesImportPreviewSignalFilter(view, family as ImportPreviewVisibleSignalFilterValue)).toBe(false);
    });

    test.each([
        ['learning', { learningSummary: '支出 | 餐饮/咖啡' }],
        ['llm', { llmCategoryPath: '餐饮/咖啡', llmConfidence: 0.87, llmTitle: '根据历史记忆推荐' }]
    ])('makes matching-only meaningful %s data visible as pending', (family, rawState) => {
        const view = buildImportPreviewSignalViewModel(rawState as ImportPreviewSignalState);
        const familyView = view[family as 'learning' | 'llm'];

        expect(familyView).toMatchObject({ status: 'pending' });
        expect(matchesImportPreviewSignalFilter(view, family as ImportPreviewVisibleSignalFilterValue)).toBe(true);
    });

    test('applies the backend parser exclusion rule to the visible family view', () => {
        const view = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            learningStatus: 'pending',
            learningSummary: '支出 | 餐饮/咖啡'
        });

        expect(view.parser).toBeNull();
        expect(matchesImportPreviewSignalFilter(view, 'parser')).toBe(false);
        expect(matchesImportPreviewSignalFilter(view, 'learning')).toBe(true);
    });

    test.each([
        ['accepted', 'accepted'],
        ['rejected', 'rejected'],
        ['skipped', 'skipped'],
        ['auto_applied', 'accepted'],
        ['auto-applied', 'accepted'],
    ] as const)('keeps terminal transfer %s displayable without transfer-family membership', (rawStatus, displayedStatus) => {
        const view = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            parserTags: ['parser:alipay'],
            transferStatus: rawStatus,
            transferTitle: '历史转账决策',
        });

        expect(view.transferSuggestion).toMatchObject({ status: displayedStatus });
        expect(matchesImportPreviewSignalFilter(view, 'transfer')).toBe(false);
        expect(matchesImportPreviewSignalFilter(view, 'parser')).toBe(true);
    });

    test('matches sparse platform duplicates from canonical dedup type without source context', () => {
        const sparseView = buildImportPreviewSignalViewModel({
            dedupType: '\u3000platform_bank\u00A0',
        });
        const contextualView = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            parserTags: ['parser:alipay'],
            dedupType: '\u3000platform_bank\u00A0',
        });

        expect(sparseView.dedup).toMatchObject({ dedupType: '\u3000platform_bank\u00A0' });
        expect(matchesImportPreviewSignalFilter(sparseView, 'platform_duplicate')).toBe(true);
        expect(matchesImportPreviewSignalFilter(contextualView, 'platform_duplicate')).toBe(true);
        expect(contextualView.dedup?.detailLines).toContain('Duplicate Sources: alipay');
        expect(matchesImportPreviewSignalFilter(contextualView, 'parser')).toBe(false);
    });

    test.each([
        ['learning lifecycle status', { learningLifecycleStatus: 'pending' }, 'learning'],
        ['learning signal state', { learningSignalState: 'pending' }, 'learning'],
        ['LLM lifecycle status', { llmLifecycleStatus: 'pending' }, 'llm'],
        ['LLM signal state', { llmSignalState: 'pending' }, 'llm'],
    ] as const)('does not treat status-only pending %s as business evidence', (_name, state, family) => {
        const view = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            parserTags: ['parser:alipay'],
            ...state,
        });

        expect(view[family]).toBeNull();
        expect(matchesImportPreviewSignalFilter(view, family)).toBe(false);
        expect(matchesImportPreviewSignalFilter(view, 'parser')).toBe(true);
    });

    test.each([
        ['400-digit learning score', { learningScore: '9'.repeat(400) }, 'learning'],
        ['tiny nonzero LLM confidence', { llmConfidence: `.${'0'.repeat(400)}1` }, 'llm'],
    ] as const)('uses lexical positive-decimal evidence for %s', (_name, state, family) => {
        const view = buildImportPreviewSignalViewModel(state);

        expect(view[family]).toMatchObject({ status: 'pending' });
        expect(matchesImportPreviewSignalFilter(view, family)).toBe(true);
    });

    test.each([
        ['zero', '000.000'],
        ['negative', '-0.0001'],
        ['trailing dot', '1.'],
        ['exponent', '1e400'],
    ])('rejects %s numeric text as positive signal evidence', (_name, value) => {
        const view = buildImportPreviewSignalViewModel({ learningScore: value });

        expect(view.learning).toBeNull();
        expect(matchesImportPreviewSignalFilter(view, 'learning')).toBe(false);
    });

    test('keeps normalized auxiliary row sections available without adding six-family filters', () => {
        const transaction = buildImportTransactionFromPreviewRecord({
            id: 45,
            preview_type: '支出',
            preview_amount_cents: 1234,
            preview_source_account_id: 101,
            matching: {
                parser: {
                    id: 'alipay',
                    tags: ['parser:alipay']
                },
                recurring: {
                    id: 8,
                    name: '月度周期',
                    candidate_count: 2,
                    match_score: 0.91,
                    match_reasons: 'monthly'
                },
                reconciliation: {
                    candidate_type: 'duplicate',
                    status: 'pending',
                    signal_label: '普通对账提示',
                    source_chain: []
                },
                identity_validation: {
                    status: 'invalid',
                    issues: [{ field: 'source_account', message: 'unknown account' }]
                }
            } as unknown as ImportPreviewRecord['matching']
        }, 0, {
            categoriesById: {},
            timeZone: 'Asia/Shanghai'
        }) as ImportPreviewTransactionDraft;
        const matching = transaction.matching as unknown as Record<string, Record<string, unknown> | undefined>;

        const reconciliation = matching['reconciliation'];

        expect(matching['parser']).toMatchObject({ id: 'alipay', tags: ['parser:alipay'] });
        expect(matching['recurring']).toMatchObject({ id: 8, candidate_count: 2, match_reasons: 'monthly' });
        expect(reconciliation).toMatchObject({ status: 'pending', signal_label: '普通对账提示' });
        expect(matching['identity_validation']).toMatchObject({ status: 'invalid' });

        const view = buildImportPreviewSignalViewModel({
            parserId: transaction.parserId,
            parserTags: transaction.parserTags,
            hasRecurringMatch: transaction.hasRecurringMatch(),
            recurringTitle: transaction.recurringMatchReasons,
            recurringCandidateCount: transaction.recurringCandidateCount,
            recurringPrimaryReason: transaction.recurringMatchReasons,
            reconciliationType: String(reconciliation?.['candidate_type'] || ''),
            reconciliationStatus: String(reconciliation?.['status'] || ''),
            reconciliationTitle: String(reconciliation?.['signal_label'] || ''),
            reconciliationSourceChain: []
        });

        expect(view.recurring).toMatchObject({ hasMatch: true, candidateCount: 2, primaryReason: 'monthly' });
        expect(view.dedup).toMatchObject({ title: '普通对账提示' });
        expect(SIX_FAMILIES.filter(family => matchesImportPreviewSignalFilter(view, family))).toStrictEqual(['parser']);
    });

    test('maps full and lightweight index fixtures to the same LLM family', () => {
        const transaction = buildImportTransactionFromPreviewRecord({
            id: 44,
            preview_type: '支出',
            preview_amount_cents: 1234,
            preview_source_account_id: 101,
            matching: {
                llm: LLM_FIXTURE
            } as unknown as ImportPreviewRecord['matching']
        }, 0, {
            categoriesById: {},
            timeZone: 'Asia/Shanghai'
        }) as ImportPreviewTransactionDraft;
        const llm = (transaction.matching as unknown as { llm?: typeof LLM_FIXTURE } | undefined)?.llm;
        const fullView = buildImportPreviewSignalViewModel({
            llmStatus: llm?.review_status as ImportPreviewSignalState['llmStatus'],
            llmTitle: llm?.reason,
            llmCategoryPath: [llm?.suggested_main_category, llm?.suggested_sub_category].filter(Boolean).join('/'),
            llmConfidence: llm?.confidence,
            llmSourceAccount: llm?.suggested_source_account,
            llmDestinationAccount: llm?.suggested_destination_account
        });
        const indexItem = mapImportPreviewIndexResponseItem({
            id: 44,
            type: TransactionType.Expense,
            llm_status: 'pending',
            llm_title: LLM_FIXTURE.reason,
            llm_summary: '支出 | 餐饮/咖啡',
            llm_confidence: LLM_FIXTURE.confidence,
            llm_category_path: '餐饮/咖啡',
            llm_source_account: '招商银行',
            llm_destination_account: '支付宝',
            preview_state: previewStateSnapshot(['llm'])
        } as never);
        const indexView = buildImportPreviewIndexSignalViewModel(indexItem);

        expect(SIX_FAMILIES.filter(family => matchesImportPreviewSignalFilter(fullView, family))).toStrictEqual(['llm']);
        expect(SIX_FAMILIES.filter(family => matchesImportPreviewSignalFilter(indexView, family))).toStrictEqual(['llm']);
    });

    test('omits false clear fields and emits explicit true invalidation', () => {
        const draft = makeDraft();
        const unchanged = buildImportPreviewUpdateFromTransaction(draft, {
            categoryPath: null,
            clearLearningDecision: false,
            clearLlmDecision: false,
            includeSuggestionDecisionClears: true
        });

        expect(unchanged).not.toHaveProperty('clear_learning_decision');
        expect(unchanged).not.toHaveProperty('clear_llm_decision');
        expect(unchanged).not.toHaveProperty('clear_actionable_suggestions');

        const changed = buildImportPreviewUpdateFromTransaction(draft, {
            categoryPath: null,
            clearLearningDecision: true,
            clearLlmDecision: true,
            includeSuggestionDecisionClears: true
        });
        expect(changed).toMatchObject({
            clear_learning_decision: true,
            clear_llm_decision: true,
            clear_actionable_suggestions: ['learning', 'llm']
        });
    });

    test.each(['accepted', 'rejected'] as const)('keeps a terminal %s LLM out of pending state', status => {
        const view = buildImportPreviewSignalViewModel({
            llmStatus: status,
            llmCategoryPath: '餐饮/咖啡',
            llmTitle: `用户已${status}`
        });

        expect(view.llm).toMatchObject({ status, actions: [] });
        expect(view.llm?.status).not.toBe('pending');
    });

    describe('residual-3 table-driven signal contracts', () => {
        type SignalTestCase = {
            name: string;
            state: ImportPreviewSignalState;
            expectedLearning: ImportPreviewSignalStatus | null;
            expectedLlm: ImportPreviewSignalStatus | null;
            expectedParser: boolean;
        };

        const TABLE: SignalTestCase[] = [
            {
                name: 'all four learning aliases resolved; precedence: review_status > status > lifecycle_status > signal_state',
                state: {
                    learningStatus: normalizeImportPreviewSignalStatusAlias('accepted', 'rejected', 'skipped', 'pending'),
                },
                expectedLearning: 'accepted',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'learning confidence-only evidence is pending',
                state: {
                    learningConfidence: 0.85,
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'learning score-only evidence is pending',
                state: {
                    learningScore: 75,
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'learning rule-id-only evidence is pending',
                state: {
                    learningRuleId: 42,
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'learning margin-only evidence is pending',
                state: {
                    learningMargin: 0.3,
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'learning accepted_count-only evidence is pending',
                state: {
                    learningAcceptedCount: 5,
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'learning auto_applied_count-only evidence is pending',
                state: {
                    learningAutoAppliedCount: 3,
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'LLM category-id-only evidence is pending',
                state: {
                    llmSuggestedCategoryId: 7,
                },
                expectedLlm: 'pending',
                expectedLearning: null,
                expectedParser: false,
            },
            {
                name: 'learning lifecycle_status alias resolves to accepted',
                state: {
                    learningLifecycleStatus: 'accepted',
                },
                expectedLearning: 'accepted',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'learning signal_state alias resolves to skipped',
                state: {
                    learningSignalState: 'skipped',
                },
                expectedLearning: 'skipped',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'status alias precedence: review_status > signal_state conflict resolved',
                state: {
                    learningStatus: 'pending',
                    learningSignalState: 'accepted',
                },
                expectedLearning: null,
                expectedLlm: null,
                expectedParser: true,
            },
            {
                name: 'suppressed with true hides learning',
                state: {
                    learningTitle: '分类建议',
                    learningSummary: '支出 | 餐饮/咖啡',
                    learningSuppressed: true,
                },
                expectedLearning: null,
                expectedLlm: null,
                expectedParser: true,
            },
            {
                name: 'suppressed with string "2" hides learning',
                state: {
                    learningTitle: '分类建议',
                    learningSummary: '支出 | 餐饮/咖啡',
                    learningSuppressed: '2',
                },
                expectedLearning: null,
                expectedLlm: null,
                expectedParser: true,
            },
            {
                name: 'suppressed with string "0" does not hide (canonical truthy)',
                state: {
                    learningTitle: '分类建议',
                    learningSummary: '支出 | 餐饮/咖啡',
                    learningSuppressed: '0',
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'suppressed with unknown string is false',
                state: {
                    learningTitle: '分类建议',
                    learningSummary: '支出 | 餐饮/咖啡',
                    learningSuppressed: 'unknown',
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'LLM skipped keeps terminal status',
                state: {
                    llmStatus: 'skipped',
                    llmTitle: '已跳过',
                },
                expectedLlm: 'skipped',
                expectedLearning: null,
                expectedParser: false,
            },
            {
                name: 'LLM pending-only without evidence is invisible',
                state: {
                    llmStatus: 'pending',
                },
                expectedLlm: null,
                expectedLearning: null,
                expectedParser: true,
            },
            {
                name: 'index absent (undefined) learning permits evidence inference',
                state: {
                    learningTitle: '分类',
                    learningSummary: '支出',
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'index explicit null learning stops evidence inference',
                state: {
                    learningStatus: null as unknown as ImportPreviewSignalStatus,
                    learningTitle: '分类',
                    learningSummary: '支出',
                    learningStatusAuthoritative: true,
                },
                expectedLearning: null,
                expectedLlm: null,
                expectedParser: true,
            },
            {
                name: 'index absent LLM permits evidence inference',
                state: {
                    llmTitle: 'L',
                    llmCategoryPath: '餐饮/咖啡',
                    llmConfidence: 0.9,
                },
                expectedLlm: 'pending',
                expectedLearning: null,
                expectedParser: false,
            },
            {
                name: 'index explicit null LLM stops evidence inference',
                state: {
                    llmStatus: null as unknown as ImportPreviewSignalStatus,
                    llmTitle: 'L',
                    llmCategoryPath: '餐饮/咖啡',
                    llmConfidence: 0.9,
                    llmStatusAuthoritative: true,
                },
                expectedLlm: null,
                expectedLearning: null,
                expectedParser: true,
            },
            {
                name: 'authoritative pending with text learning stays pending',
                state: {
                    learningStatus: 'pending',
                    learningTitle: '规则匹配',
                    learningStatusAuthoritative: true,
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
            {
                name: 'canonical truthy "yes" string on auto_apply makes learning green',
                state: {
                    learningStatus: 'pending',
                    learningTitle: '分类建议',
                    learningSummary: '支出 | 餐饮/咖啡',
                    learningAutoApplied: 'yes',
                },
                expectedLearning: 'pending',
                expectedLlm: null,
                expectedParser: false,
            },
        ];

        test.each(TABLE)('$name', ({ state, expectedLearning, expectedLlm, expectedParser }) => {
            const view = buildImportPreviewSignalViewModel(state, {
                parserLabels: { alipay: '支付宝' },
                parserColors: { alipay: 'blue' }
            });

            if (expectedParser) {
                const parserView = buildImportPreviewSignalViewModel({
                    ...state,
                    parserId: 'alipay',
                    parserTags: ['parser:alipay'],
                });
                expect(matchesImportPreviewSignalFilter(parserView, 'parser')).toBe(true);
            }

            expect(view.learning ? view.learning.status : null).toBe(expectedLearning);
            expect(view.llm ? view.llm.status : null).toBe(expectedLlm);
        });

        test('canonical truthy helper matches Rust flag semantics', () => {
            expect([
                true,
                'true',
                1,
                '1',
                'yes',
                'y',
                '2',
                '-0.5',
                '9'.repeat(400),
                `.${'0'.repeat(400)}1`,
                '\u30001\u00A0'
            ].map(isImportPreviewCanonicalTruthy)).toStrictEqual(Array(11).fill(true));
            expect([
                false,
                0,
                '0',
                'false',
                'no',
                'n',
                'none',
                'suppressed',
                'null',
                '',
                'unknown',
                '1x',
                {},
                [],
                null,
                undefined,
                Number.NaN,
                Number.POSITIVE_INFINITY
            ].map(isImportPreviewCanonicalTruthy)).toStrictEqual(Array(18).fill(false));
        });
    });
});
