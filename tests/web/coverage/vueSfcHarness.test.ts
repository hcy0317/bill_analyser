import VueCoverageHarness from '../fixtures/VueCoverageHarness.vue';

const { createSSRApp } = jest.requireActual('vue');
const { renderToString } = jest.requireActual('vue/server-renderer');

describe('Vue SFC Jest coverage harness', () => {
    it('imports, mounts, and renders a Vue single-file component', async () => {
        const app = createSSRApp(VueCoverageHarness, {
            label: '  Vue SFC coverage is live  '
        });

        const renderedHtml = await renderToString(app);

        expect(renderedHtml).toContain('data-testid="vue-coverage-harness"');
        expect(renderedHtml).toContain('VUE SFC COVERAGE IS LIVE');
    });
});
