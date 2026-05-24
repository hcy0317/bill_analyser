export type BillMatchingCandidateKind =
    'transfer'
    | 'investment'
    | 'learning'
    | 'duplicate'
    | 'reconciliation_transfer'
    | 'reconciliation_duplicate'
    | string;
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
    mainCategory: string;
    subCategory: string;
    sourceAccountId: number;
    destinationAccountId: number;
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
    reconciliation?: Record<string, unknown>;
}

export interface BillMatchingCandidatesResponse {
    billId: number;
    linkedPair: BillMatchingPairSummary | null;
    candidates: BillMatchingCandidate[];
    reconciliation?: Record<string, unknown>;
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

function toStringOrNumber(value: unknown): string {
    if (typeof value === 'string') {
        return value;
    }

    if (typeof value === 'number' && Number.isFinite(value)) {
        return String(value);
    }

    return '';
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
        reconciliation: toRecord(response['reconciliation']),
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
                    type: toStringOrNumber(billRecord['type']),
                    amount: toNumber(billRecord['amount']),
                    date: toStringValue(billRecord['date']),
                    description: toStringValue(billRecord['description']),
                    counterparty: toStringValue(billRecord['counterparty']),
                    paymentMethod: toStringValue(billRecord['paymentMethod'] ?? billRecord['payment_method']),
                    mainCategory: toStringValue(billRecord['mainCategory'] ?? billRecord['main_category']),
                    subCategory: toStringValue(billRecord['subCategory'] ?? billRecord['sub_category']),
                    sourceAccountId: toNumber(billRecord['sourceAccountId'] ?? billRecord['source_account_id']),
                    destinationAccountId: toNumber(billRecord['destinationAccountId'] ?? billRecord['destination_account_id'])
                } : null,
                ruleId: toNullableNumber(candidateRecord['ruleId']),
                recommendedType: toStringValue(candidateRecord['recommendedType']),
                summary: toStringValue(candidateRecord['summary']),
                suppressed: toBooleanValue(candidateRecord['suppressed']),
                reconciliation: toRecord(candidateRecord['reconciliation'])
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

export function getBillMatchingCandidateBillCategoryLabel(
    bill: BillMatchingCandidateBillSummary | null
): string {
    if (!bill) {
        return '';
    }

    if (bill.mainCategory && bill.subCategory) {
        return `${bill.mainCategory} / ${bill.subCategory}`;
    }

    return bill.subCategory || bill.mainCategory;
}

export function getBillMatchingCandidateBillTitle(candidate: BillMatchingCandidate): string {
    const bill = candidate.bill;

    if (!bill) {
        return '';
    }

    if (bill.description) {
        return bill.description;
    }

    if (bill.counterparty) {
        return bill.counterparty;
    }

    return bill.id > 0 ? `#${bill.id}` : '';
}

export function getBillMatchingCandidateBillSubtitleParts(candidate: BillMatchingCandidate): string[] {
    const bill = candidate.bill;

    if (!bill) {
        return [];
    }

    const parts = [
        bill.date,
        bill.type,
        getBillMatchingCandidateBillCategoryLabel(bill),
        bill.counterparty
    ];
    const seen: Record<string, boolean> = {};

    return parts.filter(part => {
        const value = part.trim();

        if (!value || seen[value]) {
            return false;
        }

        seen[value] = true;
        return true;
    });
}

export function getBillMatchingCandidateBillAmountCents(candidate: BillMatchingCandidate): number | null {
    const amount = candidate.bill?.amount;

    if (typeof amount !== 'number' || !Number.isFinite(amount)) {
        return null;
    }

    return Math.round(amount * 100);
}

export function isBillMatchingCandidateReviewActionSupported(candidate: BillMatchingCandidate): boolean {
    return candidate.kind.trim().toLowerCase() !== 'duplicate';
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
            showCandidateActions: sortedCandidates.some(isBillMatchingCandidateReviewActionSupported),
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

// --- Pair list & reconcile-history types ---

export interface BillMatchingPairBillSummary {
    id: number;
    type: string;
    amount: number;
    date: string;
    description: string;
    counterparty: string;
    mainCategory: string;
    subCategory: string;
    sourceAccountName: string;
}

export interface BillMatchingPairDetail {
    id: number;
    pairType: string;
    source: string;
    leftBillId: number;
    rightBillId: number;
    createdAt: string;
    updatedAt: string;
    leftBill: BillMatchingPairBillSummary | null;
    rightBill: BillMatchingPairBillSummary | null;
}

export interface MatchingPairsResponse {
    pairs: BillMatchingPairDetail[];
}

export interface ReconcileHistorySummary {
    billCount: number;
    candidateCount: number;
    linkedPairCount: number;
}

export interface ReconcileHistoryBillResult {
    billId: number;
    linkedPair: BillMatchingPairSummary | null;
    candidates: BillMatchingCandidate[];
}

export interface ReconcileHistoryResponse {
    summary: ReconcileHistorySummary;
    results: ReconcileHistoryBillResult[];
}

export function normalizeMatchingPairsResponse(
    payload: unknown
): MatchingPairsResponse {
    const response = toRecord(payload);
    const rawPairs = Array.isArray(response['pairs']) ? response['pairs'] : [];

    return {
        pairs: rawPairs.map(item => {
            const pairRecord = toRecord(item);
            const leftBillRecord = pairRecord['leftBill'] ? toRecord(pairRecord['leftBill']) : null;
            const rightBillRecord = pairRecord['rightBill'] ? toRecord(pairRecord['rightBill']) : null;

            function toBillSummary(record: Record<string, unknown>): BillMatchingPairBillSummary {
                return {
                    id: toNumber(record['id']),
                    type: toStringValue(record['type']),
                    amount: toNumber(record['amount']),
                    date: toStringValue(record['date']),
                    description: toStringValue(record['description']),
                    counterparty: toStringValue(record['counterparty']),
                    mainCategory: toStringValue(record['mainCategory']),
                    subCategory: toStringValue(record['subCategory']),
                    sourceAccountName: toStringValue(record['sourceAccountName'])
                };
            }

            return {
                id: toNumber(pairRecord['id']),
                pairType: toStringValue(pairRecord['pairType']),
                source: toStringValue(pairRecord['source']),
                leftBillId: toNumber(pairRecord['leftBillId']),
                rightBillId: toNumber(pairRecord['rightBillId']),
                createdAt: toStringValue(pairRecord['createdAt']),
                updatedAt: toStringValue(pairRecord['updatedAt']),
                leftBill: leftBillRecord ? toBillSummary(leftBillRecord) : null,
                rightBill: rightBillRecord ? toBillSummary(rightBillRecord) : null
            } satisfies BillMatchingPairDetail;
        })
    };
}
