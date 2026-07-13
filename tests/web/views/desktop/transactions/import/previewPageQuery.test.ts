import {
    appendPreviewPageFilters,
    buildCanonicalPreviewPageRequestKey,
    normalizePreviewPageFilters,
    normalizePreviewPageSortBy,
    normalizePreviewPageSortDirection
} from '../../../../../../src/web/src/views/desktop/transactions/import/import-dialog/previewPageQuery';

describe('preview page canonical request', () => {
    test('normalizes equivalent header and menu requests to one key', () => {
        const header = buildCanonicalPreviewPageRequestKey(1, 10, '', 'desc', {
            signal: 'learning',
            category: '42'
        });
        const menu = buildCanonicalPreviewPageRequestKey(1, 10, null, null, {
            category: '42',
            signal: 'learning'
        });
        expect(header).toBe(menu);
    });

    test('keeps meaningful page, sort and filter changes distinct', () => {
        const baseline = buildCanonicalPreviewPageRequestKey(1, 10, 'time', 'asc', { signal: 'learning' });
        expect(buildCanonicalPreviewPageRequestKey(2, 10, 'time', 'asc', { signal: 'learning' })).not.toBe(baseline);
        expect(buildCanonicalPreviewPageRequestKey(1, 10, 'time', 'desc', { signal: 'learning' })).not.toBe(baseline);
        expect(buildCanonicalPreviewPageRequestKey(1, 10, 'time', 'asc', { signal: 'transfer' })).not.toBe(baseline);
    });

    test('uses the same sort normalization as the transport', () => {
        expect(normalizePreviewPageSortBy('unknown')).toBe('');
        expect(normalizePreviewPageSortBy(undefined)).toBe('');
        expect(normalizePreviewPageSortDirection('DESC')).toBe('desc');
        expect(normalizePreviewPageSortDirection(undefined)).toBe('asc');
    });

    test('trims meaningful filters and omits empty filter drafts', () => {
        expect(buildCanonicalPreviewPageRequestKey(1, 10, 'time', 'asc', {
            category: ' 42 ',
            account: '   ',
            signal: ' learning '
        })).toBe(buildCanonicalPreviewPageRequestKey(1, 10, 'time', 'asc', {
            category: '42',
            signal: 'learning'
        }));
    });

    test('normalizes absent and non-string filter drafts without emitting query parameters', () => {
        expect(normalizePreviewPageFilters(undefined)).toEqual({});
        expect(normalizePreviewPageFilters({ category: 42 } as never)).toEqual({});

        const searchParams = new URLSearchParams();
        appendPreviewPageFilters(searchParams, undefined);
        expect(searchParams.toString()).toBe('');
    });

    test('normalizes zero page inputs to the first canonical page', () => {
        expect(buildCanonicalPreviewPageRequestKey(0, 0, null, null, undefined))
            .toBe('page=1&page_size=10');
    });
});
