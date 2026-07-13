import { buildCanonicalPreviewPageRequestKey } from '../../../../../../src/web/src/views/desktop/transactions/import/import-dialog/previewPageQuery';
import { PreviewPageRequestCoordinator } from '../../../../../../src/web/src/views/desktop/transactions/import/import-dialog/previewPageRequestCoordinator';

describe('preview canonical request duplicate regression', () => {
    test('empty header filters and omitted menu filters produce one request', () => {
        const headerKey = buildCanonicalPreviewPageRequestKey(1, 10, 'time', 'asc', {
            category: '',
            account: '   ',
            signal: 'learning'
        });
        const menuKey = buildCanonicalPreviewPageRequestKey(1, 10, 'time', 'asc', {
            signal: 'learning'
        });
        const coordinator = new PreviewPageRequestCoordinator();
        const transport = jest.fn();

        const header = coordinator.begin(headerKey);
        if (header) transport(header.key);
        const menu = coordinator.begin(menuKey);
        if (menu) transport(menu.key);

        expect(headerKey).toBe(menuKey);
        expect(transport).toHaveBeenCalledTimes(1);
    });
});
