import { defineComponent } from 'vue';

import type { ImportPreviewRecord } from './importPreview';

export default defineComponent({
    name: 'DecisionPreviewReplacementBridge',
    props: {
        onReclassified: {
            type: Function as unknown as () => (items: ImportPreviewRecord[], removedPreviewIds?: number[]) => void,
            required: true,
        },
    },
    setup(props, { slots }) {
        const handleReclassified = (items: ImportPreviewRecord[], removedPreviewIds?: number[]): void => {
            props.onReclassified(items, removedPreviewIds);
        };
        return () => slots['default']?.({ handleReclassified });
    },
});
