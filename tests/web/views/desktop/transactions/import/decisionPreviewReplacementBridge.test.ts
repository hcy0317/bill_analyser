import { describe, expect, jest, test } from '@jest/globals';

import DecisionPreviewReplacementBridge from '@/views/desktop/transactions/import/DecisionPreviewReplacementBridge';

describe('decision preview replacement component wiring', () => {
    test.each([false, true])('slot bridge reaches %s parent mode with removed and upserted rows', serverPaged => {
        const refreshPageAndMetadata = jest.fn();
        const replaceLocalRows = jest.fn();
        const onReclassified = (items: Array<{ id: number }>, removed: number[] = []): void => {
            if (serverPaged) refreshPageAndMetadata({ page: 1, metadata: 'refreshed' });
            else replaceLocalRows({ items, removed });
        };
        const setup = DecisionPreviewReplacementBridge.setup;
        if (!setup) throw new Error('missing setup');
        const render = setup(
            { onReclassified },
            {
                slots: {
                    default: ({ handleReclassified }: { handleReclassified: typeof onReclassified }) => {
                        handleReclassified([{ id: 51 }, { id: 52 }], [41]);
                        return [];
                    }
                },
                attrs: {},
                emit: jest.fn(),
                expose: jest.fn()
            }
        );
        if (typeof render !== 'function') throw new Error('missing render');
        render();
        if (serverPaged) {
            expect(refreshPageAndMetadata).toHaveBeenCalledWith({ page: 1, metadata: 'refreshed' });
            expect(replaceLocalRows).not.toHaveBeenCalled();
        } else {
            expect(replaceLocalRows).toHaveBeenCalledWith({ items: [{ id: 51 }, { id: 52 }], removed: [41] });
            expect(refreshPageAndMetadata).not.toHaveBeenCalled();
        }
    });
});
