import fs from 'node:fs';
import path from 'node:path';

function readSource(relativePath: string): string {
    return fs.readFileSync(path.resolve(process.cwd(), relativePath), 'utf-8');
}

const PANEL_PATH = 'src/views/desktop/pairingcenter/components/LearningCenterPanel.vue';
const PANEL_TEMPLATE_PATH = 'src/views/desktop/pairingcenter/components/learning-center/LearningCenterPanel.template.html';
const OCR_PANEL_PATH = 'src/views/desktop/pairingcenter/components/OcrConfigPanel.vue';
const OCR_PANEL_TEMPLATE_PATH = 'src/views/desktop/pairingcenter/components/ocr-config/OcrConfigPanel.template.html';
const OCR_PANEL_STYLE_PATH = 'src/views/desktop/pairingcenter/components/ocr-config/OcrConfigPanel.scss';
const LIST_PAGE_PATH = 'src/views/desktop/pairingcenter/ListPage.vue';
const LIST_PAGE_STYLE_PATH = 'src/views/desktop/pairingcenter/ListPage.scss';

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
        readSource(OCR_PANEL_STYLE_PATH),
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
        expect(source).toContain('<Teleport defer :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">');
    });

    test('OCR config leads with processing location and keeps expert contracts behind disclosure', () => {
        const source = readOcrPanelSource();

        expect(source).toContain("tt('Where should receipt images be recognized?')");
        expect(source).toContain('ocr-provider-option__description');
        expect(source).toContain(':items="ocrLanguageOptions"');
        expect(source).toContain("tt('Expert settings')");
        expect(source).toContain("tt('How to use receipt recognition')");
        expect(source).toContain('ocrSaveActionLabel');
        expect(source).toContain('width: 100%');
        expect(source).toContain('max-width: none');
        expect(source).not.toContain('v-model="ocrConfigForm.lang" :label="tt(\'OCR Language\')"');
    });

    test('rules center height follows the viewport instead of forcing a 760px canvas', () => {
        const listPage = [readSource(LIST_PAGE_PATH), readSource(LIST_PAGE_STYLE_PATH)].join('\n');

        expect(listPage).not.toContain('min-height="760"');
        expect(listPage).not.toContain('min-height: 760px');
        expect(listPage).toContain('100dvh');
        expect(listPage).toContain('rule-center-title-toolbar');
        expect(listPage).toContain('flex-wrap: wrap');
    });
});

describe('LearningCenterPanel guided LLM config', () => {
    test('uses a three-step connection flow and moves OAuth and prompts out of the primary path', () => {
        const source = readLearningPanelSource();

        expect(source).toContain("tt('1. Choose model service')");
        expect(source).toContain("tt('2. Enter connection details')");
        expect(source).toContain("tt('3. Save and use')");
        expect(source).toContain("tt('Other sign-in methods (expert)')");
        expect(source).toContain("tt('Expert settings; normally unchanged')");
        expect(source).not.toContain('<v-tabs v-model="llmConnectionMode"');
    });

    test('shows saved connections as status cards instead of an administrator table', () => {
        const source = readLearningPanelSource();

        expect(source).toContain('llm-config-saved-grid');
        expect(source).toContain("tt('Test connection')");
        expect(source).toContain("tt('Use this connection')");
        expect(source).not.toContain('<v-table v-if="llmSavedConfigs.length > 0"');
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
