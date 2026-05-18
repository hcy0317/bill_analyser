export type ImportCheckAnnotationFilterValue = 'needs-review' | 'no-issues' | null;

export interface ImportCheckAnnotationState {
    hasAnnotationIssues: boolean;
    isManuallyAnnotated: boolean;
    isEditing?: boolean;
}

export function matchesImportCheckAnnotationFilter(
    filter: ImportCheckAnnotationFilterValue,
    state: ImportCheckAnnotationState
): boolean {
    if (state.isEditing) {
        return true;
    }

    if (filter === 'needs-review') {
        return state.hasAnnotationIssues;
    }

    if (filter === 'no-issues') {
        return !state.hasAnnotationIssues;
    }

    return true;
}
