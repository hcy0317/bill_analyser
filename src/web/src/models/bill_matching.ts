export type BillMatchingCandidateKind = 'transfer' | 'investment' | 'learning' | string;
export type BillMatchingViewMode = 'linked' | 'candidates' | 'empty';

export interface BillMatchingPairSummary {
    id: number;
    pairType: string;
    source: string;
    leftBillId: number;
    rightBillId: number;
    otherBillId: number;
}

export interface BillMatchingCandidateBillSummary {
    id: number;
    type: string;
    amount: number;
    date: string;
    description: string;
    counterparty: string;
    paymentMethod: string;
}

export interface BillMatchingCandidate {
    candidateId: string;
    kind: BillMatchingCandidateKind;
    score: number;
    level: string;
    reason: string;
    billId: number | null;
    bill: BillMatchingCandidateBillSummary | null;
    ruleId: number | null;
    recommendedType: string;
    summary: string;
    suppressed: boolean;
}

export interface BillMatchingCandidatesResponse {
    billId: number;
    linkedPair: BillMatchingPairSummary | null;
    candidates: BillMatchingCandidate[];
}

export interface BillMatchingFeedbackEvent {
    id: number;
    candidateId: string;
    action: string;
    createdAt: string;
    payload: Record<string, unknown>;
}

export interface BillMatchingFeedbackResponse {
    billId: number;
    events: BillMatchingFeedbackEvent[];
}

export interface BillMatchingViewState {
    mode: BillMatchingViewMode;
    hasLinkedPair: boolean;
    hasCandidates: boolean;
    candidateCount: number;
    primaryCandidate: BillMatchingCandidate | null;
    showCandidateActions: boolean;
    showDeletePairAction: boolean;
}

function toNumber(value: unknown): number {
    if (typeof value === 'number' && Number.isFinite(value)) {
        return value;
    }

    if (typeof value === 'string') {
        const parsedValue = Number(value);
        if (Number.isFinite(parsedValue)) {
            return parsedValue;
        }
    }

    return 0;
}

function toNullableNumber(value: unknown): number | null {
    if (value === null || value === undefined || value === '') {
        return null;
    }

    const parsedValue = toNumber(value);
    return parsedValue > 0 ? parsedValue : null;
}

function toStringValue(value: unknown): string {
    return typeof value === 'string' ? value : '';
}

function toBooleanValue(value: unknown): boolean {
    return value === true;
}

function toRecord(value: unknown): Record<string, unknown> {
    if (value && typeof value === 'object' && !Array.isArray(value)) {
        return value as Record<string, unknown>;
    }

    return {};
}

export function normalizeBillMatchingCandidatesResponse(
    payload: unknown
): BillMatchingCandidatesResponse {
    const response = toRecord(payload);
    const linkedPairRecord = response['linkedPair'] === null ? null : toRecord(response['linkedPair']);
    const rawCandidates = Array.isArray(response['candidates']) ? response['candidates'] : [];

    return {
        billId: toNumber(response['billId']),
        linkedPair: linkedPairRecord ? {
            id: toNumber(linkedPairRecord['id']),
            pairType: toStringValue(linkedPairRecord['pairType']),
            source: toStringValue(linkedPairRecord['source']),
            leftBillId: toNumber(linkedPairRecord['leftBillId']),
            rightBillId: toNumber(linkedPairRecord['rightBillId']),
            otherBillId: toNumber(linkedPairRecord['otherBillId'])
        } : null,
        candidates: rawCandidates.map(item => {
            const candidateRecord = toRecord(item);
            const billRecord = candidateRecord['bill'] ? toRecord(candidateRecord['bill']) : null;

            return {
                candidateId: toStringValue(candidateRecord['candidateId']),
                kind: toStringValue(candidateRecord['kind']),
                score: toNumber(candidateRecord['score']),
                level: toStringValue(candidateRecord['level']),
                reason: toStringValue(candidateRecord['reason']),
                billId: toNullableNumber(candidateRecord['billId']),
                bill: billRecord ? {
                    id: toNumber(billRecord['id']),
                    type: toStringValue(billRecord['type']),
                    amount: toNumber(billRecord['amount']),
                    date: toStringValue(billRecord['date']),
                    description: toStringValue(billRecord['description']),
                    counterparty: toStringValue(billRecord['counterparty']),
                    paymentMethod: toStringValue(billRecord['paymentMethod'])
                } : null,
                ruleId: toNullableNumber(candidateRecord['ruleId']),
                recommendedType: toStringValue(candidateRecord['recommendedType']),
                summary: toStringValue(candidateRecord['summary']),
                suppressed: toBooleanValue(candidateRecord['suppressed'])
            } satisfies BillMatchingCandidate;
        })
    };
}

export function normalizeBillMatchingFeedbackResponse(
    payload: unknown
): BillMatchingFeedbackResponse {
    const response = toRecord(payload);
    const rawEvents = Array.isArray(response['events']) ? response['events'] : [];

    return {
        billId: toNumber(response['billId']),
        events: rawEvents.map(item => {
            const eventRecord = toRecord(item);
            return {
                id: toNumber(eventRecord['id']),
                candidateId: toStringValue(eventRecord['candidateId']),
                action: toStringValue(eventRecord['action']),
                createdAt: toStringValue(eventRecord['createdAt']),
                payload: toRecord(eventRecord['payload'])
            } satisfies BillMatchingFeedbackEvent;
        })
    };
}

export function buildBillMatchingViewState(
    response: BillMatchingCandidatesResponse
): BillMatchingViewState {
    const sortedCandidates = [...response.candidates].sort((left, right) => right.score - left.score);
    const hasLinkedPair = !!response.linkedPair;
    const hasCandidates = sortedCandidates.length > 0;

    if (hasLinkedPair) {
        return {
            mode: 'linked',
            hasLinkedPair: true,
            hasCandidates,
            candidateCount: sortedCandidates.length,
            primaryCandidate: null,
            showCandidateActions: false,
            showDeletePairAction: true
        };
    }

    if (hasCandidates) {
        return {
            mode: 'candidates',
            hasLinkedPair: false,
            hasCandidates: true,
            candidateCount: sortedCandidates.length,
            primaryCandidate: sortedCandidates[0] || null,
            showCandidateActions: true,
            showDeletePairAction: false
        };
    }

    return {
        mode: 'empty',
        hasLinkedPair: false,
        hasCandidates: false,
        candidateCount: 0,
        primaryCandidate: null,
        showCandidateActions: false,
        showDeletePairAction: false
    };
}
