import { describe, expect, test } from '@jest/globals';

import {
    matchesImportCheckAnnotationFilter,
    type ImportCheckAnnotationFilterValue
} from '@/views/desktop/transactions/import/checkDataAnnotation.ts';

function matches(
    filter: ImportCheckAnnotationFilterValue,
    options: {
        hasAnnotationIssues: boolean;
        isManuallyAnnotated: boolean;
        isEditing?: boolean;
    }
): boolean {
    return matchesImportCheckAnnotationFilter(filter, options);
}

describe('checkDataAnnotation helpers', () => {
    test('keeps the currently edited row visible in the needs-review filter after issues are resolved', () => {
        expect(matches('needs-review', {
            hasAnnotationIssues: false,
            isManuallyAnnotated: false,
            isEditing: true
        })).toBe(true);
    });

    test('hides manually annotated rows from needs-review after issues are resolved', () => {
        expect(matches('needs-review', {
            hasAnnotationIssues: false,
            isManuallyAnnotated: true,
            isEditing: false
        })).toBe(false);
    });

    test('hides resolved non-manual rows from the needs-review filter after editing ends', () => {
        expect(matches('needs-review', {
            hasAnnotationIssues: false,
            isManuallyAnnotated: false,
            isEditing: false
        })).toBe(false);
    });

    test('keeps resolved rows visible in no-issues even after manual edits', () => {
        expect(matches('no-issues', {
            hasAnnotationIssues: false,
            isManuallyAnnotated: false,
            isEditing: false
        })).toBe(true);

        expect(matches('no-issues', {
            hasAnnotationIssues: false,
            isManuallyAnnotated: true,
            isEditing: false
        })).toBe(true);

        expect(matches('no-issues', {
            hasAnnotationIssues: false,
            isManuallyAnnotated: true,
            isEditing: true
        })).toBe(true);
    });
});
