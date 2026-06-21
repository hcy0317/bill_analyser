export interface RecurringCandidateItem {
    id: string | number;
    name: string;
    matchScore?: number;
    matchedOccurrenceDate?: string;
    matchReasons?: string[];
}

type Translate = (key: string) => string;

export function createRecurringCandidateSubtitleFormatter(tt: Translate): (candidate: RecurringCandidateItem) => string {
    return (candidate: RecurringCandidateItem): string => {
        const reasons = Array.isArray(candidate.matchReasons)
            ? candidate.matchReasons.join(' | ')
            : '';

        return [
            candidate.matchedOccurrenceDate ? `${tt('Matched Date')}: ${candidate.matchedOccurrenceDate}` : '',
            reasons ? `${tt('Match Reasons')}: ${reasons}` : ''
        ].filter(text => !!text).join(' · ');
    };
}

export function getRecurringCandidatePrimaryReason(candidate: RecurringCandidateItem): string {
    if (!Array.isArray(candidate.matchReasons) || candidate.matchReasons.length < 1) {
        return '';
    }

    return String(candidate.matchReasons[0] || '');
}
