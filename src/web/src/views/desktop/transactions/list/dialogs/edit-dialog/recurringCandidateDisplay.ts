export interface RecurringCandidateItem {
    id: string | number;
    name: string;
    matchScore?: number;
    matchedOccurrenceDate?: string;
    matchReasons?: string[];
}

type Translate = (key: string) => string;

/** 创建周期候选副标题格式化器，集中维护日期和原因展示文案。 */
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

/** 取周期候选的首个主要原因，作为按钮和紧凑列表中的摘要。 */
export function getRecurringCandidatePrimaryReason(candidate: RecurringCandidateItem): string {
    if (!Array.isArray(candidate.matchReasons) || candidate.matchReasons.length < 1) {
        return '';
    }

    return String(candidate.matchReasons[0] || '');
}
