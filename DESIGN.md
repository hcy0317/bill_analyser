# Design

## Source of truth

Status: Active  
Date: 2026-08-31  
Product surfaces: Bill Analyser desktop web; Rules Center → LLM Recognition → LLM Configuration / OCR Configuration.

Evidence reviewed:

- Current implementation and browser state: `LearningCenterPanel.vue`, `LearningCenterPanel.template.html`, `useLearningCenterLlmConfig.ts`, `OcrConfigPanel.vue`, and `ocr-config/**`.
- Existing visual contracts: `.agents/skills/bill-analyser-ui-style-reference/SKILL.md`, `src/web/src/desktop-main.ts`, and desktop card/form patterns.
- Current runtime contracts: `docs/PROJECT_OVERVIEW.md` and Rust LLM/OCR provider, credential-redaction, SSRF, and receipt-recognition paths.
- Mature references: WeKnora model/provider and parser-engine settings, Open WebUI OpenAI-compatible connections, Dify provider credentials, PaddleOCR local/self-hosted/cloud modes, and Sub2API guided account setup.

## Brand

Bill Analyser should feel private, dependable, and calm. Intelligent features explain what data leaves the server, what may cost money, and what the user must do next. Avoid administrator-console density, unexplained acronyms, raw JSON in the primary flow, decorative gradients, and claims that a provider is usable before it has been saved or tested.

## Product goals

- Let a non-technical user configure a common LLM with provider, credential, and model in one guided flow.
- Let a user understand OCR by answering one question: “Where should receipt images be recognized?”
- Make the bundled PP-OCRv6 small server model a one-click private default, with Tesseract retained as a lightweight fallback.
- Make the current saved state, unsaved selection, next action, privacy boundary, and possible cloud cost explicit.
- Keep expert provider, OAuth, prompt, and JSON contracts available without making them prerequisites.

Non-goals:

- Building a general model gateway or provider marketplace.
- Replacing existing Rust provider adapters, credential redaction, SSRF controls, or receipt-recognition APIs.
- Automatically sending a receipt or paid model request from the settings page.

Success signals:

- The default LLM form exposes only provider, API key, model, endpoint summary, and optional display name.
- The default OCR form exposes only one mode choice and fields required by that mode.
- Saved and unsaved states are visually distinguishable, and every page explains where the feature is used.

## Personas and jobs

- Personal bookkeeper: wants receipt recognition without knowing OCR engines or JSON.
- Privacy-conscious self-hoster: wants to know whether an image stays on the Bill Analyser server.
- Advanced operator: needs custom OpenAI-compatible endpoints, local JSON OCR commands, OAuth/session credentials, and prompt overrides.

Primary jobs:

- Connect a model safely.
- Choose where images are processed.
- Understand whether the configuration is saved and active.
- Find the actual “recognize a receipt” entry after configuration.

## Information architecture

The existing Rules Center and its three LLM Recognition tabs remain stable.

LLM Configuration:

1. Current connection summary and saved connection cards.
2. “Add model connection” dialog with three numbered sections: choose service, enter connection details, save and use.
3. API key is the default path. OAuth/credential JSON is an expert disclosure, not a peer tab.
4. Prompt overrides and generation parameters are grouped under “Expert settings; normally unchanged.”

OCR Configuration:

1. Current saved mode summary.
2. Primary question: “Where should receipt images be recognized?”
3. Four mutually exclusive cards: built-in (recommended), self-hosted/local, cloud vision, off.
4. Only the selected mode’s necessary fields appear.
5. A “How to use it” section points to Transactions → Add → Recognize receipt image; settings never upload or bill automatically.

## Design principles

- Task language before implementation language: say “built-in recognition,” not “Tesseract,” until supporting detail.
- Progressive disclosure: common setup first, expert contracts second.
- Saved state is authoritative: an unsaved selection never changes the “currently enabled” label.
- Privacy and cost at the decision point: cloud-image transfer warnings sit on the cloud card and form.
- One primary action per surface: save and enable; testing/activation are secondary actions on saved LLM cards.

## Visual language

- Use existing Vuetify outlined cards, comfortable fields, tonal status chips, and the repository primary color.
- Use a 4/8/12/16/24 px spacing rhythm from existing framework utilities.
- Choice cards use a clear selected border/background, short title, one-sentence description, and a small semantic badge.
- Avoid new hard-coded brand colors; use `primary`, `success`, `warning`, `error`, and theme surface variables.
- Motion is limited to existing expansion and dialog transitions.

## Components

- Existing: `v-card`, `v-item-group`, `v-alert`, `v-chip`, `v-select`, `v-text-field`, `v-expansion-panels`, `v-empty-state`, `v-dialog`.
- LLM saved connection cards replace the dense table while retaining test, activate, and delete actions.
- OCR setup cards remain local to `OcrConfigPanel`; LLM provider guidance remains local to the LLM configuration dialog.
- Tokens and shared defaults remain owned by Vuetify theme and existing desktop styles; no parallel design-system package is introduced.

## Accessibility

- Target WCAG 2.1 AA semantics and contrast.
- Choice cards must remain real keyboard-focusable controls with a visible selected state not conveyed by color alone.
- Fields retain programmatic labels; help text is adjacent to the controlled field.
- Dialog close, cancel, and primary save actions remain keyboard reachable.
- Loading, success, warning, error, and unsaved states include text labels.

## Responsive behavior

- Desktop choice cards use up to four columns; under 960 px they use two columns; narrow screens use one column.
- The Rules Center uses viewport-relative `dvh` sizing instead of a fixed 760 px canvas so Windows display scaling from 100% through 200% does not clip the default surface.
- Dialog content is single-column first and never requires horizontal scrolling.
- Toolbar actions wrap; the primary action stays visible after content.
- Long Chinese and English provider names wrap without truncating required meaning.

## Interaction states

- Loading: skeleton/progress on the affected card or action only.
- Empty: explain why no saved LLM connection exists and offer one primary add action.
- Unsaved: show “Unsaved changes” while preserving the current saved-mode status.
- Success: refresh from the server and show the resulting active mode/connection.
- Error: preserve inputs and provide an actionable category such as address, credential, model, timeout, or provider limit.
- Disabled: explain that no automatic OCR request will run.
- Slow network: action remains busy and duplicate submission is disabled.

## Content voice

- Use plain Chinese first; retain product/provider names only when they help recognition.
- Prefer “服务地址” over “Base URL,” “密钥” over unexplained credential terminology, and “专家设置” over “高级模式.”
- State consequences directly: “图片会发送到该云端服务，可能产生费用.”
- Avoid raw environment variable names and JSON syntax in the common path.

## Implementation constraints

- Vue 3 + Vuetify desktop patterns; preserve existing route, REST, user scope, redaction, allowlist, and provider values.
- Do not issue paid LLM/OCR calls during automated verification.
- The backend image pins RapidOCR/ONNX Runtime, verifies packaged model hashes at build and deployment checks, caps image bytes/pixels, and performs no first-run model download.
- API keys are never read back in plaintext; blank same-provider credentials keep the saved secret.
- Existing import/export sections and service method names remain compatible.
- Frontend changes require structure check, lint, full Jest coverage above 90%, build, and real browser checks in wide/narrow layouts.

## Open questions

- [ ] Should a future backend endpoint discover model IDs for compatible providers? Owner: LLM runtime. Impact: it could replace manual model entry, but it is not required for this simplification.
- [ ] Should a future OCR settings flow offer an explicit user-selected sample image test? Owner: product/security. Impact: it requires a deliberate upload/cost confirmation and is not performed automatically.
