# Prosaic Studio — App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or subagent-driven-development if available). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Prosaic Studio Tauri 2 + Angular 21 desktop app that consumes `prosaic-project` (load/save/validate/bundle) and `prosaic-wasm` (render/explain/score) to provide the live editing loop locked during brainstorming.

**Architecture:** New repo `~/Documents/development/@wildmason/prosaic-studio/`. Tauri Rust backend exposes filesystem commands (open project, save template, watch project) wrapping `prosaic-project`. Angular 21 frontend loads `prosaic-wasm` into the renderer for sub-millisecond render previews and uses Aegis components throughout. Layout matches the v2 mockup: folder tree + sidebar cards on the left, editor + stacked previews on the right.

**Tech Stack:** Tauri 2, Angular 21 (signals + new control flow + dedicated `.html` templates), `@wildmason/aegis`, Monaco editor with custom Prosaic DSL language, prosaic-wasm.

---

## Task 1: Repo + Tauri scaffold

**Files:** `prosaic-studio/{src-tauri/Cargo.toml, src-tauri/tauri.conf.json, src-tauri/build.rs, src-tauri/capabilities/main.json, src-tauri/src/{main.rs,lib.rs}, README.md, .gitignore}`

- [ ] Create the repo directory and initial structure mirroring Crucible.
- [ ] `Cargo.toml` for src-tauri with `tauri`, `tauri-plugin-dialog`, `tauri-plugin-updater`, `tauri-plugin-fs`, `prosaic-project`, `prosaic-core` (path deps).
- [ ] `tauri.conf.json` with `productName="Prosaic Studio"`, `identifier="com.wildmason.prosaic-studio"`, `version="0.1.0"`, non-empty `beforeBuildCommand="npm --prefix ../ui run build"`, `frontendDist="../ui/dist/ui/browser"`, updater endpoint, capabilities for fs + dialog + updater.
- [ ] `src/main.rs` invoking `tauri::Builder::default()` with the project commands registered (defined in Task 2).
- [ ] Init git, first commit "Scaffold Prosaic Studio Tauri shell".

## Task 2: Tauri commands wrapping prosaic-project

**Files:** `prosaic-studio/src-tauri/src/commands/{mod.rs,project.rs,template.rs,build.rs,watch.rs}`

- [ ] `commands::project::open_project(path: String) -> Result<ProjectSnapshot, String>` — calls `prosaic_project::Project::load_from_dir`, returns a serializable snapshot (manifest + lists of templates/partials/fixtures/scenarios with their bodies).
- [ ] `commands::template::save_template(project_root: String, template_toml: String, key: String) -> Result<(), String>` — parse incoming TOML, validate, write via `Project::save_template`.
- [ ] `commands::build::build_bundle(project_root: String, target: String) -> Result<BuildBundleResult, String>` — calls `prosaic_project::build_bundle` with the requested target, returns paths of written bundles.
- [ ] `commands::watch::watch_project(project_root: String, on_change: tauri::ipc::Channel<ProjectChangeEvent>)` — uses `notify` crate to watch the project dir, emits change events to the renderer.
- [ ] Test each command via Rust unit tests against fixture projects from prosaic-project's test fixtures.
- [ ] Commit "Add Tauri commands wrapping prosaic-project".

## Task 3: Angular 21 scaffold

**Files:** `prosaic-studio/ui/{package.json, angular.json, tsconfig.json, tsconfig.app.json, public/index.html, src/{main.ts, styles.scss, index.html, app/{app.ts, app.config.ts, app.routes.ts, app.html, app.scss}}}`

- [ ] `package.json` mirroring Crucible's deps: `@angular/{core,common,compiler,forms,platform-browser,router,cdk}@^21`, `@wildmason/aegis@^1.5`, `@tauri-apps/api`, `@tauri-apps/plugin-{dialog,updater,fs}`, `monaco-editor`, `monaco-yaml`, `rxjs`, `zod`.
- [ ] `angular.json` with `main.ts` entry, output to `dist/ui/browser`, dev server on port 47200 (matches Crucible).
- [ ] `app.ts` standalone component with the shell layout signal model (currentTemplateKey, currentSalience, currentVariantIndex, currentFixtureName, currentScenarioName, dirtyFiles).
- [ ] `app.html` containing the locked layout grid: 220px left rail (folder tree top + sidebar cards bottom), right side with editor + stacked single-render + flow-render previews.
- [ ] Wire Aegis stylesheets in `styles.scss` (`@import '@wildmason/aegis/themes.css'; @import '@wildmason/aegis/inputs.css';`).
- [ ] Verify `npm install && npm run build` succeeds.
- [ ] Commit "Scaffold Angular 21 UI with Aegis design system".

## Task 4: Core services — StudioFs (Tauri adapter) + ProjectService (signals)

**Files:** `prosaic-studio/ui/src/app/core/{fs/studio-fs.service.ts, project/project.types.ts, project/project.service.ts}`

- [ ] `StudioFs` injectable service with methods `openProject(path)`, `saveTemplate(root, key, toml)`, `buildBundle(root, target)`, `watchProject(root, callback)`. All call Tauri `invoke()` with the commands from Task 2. Pure thin wrapper for portability (SaaS port replaces this with a REST adapter).
- [ ] `project.types.ts` with TypeScript types matching the Rust ProjectSnapshot shape (Manifest, TemplateFile, Variant, PartialFile, Fixture, Scenario).
- [ ] `ProjectService` injectable singleton holding signals: `project`, `currentTemplateKey`, `currentSalience`, `currentVariantIndex`, `currentFixtureName`, `currentScenarioName`, `dirtyFiles`. Computed signals: `currentTemplate`, `currentVariant`, `currentFixture`, `currentScenario`, `isDirty`.
- [ ] Actions: `openProject(path)`, `openTemplate(key)`, `setSalience(s)`, `setVariant(i)`, `setFixture(name)`, `setScenario(name)`, `updateCurrentVariantBody(body)`, `saveCurrentTemplate()`.
- [ ] Component-free unit tests via vitest browser-mode.
- [ ] Commit "Add StudioFs adapter and ProjectService signal store".

## Task 5: prosaic-wasm integration — StudioEngine service

**Files:** `prosaic-studio/ui/src/app/core/engine/{prosaic-wasm.loader.ts, studio-engine.service.ts, studio-engine.types.ts}`

- [ ] Build `prosaic-wasm` with `wasm-pack build --target web --release` producing `pkg/`.
- [ ] Copy `pkg/*.wasm` and `pkg/*.js` artifacts to `prosaic-studio/ui/public/wasm/` (or vendor under `src/assets/wasm/`); update `package.json` with a `prebuild` script that re-runs `wasm-pack`.
- [ ] `prosaic-wasm.loader.ts` — async `loadProsaicWasm()` that imports the JS glue + initializes the wasm module. Memoizes the result.
- [ ] `studio-engine.types.ts` — TS interfaces matching the wasm-bindgen output (`RenderExplanation`, `VariantScore`, `FaithfulnessScore`).
- [ ] `StudioEngine` injectable singleton: `loadProject(snapshot)`, `renderTemplate(key, fixture)`, `renderTemplateExplained(key, fixture)`, `renderScenario(events)`, `renderScenarioExplained(events)`, `scoreVariants(key, fixture)`, `scoreFaithfulness(output, ctx)`, `validateTemplate(body)`. All synchronous after wasm init.
- [ ] Vitest tests with a mocked wasm module verifying the service contracts.
- [ ] Commit "Add StudioEngine service wrapping prosaic-wasm".

## Task 6: Folder tree component

**Files:** `prosaic-studio/ui/src/app/features/folder-tree/{folder-tree.component.ts, folder-tree.component.html, folder-tree.component.scss}`

- [ ] Standalone component, signal-input `project: InputSignal<LoadedProject | null>`, output events `(openTemplate)`, `(openFixture)`, `(openScenario)`.
- [ ] Renders four expandable folders (templates/partials/fixtures/tests) with their files inside, using Aegis `WmTree` if it exists, else hand-rolled with `@for` and click handlers.
- [ ] Highlights the currently-open file with the green-dot marker from the mockup.
- [ ] Vitest component tests covering: empty project, populated project, click handlers fire correct outputs.
- [ ] Commit "Add folder tree component".

## Task 7: Sidebar component (salience toggle + variants + fixture picker)

**Files:** `prosaic-studio/ui/src/app/features/sidebar/{sidebar.component.ts, sidebar.component.html, sidebar.component.scss, salience-toggle.component.ts, variant-list.component.ts, fixture-picker.component.ts}`

- [ ] `SidebarComponent` composes `SalienceToggleComponent` (low/medium/high segmented control), `VariantListComponent` (radio list of variants for current salience), `FixturePickerComponent` (select dropdown).
- [ ] All three child components are signal-input/output. Wire to `ProjectService` actions.
- [ ] Aegis `WmButtonGroup` and `WmSelect` components used where available.
- [ ] Vitest tests for each child + the composed sidebar.
- [ ] Commit "Add sidebar with salience/variant/fixture controls".

## Task 8: Monaco editor with Prosaic DSL language

**Files:** `prosaic-studio/ui/src/app/core/monaco/{prosaic-language.ts, monaco.loader.ts}`, `prosaic-studio/ui/src/app/features/editor/{editor.component.ts, editor.component.html, editor.component.scss}`

- [ ] `prosaic-language.ts` — Monarch tokenizer for Prosaic DSL: highlight `{slot}`, `{slot|pipe:arg}`, `{?key}...{/?}`, `{>partial}`. Comment style: TOML triple-quoted strings inherit syntax from the host TOML; the language runs only on extracted body strings inside a Monaco model dedicated to template bodies.
- [ ] `monaco.loader.ts` — async loader that registers the language once on first use.
- [ ] `EditorComponent` — Monaco wrapper around the current variant body. Updates `ProjectService` on every change (debounced 200ms). Shows inline error markers from `StudioEngine.validateTemplate`.
- [ ] Vitest tests using a stubbed Monaco mock.
- [ ] Commit "Add Monaco editor with Prosaic DSL language definition".

## Task 9: Single-render preview

**Files:** `prosaic-studio/ui/src/app/features/preview-single/{preview-single.component.ts, preview-single.component.html, preview-single.component.scss}`, `prosaic-studio/ui/src/app/features/explain-overlay/explain-overlay.component.ts`

- [ ] `PreviewSingleComponent` — reactive: subscribes to `currentTemplate` + `currentVariant` + `currentFixture` signals. Calls `StudioEngine.renderTemplateExplained` whenever any change. Debounced 50ms.
- [ ] Renders the output text plus an `ExplainOverlayComponent` toggle (off/on switch) showing variant/salience/refForm/connective/faithfulness chips.
- [ ] Faithfulness score chip is colour-coded green ≥0.9, yellow ≥0.7, red <0.7.
- [ ] Vitest tests with a mock StudioEngine.
- [ ] Commit "Add single-render preview with explain overlay".

## Task 10: Narrative-flow preview

**Files:** `prosaic-studio/ui/src/app/features/preview-flow/{preview-flow.component.ts, preview-flow.component.html, preview-flow.component.scss}`

- [ ] `PreviewFlowComponent` — reactive: subscribes to `currentScenario` + `project` signals. Calls `StudioEngine.renderScenarioExplained` whenever any change. Debounced 100ms.
- [ ] Renders the joined narrative output with inline highlighting of pronoun substitutions, connectives, and faithfulness scores per event (as overlay chips below the output).
- [ ] Toggle: explain mode shows transition classification per event, RST relations applied, paragraph breaks.
- [ ] Vitest tests with a mock StudioEngine returning a fake scenario explanation.
- [ ] Commit "Add narrative-flow preview component".

## Task 11: Shell composition + main.ts wiring

**Files:** `prosaic-studio/ui/src/app/features/shell/{shell.component.ts, shell.component.html, shell.component.scss}`, `prosaic-studio/ui/src/app/app.html`

- [ ] `ShellComponent` composes folder-tree + sidebar + editor + preview-single + preview-flow inside the locked grid. Splitter-resizable columns/rows (use Aegis splitter or hand-rolled with CSS resize).
- [ ] Persist column/row sizes to localStorage.
- [ ] Wire `app.html` to host `<wm-shell></wm-shell>` and load the `WmTitleBar` at the top with project name.
- [ ] Vitest snapshot test verifying the shell renders all panes.
- [ ] Commit "Add shell composition with the locked layout grid".

## Task 12: New project wizard

**Files:** `prosaic-studio/ui/src/app/features/new-project-dialog/{new-project-dialog.component.ts, new-project-dialog.component.html}`

- [ ] Wraps Aegis `WmDialog`. Form fields: name, language (en/es/de), starter (blank/changelog/vocab-pack), parent directory (Tauri dialog picker).
- [ ] Calls a new Tauri command `commands::project::scaffold_project(name, parent_dir, starter)` which delegates to `prosaic_project::scaffold_project`.
- [ ] On success, opens the new project automatically.
- [ ] Vitest tests covering form validation and submission flow.
- [ ] Commit "Add new project wizard dialog".

## Task 13: Test runner UI

**Files:** `prosaic-studio/ui/src/app/features/test-runner/{test-runner.component.ts, test-runner.component.html, test-runner.component.scss}`

- [ ] List view of all scenarios with PASS/FAIL badges and last-run timestamp. "Run all" button + per-scenario "Run" button.
- [ ] Click into a failing scenario shows expected-vs-actual diff (use Monaco diff editor or hand-rolled side-by-side).
- [ ] Calls `StudioEngine.renderScenario` for each test, compares against `scenario.expected.output`, records failures.
- [ ] Vitest tests with mock scenarios.
- [ ] Commit "Add test runner UI".

## Task 14: Distribution — auto-updater + signed builds

**Files:** `prosaic-studio/src-tauri/tauri.conf.json` (updater pubkey), `prosaic-studio/.github/workflows/release.yml`

- [ ] Configure `tauri-plugin-updater` with the wildmason releases endpoint.
- [ ] Set up a release GitHub Actions workflow that builds Windows (signed), macOS (signed + notarized), Linux (AppImage), and uploads to the releases endpoint.
- [ ] Document the release process in `prosaic-studio/RELEASES.md`.
- [ ] Commit "Add release pipeline and auto-updater configuration".

## Task 15: Workspace test sweep + first release tag

- [ ] Run `cargo test` on src-tauri side, `npm test` on ui side. All green.
- [ ] Tag `v0.1.0` and push.
- [ ] Update the `prosaic` repo's README to point at Studio.

---

## Self-Review

- **Spec coverage:** every section of the design spec maps to one or more tasks above. The acceptance criteria 1–14 in the spec map to tasks 4–14.
- **Type consistency:** TS types defined in Task 4 (`project.types.ts`) and Task 5 (`studio-engine.types.ts`) used consistently across components in Tasks 6–13.
- **Scope:** large but cohesive — single application. The cleanest cut point if the plan needs to ship in chunks is "Task 11 = MVP" (folder tree + editor + single preview is enough to demo); narrative flow + test runner + new project wizard + distribution can ship in a follow-up release.

This plan is leaner on per-step code than Plan 1 because most of the work is scaffolding and wiring (where the value is in correct decomposition, not novel algorithms). Each task carries enough specificity to execute without guessing.
