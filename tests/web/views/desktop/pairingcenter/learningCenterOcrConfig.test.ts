import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

const PANEL_PATH = 'src/views/desktop/pairingcenter/components/LearningCenterPanel.vue';

describe('LearningCenterPanel OCR config placement', () => {
    test('LLM config tab also renders OCR config controls', () => {
        const source = readSource(PANEL_PATH);

        expect(source).toContain("activeTab === 'llm-config'");
        expect(source).toContain("{{ tt('OCR Config') }}");
        expect(source).toContain('v-model="ocrConfigForm.provider"');
        expect(source).toContain('v-model="ocrConfigForm.lang"');
    });

    test('refreshing LLM config loads both LLM configs and OCR config', () => {
        const source = readSource(PANEL_PATH);

        expect(source).toContain('Promise.all([loadLLMConfigs(), loadOCRConfig()])');
        expect(source).toContain('const resp = await services.getOCRConfig();');
        expect(source).toContain('const resp = await services.updateOCRConfig');
    });
});
