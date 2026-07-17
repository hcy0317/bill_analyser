import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockConfirmOpen = jest.fn<(...args: any[]) => Promise<void>>();
const mockFormatAmount = jest.fn((amountCents: number, currency: string | false) => `${String(currency)}:${amountCents}`);
const capturedTemplateHandlers: Array<(...args: any[]) => unknown> = [];

const mockServices = {
    getMatchingBillCandidates: jest.fn<(...args: any[]) => Promise<any>>(),
    getMatchingBillFeedback: jest.fn<(...args: any[]) => Promise<any>>(),
    acceptMatchingCandidate: jest.fn<(...args: any[]) => Promise<any>>(),
    rejectMatchingCandidate: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteMatchingPair: jest.fn<(...args: any[]) => Promise<any>>(),
};
const mockAccountsStore: any = actualVue.reactive({
    allAccountsMap: {
        '10': { id: '10', name: 'Cash' },
        '20': { id: '20', name: 'Bank' },
        '30': { id: '30', name: 'Cash' },
    },
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        formatAmountToLocalizedNumeralsWithCurrency: mockFormatAmount,
    }),
}));

jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/components/desktop/ConfirmDialog.vue', () => ({
    __esModule: true,
    default: {
        name: 'ConfirmDialogCoverageStub',
        setup: (_props: unknown, { expose }: any) => {
            expose({ open: mockConfirmOpen });
            return () => actualVue.h('div', { 'data-stub': 'ConfirmDialogCoverageStub' });
        },
    },
}));

const BillMatchingPanel = require('@/views/desktop/transactions/list/dialogs/BillMatchingPanel.vue').default as any;

function createBill(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        id: 200,
        type: 'Expense',
        amountCents: 12_345,
        destinationAmountCents: 67_890,
        date: '2026-07-15',
        description: 'Coffee shop',
        counterparty: 'Cafe',
        paymentMethod: 'Card',
        mainCategory: 'Food',
        subCategory: 'Cafe',
        sourceAccountId: 10,
        destinationAccountId: 20,
        ...overrides,
    };
}

function createCandidate(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        candidateId: 'candidate-1',
        kind: 'transfer',
        score: 0.95,
        level: 'high',
        reason: 'same amount',
        billId: 200,
        bill: createBill(),
        ruleId: null,
        recommendedType: 'Transfer',
        summary: 'candidate summary',
        suppressed: false,
        ...overrides,
    };
}

function createPair(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        id: 77,
        pairType: 'transfer',
        source: 'manual',
        leftBillId: 100,
        rightBillId: 200,
        otherBillId: 200,
        ...overrides,
    };
}

function candidateResponse(overrides: Record<string, unknown> = {}): any {
    return {
        data: {
            result: {
                billId: 100,
                linkedPair: null,
                candidates: [createCandidate()],
                reconciliation: {},
                ...overrides,
            },
        },
    };
}

function feedbackResponse(overrides: Record<string, unknown> = {}): any {
    return {
        data: {
            result: {
                billId: 100,
                events: [],
                ...overrides,
            },
        },
    };
}

function setupPanel(
    props: Record<string, unknown> = {},
    reactiveProps = false,
): { bindings: any; emitted: Array<[string, ...unknown[]]>; props: any } {
    const baseProps = { billId: 0, disabled: false, ...props };
    const runtimeProps = reactiveProps ? actualVue.reactive(baseProps) : baseProps;
    const emitted: Array<[string, ...unknown[]]> = [];
    const bindings = BillMatchingPanel.setup(runtimeProps, {
        emit: (event: string, ...args: unknown[]) => emitted.push([event, ...args]),
        expose: jest.fn(),
    });
    return { bindings, emitted, props: runtimeProps };
}

async function flushAsync(): Promise<void> {
    await Promise.resolve();
    await new Promise(resolve => setImmediate(resolve));
    await actualVue.nextTick();
}

function createCaptureComponent(name: string): Record<string, unknown> {
    return {
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [attribute, value] of Object.entries(attrs)) {
                if (attribute.startsWith('on') && typeof value === 'function') {
                    capturedTemplateHandlers.push(value as (...args: any[]) => unknown);
                }
            }
            return () => actualVue.h('div', { 'data-stub': name, ...attrs }, Object.entries(slots).flatMap(([slotName, slot]) => {
                if (typeof slot !== 'function') return [];
                try {
                    return (slot as (props?: any) => unknown[])({
                        name: slotName,
                        props: { role: 'button' },
                        item: createCandidate(),
                    });
                } catch {
                    return [];
                }
            }));
        },
    };
}

async function renderPanel(mutate: (bindings: any) => void): Promise<string> {
    const { createSSRApp, defineComponent } = actualVue;
    const { renderToString } = require('vue/server-renderer') as any;
    const RuntimePanel = {
        ...BillMatchingPanel,
        setup(runtimeProps: any, context: any) {
            const bindings = BillMatchingPanel.setup(runtimeProps, context);
            mutate(bindings);
            return bindings;
        },
    };
    const app = createSSRApp(RuntimePanel, { billId: 0, disabled: false });
    const UiStub = defineComponent(createCaptureComponent('VuetifyStub'));
    for (const name of [
        'v-card', 'v-progress-circular', 'v-btn', 'v-list', 'v-list-item',
        'v-chip', 'v-divider',
    ]) app.component(name, UiStub);
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

async function invokeTemplateHandlers(): Promise<void> {
    for (const handler of [...capturedTemplateHandlers]) {
        try {
            await handler({ preventDefault: jest.fn(), stopPropagation: jest.fn() });
        } catch {
            // Generated wrappers close over different candidate/action payloads.
        }
    }
    await flushAsync();
}

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void; reject: (reason: unknown) => void } {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

beforeEach(() => {
    jest.clearAllMocks();
    capturedTemplateHandlers.length = 0;
    mockAccountsStore.allAccountsMap = {
        '10': { id: '10', name: 'Cash' },
        '20': { id: '20', name: 'Bank' },
        '30': { id: '30', name: 'Cash' },
    };
    mockServices.getMatchingBillCandidates.mockResolvedValue(candidateResponse());
    mockServices.getMatchingBillFeedback.mockResolvedValue(feedbackResponse());
    mockServices.acceptMatchingCandidate.mockResolvedValue({ data: { result: { bill: { id: 100 } } } });
    mockServices.rejectMatchingCandidate.mockResolvedValue({ data: { result: {} } });
    mockServices.deleteMatchingPair.mockResolvedValue({ data: { result: true } });
    mockConfirmOpen.mockResolvedValue(undefined);
});

describe('BillMatchingPanel production-loaded view state and labels', () => {
    test('normalizes valid and invalid bill ids and resets invalid input immediately', async () => {
        const invalid = setupPanel({ billId: 'not-a-number' }).bindings;
        await flushAsync();
        expect(invalid.normalizedBillId.value).toBe(0);
        expect(invalid.candidatesResponse.value).toMatchObject({ billId: 0, linkedPair: null, candidates: [] });
        expect(invalid.feedbackResponse.value).toMatchObject({ billId: 0, events: [] });
        expect(mockServices.getMatchingBillCandidates).not.toHaveBeenCalled();

        const negative = setupPanel({ billId: -1 }).bindings;
        expect(negative.normalizedBillId.value).toBe(0);
        const valid = setupPanel({ billId: '42' }).bindings;
        expect(valid.normalizedBillId.value).toBe(42);
        await flushAsync();
        expect(mockServices.getMatchingBillCandidates).toHaveBeenCalledWith({ billId: 42 });
    });

    test('sorts candidates and derives candidate, reconciliation, empty, and busy states', () => {
        const { bindings, props } = setupPanel({ billId: 0 }, true);
        bindings.candidatesResponse.value = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
            billId: 1,
            linkedPair: null,
            candidates: [
                createCandidate({ candidateId: 'low', score: 0.25 }),
                createCandidate({ candidateId: 'high', score: 0.95, kind: 'duplicate' }),
            ],
        });
        expect(bindings.sortedCandidates.value.map((item: any) => item.candidateId)).toStrictEqual(['high', 'low']);
        expect(bindings.headlineText.value).toBe('Matching Candidates 2');
        expect(bindings.sublineText.value).toBe('Best Candidate: Duplicate · Match Score 0.95');
        expect(bindings.isBusy.value).toBe(false);
        props.disabled = true;
        expect(bindings.isBusy.value).toBe(true);
        props.disabled = false;
        bindings.loading.value = true;
        expect(bindings.isBusy.value).toBe(true);
        bindings.loading.value = false;
        bindings.actionCandidateId.value = 'candidate';
        expect(bindings.isBusy.value).toBe(true);
        bindings.actionCandidateId.value = '';
        bindings.deletingPairId.value = 'pair';
        expect(bindings.isBusy.value).toBe(true);
        bindings.deletingPairId.value = '';

        bindings.candidatesResponse.value = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
            billId: 1,
            linkedPair: null,
            candidates: [],
            reconciliation: { signal_label: 'Needs reconciliation' },
        });
        expect(bindings.reconciliationSignal.value).toBe('Needs reconciliation');
        expect(bindings.headlineText.value).toBe('Needs reconciliation');
        expect(bindings.sublineText.value).toBe('Needs reconciliation');
        bindings.candidatesResponse.value.reconciliation = { signal_label: 123 };
        expect(bindings.reconciliationSignal.value).toBe('');
        expect(bindings.headlineText.value).toBe('None');
        expect(bindings.sublineText.value).toBe('');
    });

    test('derives linked-pair headings for canonical and unknown pair kinds', () => {
        const { bindings } = setupPanel();
        for (const [pairType, label] of [
            ['transfer', 'Transfer'],
            ['investment', 'Investment'],
            ['duplicate', 'Duplicate'],
            ['custom', 'custom'],
        ]) {
            bindings.candidatesResponse.value = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
                billId: 1,
                linkedPair: createPair({ pairType }),
                candidates: [],
            });
            expect(bindings.headlineText.value).toBe(`Linked Pair · ${label} · #200`);
            expect(bindings.sublineText.value).toBe('Pair Source: manual');
            expect(bindings.linkedPair.value?.id).toBe(77);
        }
    });

    test('maps candidate labels, colors, score fallbacks, top lines, and remarks', () => {
        const { bindings } = setupPanel();
        const expected = [
            ['duplicate', 'Duplicate', 'secondary'],
            ['reconciliation_duplicate', 'Duplicate', 'secondary'],
            ['reconciliation_transfer', 'Transfer', 'primary'],
            ['transfer', 'Transfer', 'primary'],
            ['investment', 'Investment', 'success'],
            ['learning', 'Learning Suggestion', 'info'],
            ['custom', 'custom', 'default'],
        ];
        for (const [kind, label, color] of expected) {
            expect(bindings.getCandidateKindLabel(kind)).toBe(label);
            expect(bindings.getCandidateChipColor(kind)).toBe(color);
        }
        expect(bindings.formatScore(0.956)).toBe('0.96');
        expect(bindings.formatScore(0)).toBe('0.00');
        expect(bindings.formatScore(Number.NaN)).toBe('0.00');

        const detailed = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
            candidates: [createCandidate()],
        }).candidates[0];
        expect(bindings.getCandidateTopLineParts(detailed)).toStrictEqual(['2026-07-15', 'Expense', 'Food / Cafe']);
        expect(bindings.getCandidateRemark(detailed)).toBe('Coffee shop');

        const fallback = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
            candidates: [createCandidate({ bill: null, kind: 'learning', recommendedType: 'Expense', summary: 'summary', reason: 'reason' })],
        }).candidates[0];
        expect(bindings.getCandidateTopLineParts(fallback)).toStrictEqual(['Learning Suggestion', 'Expense']);
        expect(bindings.getCandidateRemark(fallback)).toBe('summary · reason');
    });
});

describe('BillMatchingPanel production-loaded cents, accounts, and feedback formatting', () => {
    test('formats candidate bill amounts as unchanged integer cents and adds account text', () => {
        const { bindings } = setupPanel();
        const candidate = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
            candidates: [createCandidate()],
        }).candidates[0];
        expect(bindings.getCandidateSecondLineParts(candidate)).toStrictEqual(['false:12345', 'Cash -> Bank']);
        expect(mockFormatAmount).toHaveBeenLastCalledWith(12_345, false);
        expect(bindings.getCandidateBillAccountText(candidate)).toBe('Cash -> Bank');

        candidate.bill.destinationAccountId = 30;
        expect(bindings.getCandidateBillAccountText(candidate)).toBe('Cash');
        candidate.bill.sourceAccountId = 0;
        expect(bindings.getCandidateBillAccountText(candidate)).toBe('Cash');
        candidate.bill.destinationAccountId = 0;
        expect(bindings.getCandidateBillAccountText(candidate)).toBe('');
        candidate.bill = null;
        expect(bindings.getCandidateBillAccountText(candidate)).toBe('');
    });

    test('uses summary only when neither cents nor account information is available', () => {
        const { bindings } = setupPanel();
        const summaryOnly = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
            candidates: [createCandidate({
                bill: createBill({ amountCents: 1.5, sourceAccountId: 0, destinationAccountId: 0 }),
                summary: 'fallback summary',
            })],
        }).candidates[0];
        expect(bindings.getCandidateSecondLineParts(summaryOnly)).toStrictEqual(['false:0']);

        summaryOnly.bill = null;
        expect(bindings.getCandidateSecondLineParts(summaryOnly)).toStrictEqual(['fallback summary']);
        summaryOnly.summary = '';
        expect(bindings.getCandidateSecondLineParts(summaryOnly)).toStrictEqual([]);
    });

    test('formats feedback action colors, pair payload variants, titles, and subtitles', () => {
        const { bindings } = setupPanel();
        expect(bindings.getFeedbackActionColor('accept')).toBe('success');
        expect(bindings.getFeedbackActionColor('reject')).toBe('warning');
        expect(bindings.getFeedbackActionColor('ignore')).toBe('default');

        const event: any = { id: 1, candidateId: 'candidate-1', action: 'accept', createdAt: 'now', payload: { pair: { pairType: 'transfer' } } };
        expect(bindings.getFeedbackPairType(event)).toBe('transfer');
        expect(bindings.getFeedbackTitle(event)).toBe('Transfer · accept');
        expect(bindings.getFeedbackSubtitle(event)).toBe('candidate-1');
        event.payload = { pair: { pair_type: 'duplicate' } };
        expect(bindings.getFeedbackPairType(event)).toBe('duplicate');
        expect(bindings.getFeedbackTitle(event)).toBe('Duplicate · accept');
        for (const pair of [null, [], { pairType: 1 }, { pairType: '' }]) {
            event.payload = { pair } as any;
            expect(bindings.getFeedbackPairType(event)).toBe('');
            expect(bindings.getFeedbackTitle(event)).toBe('accept');
        }
    });
});

describe('BillMatchingPanel production-loaded refresh and action orchestration', () => {
    test('loads and normalizes candidates and feedback for a valid bill', async () => {
        const { bindings } = setupPanel({ billId: 100 });
        expect(bindings.loading.value).toBe(true);
        await flushAsync();
        expect(bindings.loading.value).toBe(false);
        expect(bindings.candidatesResponse.value.candidates).toHaveLength(1);
        expect(bindings.feedbackResponse.value.events).toStrictEqual([]);
        expect(mockServices.getMatchingBillCandidates).toHaveBeenCalledWith({ billId: 100 });
        expect(mockServices.getMatchingBillFeedback).toHaveBeenCalledWith({ billId: 100 });
    });

    test('reports candidate and feedback failures using every supported error shape', async () => {
        mockServices.getMatchingBillCandidates.mockRejectedValueOnce(new Error('candidate unavailable'));
        mockServices.getMatchingBillFeedback.mockRejectedValueOnce('feedback unavailable');
        const first = setupPanel({ billId: 100 });
        await flushAsync();
        expect(first.emitted).toContainEqual(['error', 'candidate unavailable']);
        expect(first.emitted).toContainEqual(['error', 'feedback unavailable']);
        expect(first.bindings.loading.value).toBe(false);

        mockServices.getMatchingBillCandidates.mockRejectedValueOnce({ response: { data: { error: 'response failure' } } });
        mockServices.getMatchingBillFeedback.mockRejectedValueOnce({ response: { data: { error: 500 } } });
        const second = setupPanel({ billId: 101 });
        await flushAsync();
        expect(second.emitted).toContainEqual(['error', 'response failure']);
        expect(second.emitted).toContainEqual(['error', 'Load Historical Matching Failed']);

        expect(second.bindings.getErrorMessage(new Error(''), 'fallback')).toBe('fallback');
        expect(second.bindings.getErrorMessage('', 'fallback')).toBe('fallback');
        expect(second.bindings.getErrorMessage(null, 'fallback')).toBe('fallback');
    });

    test('ignores stale refresh results after a newer request completes', async () => {
        const firstCandidate = deferred<any>();
        const firstFeedback = deferred<any>();
        const secondCandidate = deferred<any>();
        const secondFeedback = deferred<any>();
        mockServices.getMatchingBillCandidates
            .mockImplementationOnce(() => firstCandidate.promise)
            .mockImplementationOnce(() => secondCandidate.promise);
        mockServices.getMatchingBillFeedback
            .mockImplementationOnce(() => firstFeedback.promise)
            .mockImplementationOnce(() => secondFeedback.promise);

        const { bindings, props } = setupPanel({ billId: 0 }, true);
        props.billId = 100;
        await actualVue.nextTick();
        const newerRefresh = bindings.refreshMatchingData();
        secondCandidate.resolve(candidateResponse({ billId: 100, candidates: [createCandidate({ candidateId: 'newer' })] }));
        secondFeedback.resolve(feedbackResponse({ billId: 100 }));
        await newerRefresh;
        expect(bindings.candidatesResponse.value.candidates[0].candidateId).toBe('newer');

        firstCandidate.resolve(candidateResponse({ billId: 100, candidates: [createCandidate({ candidateId: 'stale' })] }));
        firstFeedback.resolve(feedbackResponse({ billId: 100 }));
        await flushAsync();
        expect(bindings.candidatesResponse.value.candidates[0].candidateId).toBe('newer');
    });

    test('accepts and rejects candidates, emits updates conditionally, and clears busy state', async () => {
        const { bindings, emitted } = setupPanel({ billId: 0 });
        const candidate = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
            candidates: [createCandidate()],
        }).candidates[0];
        const emptyId = { ...candidate, candidateId: '' };
        await bindings.handleCandidateAction('accept', emptyId);
        expect(mockServices.acceptMatchingCandidate).not.toHaveBeenCalled();

        await bindings.handleCandidateAction('accept', candidate);
        expect(mockServices.acceptMatchingCandidate).toHaveBeenCalledWith({ candidateId: 'candidate-1' });
        expect(emitted).toContainEqual(['notify', 'Matching Candidate Accepted']);
        expect(emitted).toContainEqual(['updated']);
        expect(bindings.actionCandidateId.value).toBe('');

        mockServices.acceptMatchingCandidate.mockResolvedValueOnce({ data: { result: { bill: null } } });
        emitted.length = 0;
        await bindings.handleCandidateAction('accept', candidate);
        expect(emitted).not.toContainEqual(['updated']);

        await bindings.handleCandidateAction('reject', candidate);
        expect(mockServices.rejectMatchingCandidate).toHaveBeenCalledWith({ candidateId: 'candidate-1' });
        expect(emitted).toContainEqual(['notify', 'Matching Candidate Rejected']);
    });

    test('reports candidate action errors and always clears the candidate id', async () => {
        const { bindings, emitted } = setupPanel();
        const candidate = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({ candidates: [createCandidate()] }).candidates[0];
        mockServices.acceptMatchingCandidate.mockRejectedValueOnce({ response: { data: { error: 'accept failed' } } });
        await bindings.handleCandidateAction('accept', candidate);
        expect(emitted).toContainEqual(['error', 'accept failed']);
        expect(bindings.actionCandidateId.value).toBe('');
    });

    test('clears linked pairs, handles failures, and confirms or cancels deletion', async () => {
        const { bindings, emitted } = setupPanel();
        await bindings.deletePair();
        expect(mockServices.deleteMatchingPair).not.toHaveBeenCalled();

        bindings.candidatesResponse.value = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
            billId: 1,
            linkedPair: createPair(),
            candidates: [],
        });
        await bindings.deletePair();
        expect(mockServices.deleteMatchingPair).toHaveBeenCalledWith({ pairId: 77 });
        expect(emitted).toContainEqual(['notify', 'Matching Pair Cleared']);
        expect(bindings.deletingPairId.value).toBe('');

        bindings.candidatesResponse.value = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
            billId: 1,
            linkedPair: createPair(),
            candidates: [],
        });
        mockServices.deleteMatchingPair.mockRejectedValueOnce(new Error('delete failed'));
        await bindings.deletePair();
        expect(emitted).toContainEqual(['error', 'delete failed']);
        expect(bindings.deletingPairId.value).toBe('');

        bindings.confirmDialog.value = { open: mockConfirmOpen };
        mockConfirmOpen.mockResolvedValueOnce(undefined);
        bindings.confirmDeletePair();
        await flushAsync();
        expect(mockConfirmOpen).toHaveBeenCalledWith('Clear Pair Confirmation');
        mockConfirmOpen.mockRejectedValueOnce(new Error('cancelled'));
        bindings.confirmDeletePair();
        await flushAsync();
        bindings.confirmDialog.value = null;
        bindings.confirmDeletePair();
    });
});

describe('BillMatchingPanel production-loaded template', () => {
    test('renders linked pair, candidates, feedback, levels, and executes template handlers', async () => {
        const html = await renderPanel(bindings => {
            bindings.candidatesResponse.value = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
                billId: 1,
                linkedPair: createPair(),
                candidates: [
                    createCandidate(),
                    createCandidate({ candidateId: 'unsupported', kind: 'custom', level: '', bill: null }),
                ],
            });
            bindings.feedbackResponse.value = require('@/models/bill_matching.ts').normalizeBillMatchingFeedbackResponse({
                billId: 1,
                events: [
                    { id: 1, candidateId: 'candidate-1', action: 'accept', createdAt: 'now', payload: { pair: { pairType: 'transfer' } } },
                    { id: 2, candidateId: '', action: 'reject', createdAt: '', payload: {} },
                ],
            });
            bindings.loading.value = true;
            bindings.actionCandidateId.value = 'candidate-1';
        });
        expect(html).toContain('Historical Matching');
        expect(html).toContain('Linked Pair');
        expect(html).toContain('Coffee shop');
        expect(html).toContain('Matching Feedback');
        expect(capturedTemplateHandlers.length).toBeGreaterThan(0);
        await invokeTemplateHandlers();
    });

    test('renders reconciliation and empty candidates without feedback', async () => {
        const reconciliationHtml = await renderPanel(bindings => {
            bindings.candidatesResponse.value = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
                billId: 1,
                linkedPair: null,
                candidates: [],
                reconciliation: { signal_label: 'Reconciliation signal' },
            });
        });
        expect(reconciliationHtml).toContain('Reconciliation signal');

        const emptyHtml = await renderPanel(bindings => {
            bindings.candidatesResponse.value = require('@/models/bill_matching.ts').normalizeBillMatchingCandidatesResponse({
                billId: 1,
                linkedPair: null,
                candidates: [],
            });
        });
        expect(emptyHtml).toContain('None');
    });
});
