---
name: bill-analyser-ui-style-reference
description: Repository-specific UI style reference for Bill Analyser. Use this for page layout, button variants, colors, spacing, dialogs, tables, and responsive consistency.
---

# Bill Analyser UI Style Reference

This is the shared UI style-reference skill for Bill Analyser.
If a tool needs its own discovery wrapper, keep that wrapper thin and point back here.

## When to Use

Use this skill when you are:

- adding or restyling Vue pages or components under `src/web/src/**`
- choosing page layout, card/panel hierarchy, or title/action placement
- adding buttons, forms, tables, dialogs, sheets, or popovers
- changing theme colors, amount colors, spacing rhythm, responsive behavior, or light/dark visuals

## Non-goals

- This is a repository reference, not a generic design-system tutorial.
- It does not replace Vuetify or Framework7 documentation.
- It does not force desktop and mobile into one visual grammar.

## Source-of-Truth Map

- Desktop theme and default props: `src/web/src/desktop-main.ts`
- Mobile theme and app shell: `src/web/src/MobileApp.vue`
- Desktop global utilities: `src/web/src/styles/desktop/global.scss`
- Mobile global utilities: `src/web/src/styles/mobile/global.scss`
- Desktop amount colors: `src/web/src/styles/desktop/amount-color.scss`
- Mobile amount colors: `src/web/src/styles/mobile/amount-color.scss`
- Amount color presets: `src/web/src/core/color.ts`
- Brand-color follow-up touchpoints: `src/web/src/index.html`, `src/web/src/components/common/PinCodeInput.vue`
- Desktop button overrides: `src/web/src/styles/desktop/template/vuetify/components/_button.scss`
- Desktop field overrides: `src/web/src/styles/desktop/template/vuetify/components/_field.scss`
- Desktop table overrides: `src/web/src/styles/desktop/template/vuetify/components/_table.scss`
- Desktop dialog overrides: `src/web/src/styles/desktop/template/vuetify/components/_dialog.scss`
- Desktop shell example: `src/web/src/views/desktop/MainLayout.vue`
- Desktop confirm dialog example: `src/web/src/components/desktop/ConfirmDialog.vue`

## Shared Style Invariants

- Brand primary stays anchored on `#c67e48`; change theme sources instead of scattering new hardcoded variants.
- Treat brand colors, status colors, and money colors as separate semantics.
- Keep both light and dark theme paths working.
- Reuse existing toolbar/title/card patterns before inventing one-off page shells.
- Prefer shared classes, theme defaults, and existing variables over inline styles or per-page color overrides.
- Keep desktop and mobile visually aligned in tone, but do not collapse them into one framework-specific pattern.

## Color and Semantic Rules

- Primary action color comes from `desktop-main.ts` or `MobileApp.vue`, not page-local hex values.
- `success` and `error` are status colors. Income and expense display should keep using `.text-income`, `.text-expense`, `.bg-income`, and `.bg-expense` semantics from the amount-color styles.
- If the primary brand color changes, review `src/web/src/desktop-main.ts`, `src/web/src/MobileApp.vue`, `src/web/src/styles/desktop/global.scss`, `src/web/src/styles/mobile/global.scss`, `src/web/src/index.html`, and `src/web/src/components/common/PinCodeInput.vue` together.
- Prefer theme variables such as `var(--v-theme-*)`, `--default-icon-color`, and Framework7 CSS variables over literal color values.

## Desktop Conventions (Vuetify)

- Use `src/web/src/views/desktop/MainLayout.vue` as the page-shell reference.
- Prefer title + toolbar rows such as `.title-and-toolbar` for page actions.
- Default content surfaces should be `v-card`, `v-card-text`, and `v-divider`, not loose blocks of raw content.
- For buttons:
  - everyday actions: prefer `variant="outlined"` or `variant="text"` for toolbar/icon actions
  - emphasized but non-destructive actions: prefer `variant="tonal"`
  - destructive actions: use `color="error"`
  - keep global defaults from `desktop-main.ts` and `_button.scss` as the baseline
- For forms:
  - prefer `outlined` + `comfortable`
  - stay consistent with `_field.scss` input height, hover, and focus behavior
- For tables:
  - follow `_table.scss` and `global.scss` header, striping, and hover patterns
  - avoid creating bespoke table chrome for a single page
- For dialogs:
  - prefer `v-dialog > v-card > v-toolbar + v-card-text + v-card-actions`
  - use `ConfirmDialog.vue` as the action hierarchy reference
  - for desktop add/edit forms that mirror account edit/new-account flows, use viewport-aware dialog width, padded cards (`pa-2 pa-sm-4 pa-md-8` or equivalent), centered `text-h4` title slots, content spacing aligned with account edit, and centered bottom primary/secondary actions instead of compact toolbars

## Mobile Conventions (Framework7)

- Use `f7-page` + `f7-navbar` as the default page shell and keep a single-column-first layout.
- Primary CTA should prefer `f7-button` with fill semantics; secondary actions should prefer `f7-link` or `f7-list-button`.
- Selection and edit flows should prefer `sheet`, `popup`, or list-driven patterns instead of copying desktop modal cards.
- Keep safe-area, dark-mode, and theme-color meta behavior aligned with `src/web/src/MobileApp.vue`.
- Prefer shared utility classes from `src/web/src/styles/mobile/global.scss`; do not treat existing inline styles as a pattern to expand.

## Layout and Spacing Rules

- Use framework spacing utilities and existing shared classes before adding new page-only spacing constants.
- Desktop pages should keep primary actions near the current title/toolbar area instead of scattering them across the page.
- Mobile pages should keep actions close to the current list/block and avoid dense desktop-style toolbars.
- Preserve existing scrollable shell behavior for desktop navigation/content and mobile page containers.

## Style Placement Rules

- Theme-wide defaults and tokens belong in `src/web/src/desktop-main.ts` or `src/web/src/MobileApp.vue`.
- Reusable shared visual classes belong in `src/web/src/styles/desktop/global.scss` or `src/web/src/styles/mobile/global.scss`.
- Money color semantics belong in the amount-color styles and `src/web/src/core/color.ts`.
- Vuetify component-family overrides belong in the matching `src/web/src/styles/desktop/template/vuetify/components/_*.scss` file.
- Keep page-local exceptions narrow; only promote them when the pattern repeats.
- Do not introduce new inline styling if the same intent can live in an existing shared class or theme surface.

## Review Checklist

- Confirm light and dark themes both still work.
- Confirm desktop and mobile each follow their own established visual grammar.
- Avoid new hardcoded brand colors and one-off button variants.
- Keep amount colors separate from status colors.
- Keep title, toolbar, and action hierarchy visually clear.
- Preserve hover, focus, disabled, and destructive states for interactive controls.
- Check long i18n labels and narrow viewports before finalizing.

## Validation

- For normal `src/web/**` UI changes, in `src\web`: `npm run lint`
- Only when the same task also changes `.agents/**`, `.github/**`, `.claude/**`, `.codex/**`, or hook adapter files, parse touched JSON and scan active hook configs for deleted `scripts/hooks/**` entrypoints.
