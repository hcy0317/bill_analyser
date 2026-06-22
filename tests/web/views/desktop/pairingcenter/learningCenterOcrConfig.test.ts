import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

const PANEL_PATH = 'src/views/desktop/pairingcenter/components/LearningCenterPanel.vue';
const PANEL_TEMPLATE_PATH = 'src/views/desktop/pairingcenter/components/learning-center/LearningCenterPanel.template.html';
const OCR_PANEL_PATH = 'src/views/desktop/pairingcenter/components/OcrConfigPanel.vue';
const OCR_PANEL_TEMPLATE_PATH = 'src/views/desktop/pairingcenter/components/ocr-config/OcrConfigPanel.template.html';
const LIST_PAGE_PATH = 'src/views/desktop/pairingcenter/ListPage.vue';

function readLearningPanelSource(): string {
    return [
        readSource(PANEL_PATH),
        readSource(PANEL_TEMPLATE_PATH),
    ].join('\n');
}

function readOcrPanelSource(): string {
    return [
        readSource(OCR_PANEL_PATH),
        readSource(OCR_PANEL_TEMPLATE_PATH),
    ].join('\n');
}

describe('LearningCenterPanel OCR config placement', () => {
    test('LLM config tab no longer renders OCR config controls', () => {
        const source = readLearningPanelSource();

        expect(source).toContain("activeTab === 'llm-config'");
        expect(source).not.toContain('v-model="ocrConfigForm.provider"');
        expect(source).not.toContain('v-model="ocrConfigForm.lang"');
        expect(source).not.toContain('loadOCRConfig');
    });

    test('OCR config is a standalone sibling page with import/export controls', () => {
        const source = readOcrPanelSource();
        const listPage = readSource(LIST_PAGE_PATH);

        expect(listPage).toContain("value: 'ocr-config'");
        expect(listPage).toContain('<ocr-config-panel');
        expect(source).toContain('const resp = await services.getOCRConfig();');
        expect(source).toContain('const resp = await services.updateOCRConfig');
        expect(source).toContain('section-key="ocrConfig"');
        expect(source).toContain('password-required-for-export');
    });
});

describe('LearningCenterPanel feature display', () => {
    test('learning suggestions and rules render typed feature chips instead of splitting raw summaries', () => {
        const source = readLearningPanelSource();

        expect(source).toContain('getSuggestionFeatureChips(item)');
        expect(source).toContain('getRuleFeatureChips(rule)');
        expect(source).toContain('tt(feat.labelKey)');
        expect(source).not.toContain("getFeatureSummary(item).split(' · ').filter(Boolean)");
        expect(source).not.toContain("getRuleFeatSummary(rule).split(' · ').filter(Boolean)");
    });
});
