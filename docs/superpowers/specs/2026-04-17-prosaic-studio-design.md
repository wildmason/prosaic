# Prosaic Studio — Design Spec

**Status:** Approved 2026-04-17 (Matt + Claude during brainstorming session).
**Author:** Claude Opus 4.7 (1M context).

## Overview

Prosaic Studio is a desktop authoring environment for Prosaic templates. It targets the gap between the Prosaic library (a deterministic, discourse-aware NLG engine) and the people who *use* it: app developers writing templates for their own event streams (changelog generators, code-review summaries, release notes, BI narratives). Studio replaces the current "edit a Rust file with template strings, recompile, hand-test" workflow with a fast, visual loop: edit a template, see live single-render and multi-event narrative-flow previews update on every keystroke, run scenario tests against expected output and discourse properties, and bundle the project to a portable manifest the user's app loads at runtime.

Studio's reason for existing — the thing no other commercial NLG product offers — is the **narrative-flow preview**. Single-template previews catch grammatical bugs but miss the discourse problems that actually make NLG output sound robotic (the "2 of 2 modified files" issue we just fixed; missed pronoun substitution; jarring connectives; centering violations). Studio renders a sequence of events through a single Session and surfaces those issues in real time, with explain overlays exposing every decision the engine made.

## Goals

1. **Fast feedback loop.** Edit a template → see live single-render preview update with sub-millisecond latency. Edit again → see narrative-flow preview update across the entire scenario.
2. **Discourse-coherence visibility.** Both preview panes can toggle an explain overlay showing variant chosen, salience bucket, reference form, connective, transition classification, and faithfulness score with token highlighting.
3. **Self-contained, diffable project format.** Folder of TOML files (`templates/`, `partials/`, `fixtures/`, `tests/`); every diff captures intent.
4. **Test-first authoring.** Scenarios are first-class artifacts with discourse assertions, not just snapshot tests. A refactor that breaks pronoun selection on the second mention should fail loudly.
5. **Multi-language support from day one.** EN/ES/DE selectable in Studio; per-variant `language` field; translation-memory UI deferred but the schema accommodates it without migration.
6. **Vocab pack composition.** Projects declare dependencies on existing vocab packs (`prosaic-vocab-code`, `-git`, `-pr`, `-release`); Studio merges them at load time. Project-local templates with the same key override.
7. **Portable runtime artifact.** `prosaic build` produces a JSON manifest (loadable by any host language via `Engine::load_manifest`) and optionally a generated Rust source module (for Rust hosts wanting compile-time validation).
8. **Architectural readiness for SaaS.** The Angular UI talks to a thin engine adapter (`StudioEngine`) and a thin filesystem adapter (`StudioFs`). Swapping the Tauri implementations for REST API implementations is the entire web port.

## Non-Goals (v1)

- Hosted SaaS (deferred — architecture supports it, no infra in v1).
- Translation memory UI (schema supports it; UI is post-v1).
- BI plugins / Tableau / Power BI integrations (deferred to a later sales push).
- Insight detection module (separate workstream after Studio).
- Style-guide enforcement layer (separate workstream after Studio).
- Tone/brand presets (separate workstream after Studio).
- Multi-user collaboration / real-time sync (deferred to SaaS).
- Authentication / billing (deferred to SaaS).

## Target User

**Primary persona:** App developer writing Prosaic templates for their own product. Examples: someone building a Crucible-style code-review summary engine, a release-notes generator, a customer-incident narrative tool. Comfortable with code editors and JSON; not necessarily an expert in NLG theory. Wants templates to *sound right* and to *stay sounding right* through refactors.

**Secondary persona (foundation laid, full UX deferred):** Vocab pack author building reusable template families for distribution as crates. The TOML schema, salience tier UI, and variant management are designed to support this workflow when the full publishing pipeline ships in a follow-up release.

## Project Structure on Disk

```
my-changelog-project/
├── prosaic.toml              # Project settings, language, salience thresholds, vocab deps
├── templates/                # User-authored templates, one .toml per key
│   ├── code.modified.toml
│   ├── code.renamed.toml
│   └── summary.changeset_overview.toml
├── partials/                 # Reusable template fragments
│   └── impact_tail.toml
├── fixtures/                 # Single-event context maps for live preview
│   ├── userservice-modified.json
│   ├── pr-142-summary.json
│   └── empty-changeset.json
├── tests/                    # Multi-event scenarios with expected output + discourse assertions
│   ├── pr-142-auth-refactor.toml
│   └── empty-changeset.toml
└── .prosaic/                 # Studio-managed cache (not committed)
    ├── snapshots/            # Auto-saved test snapshots
    └── build/                # Build artifacts
```

### `prosaic.toml`

```toml
name = "my-changelog-project"
version = "0.1.0"
language = "en"                    # Default render language; per-variant override available

[engine]
strictness = "strict"
variation = "fixed"
smart_quotes = true
max_sentence_length = 0            # 0 = disabled
faithfulness_min = 0.0             # 0.0 = no gate

[engine.salience_thresholds]
low_max = 1
high_min = 20

# Vocab pack dependencies. Studio merges these at engine construction time.
# Project-local templates with the same key override.
[[dependencies]]
crate = "prosaic-vocab-code"
version = "0.3"
languages = ["en"]                 # Subset to register; default: all

[[dependencies]]
crate = "prosaic-vocab-git"
version = "0.3"
```

### Template TOML schema (`templates/<key>.toml`)

```toml
key = "code.modified"
description = "Render a 'class X was modified' event."
slots_required = ["name"]
slots_optional = ["consumer_count", "consumers"]

[[variants]]
salience = "low"
language = "en"                    # Optional; default = project language
body = "{name|refer} was modified"

[[variants]]
salience = "medium"
language = "en"
description = "Default medium variant — uses impact_tail partial."
body = """
{name|refer} was modified{?consumer_count}, \
  affecting {consumer_count} {consumer_count|pluralize:consumer}{/?}\
"""

[[variants]]
salience = "medium"
language = "en"
body = "{name|refer} got an update"   # second medium variant for variation

[[variants]]
salience = "high"
language = "en"
body = """
{name|refer} has been substantially modified, with downstream impact across \
{consumer_count} {consumer_count|pluralize:consumer}{?consumers} including \
{consumers|truncate:5|join:bracketed}{/?}. Thorough review is recommended.\
"""

[[variants]]
salience = "medium"
language = "es"
body = "{name|refer} fue modificado"
```

### Partial TOML schema (`partials/<name>.toml`)

```toml
name = "impact_tail"
description = "Trailing 'affecting N consumers' clause shared across vocab entries."
body = """
{?consumer_count}, affecting {consumer_count} {consumer_count|pluralize:consumer}{/?}\
"""
```

### Fixture JSON schema (`fixtures/<name>.json`)

```json
{
  "name": "UserService",
  "entity_type": "class",
  "consumer_count": 6,
  "consumers": ["ProfileComponent", "SettingsComponent", "AdminModule"]
}
```

A fixture is exactly the shape of a Prosaic `Context` after JSON serialization. Drop in a real event blob from your app to drive single-template preview.

### Scenario TOML schema (`tests/<name>.toml`)

```toml
name = "PR 142 — auth refactor"
description = "Real changeset from issue #142, exercises module summary + sibling code events."

[engine]
variation = "fixed"
language = "en"

# Optional per-scenario overrides (otherwise inherits prosaic.toml)
salience_thresholds = { low_max = 1, high_min = 20 }
faithfulness_min = 0.85

[[events]]
template = "summary.changeset_overview"
context = { module = "src/auth", matching = 2, total = 2 }

[[events]]
template = "code.modified"
context = { name = "UserService", entity_type = "class", consumer_count = 6 }

[[events]]
template = "code.moved"
context = { name = "UserService", from = "src/auth", to = "src/users" }
rst_relation = "elaboration"

[[events]]
template = "code.modified"
context = { name = "AuthGuard", entity_type = "class", consumer_count = 2 }

[expected]
output = """
The bulk of this changeset lives in src/auth, with both modified files belonging to that module. The class UserService was modified, affecting 6 consumers. It was also moved from src/auth to src/users. Similarly, the AuthGuard class was modified, affecting 2 consumers.
"""

# Optional discourse assertions — fail the test if the property doesn't hold
[[expected.discourse]]
event_index = 2                    # the "moved" event
reference_form = "Pronoun"         # second mention of UserService should pronominalize

[[expected.discourse]]
event_index = 3
connective_contains = "Similarly"  # AuthGuard with same action as prior entity → similarity connective
```

## Studio Architecture

### Layout (locked from brainstorming v2 mockup)

```
┌────────────────┬──────────────────────────────────────────────┐
│  FOLDER TREE   │                                              │
│                │              EDITOR                          │
│  📁 templates  │    (Monaco, Prosaic DSL syntax)              │
│    code.added  │    All variants of current template visible, │
│    code.modi●  │    with salience tier toggles and variant    │
│    code.moved  │    selector controlled by sidebar.           │
│    ...         │                                              │
│  📁 partials   ├──────────────────────────────────────────────┤
│  📁 fixtures   │              SINGLE RENDER                   │
│  📁 tests      │    (current template + selected fixture)     │
│  📄 prosaic.t  │    Toggleable explain overlay.               │
│                │                                              │
│  ── SIDEBAR ── ├──────────────────────────────────────────────┤
│                │              NARRATIVE FLOW                  │
│  Salience      │    (currently-selected scenario, all events  │
│   [low|MED|hi] │     rendered through one Session)            │
│                │    Toggleable explain overlay.               │
│  Variants      │                                              │
│   ● variant 1  │                                              │
│   ○ variant 2  │                                              │
│                │                                              │
│  Fixture       │                                              │
│   pr-142.json  │                                              │
└────────────────┴──────────────────────────────────────────────┘
```

### Tech Stack

| Layer | Choice | Rationale |
|---|---|---|
| Shell | Tauri 2 | Matches Helm/Mortar/Crucible/Vector; Rust backend reuses prosaic-core directly |
| UI | Angular 21 (signals + new control flow) | Per global Angular Greenfield Standards |
| Components | `@wildmason/aegis` v1.5+ | Mandatory per global rules; reuses Helm/Crucible patterns |
| Editor | Monaco | Already in Crucible; supports custom language definition for Prosaic DSL |
| Rendering | `prosaic-wasm` in renderer process | Sub-millisecond keystroke updates; portable to web SaaS unchanged |
| Filesystem | Tauri commands → `prosaic-studio` Rust crate | Project load/save/watch only |
| Auto-update | `tauri-plugin-updater` | Same pattern as Crucible |
| Code signing | Windows: signtool; macOS: codesign; per Tauri docs | Standard |

### Repository Layout

Studio lives in **its own repository** at `~/Documents/development/@wildmason/prosaic-studio/`, mirroring Helm/Crucible/Mortar/Vector. It depends on `prosaic-core` and `prosaic-wasm` via path-based Cargo dependencies during local dev (`path = "../oss/nlg/prosaic-core"`), switching to crates.io versions when published.

```
prosaic-studio/
├── README.md
├── SPEC.md → symlink or copy of this design doc
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── icons/
│   ├── capabilities/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── commands/
│       │   ├── project.rs       # open_project, save_template, watch_project
│       │   ├── build.rs         # invoke `prosaic build`
│       │   └── mod.rs
│       └── error.rs
└── ui/
    ├── package.json
    ├── angular.json
    ├── tsconfig.json
    ├── public/
    │   └── prosaic_wasm_bg.wasm  # bundled wasm artifact
    └── src/
        ├── index.html
        ├── main.ts
        ├── styles.scss
        └── app/
            ├── app.config.ts
            ├── app.html
            ├── app.ts
            ├── app.routes.ts
            ├── core/
            │   ├── engine/                # StudioEngine wraps prosaic-wasm
            │   │   ├── studio-engine.service.ts
            │   │   └── prosaic-wasm.loader.ts
            │   ├── fs/                    # StudioFs wraps Tauri commands
            │   │   ├── studio-fs.service.ts
            │   │   └── tauri.adapter.ts
            │   ├── project/               # Project state management
            │   │   ├── project.service.ts
            │   │   ├── project.types.ts
            │   │   └── project.parser.ts  # TOML → typed project model
            │   └── monaco/
            │       ├── prosaic-language.ts # Monaco language definition for Prosaic DSL
            │       └── monaco.loader.ts
            ├── features/
            │   ├── shell/
            │   │   ├── shell.component.ts
            │   │   ├── shell.component.html
            │   │   └── shell.component.scss
            │   ├── folder-tree/
            │   │   ├── folder-tree.component.ts
            │   │   ├── folder-tree.component.html
            │   │   └── folder-tree.component.scss
            │   ├── sidebar/
            │   │   ├── sidebar.component.ts
            │   │   ├── salience-toggle.component.ts
            │   │   ├── variant-list.component.ts
            │   │   └── fixture-picker.component.ts
            │   ├── editor/
            │   │   ├── editor.component.ts
            │   │   └── editor.component.html
            │   ├── preview-single/
            │   │   ├── preview-single.component.ts
            │   │   └── preview-single.component.html
            │   ├── preview-flow/
            │   │   ├── preview-flow.component.ts
            │   │   └── preview-flow.component.html
            │   ├── explain-overlay/
            │   │   └── explain-overlay.component.ts
            │   ├── test-runner/
            │   │   ├── test-runner.component.ts
            │   │   └── test-runner.component.html
            │   └── new-project-dialog/
            │       └── new-project-dialog.component.ts
            └── shared/
                └── (shared utilities)
```

### Component Responsibilities

- **`StudioEngine` service** — singleton; loads prosaic-wasm once at app startup. Exposes typed APIs: `loadProject(toml)`, `renderTemplate(key, fixture)`, `renderTemplateExplained(key, fixture)`, `renderScenario(scenario)`, `renderScenarioExplained(scenario)`, `scoreVariants(key, fixture)`, `scoreFaithfulness(output, ctx)`. All synchronous (wasm calls are sub-millisecond).
- **`StudioFs` service** — Tauri-only in v1; abstracts file I/O so the SaaS port can swap implementations. Methods: `openProject(path)`, `saveTemplate(path, toml)`, `watchProject(path, onChange)`, `listProjectFiles(path)`.
- **`ProjectService`** — owns the in-memory project model (signal-based). Provides reactive selectors: currently-open template, current variant, current salience tier, current fixture, current scenario, dirty state.
- **`ShellComponent`** — owns the layout grid; responsible for splitter persistence (column widths, row heights saved to settings).
- **`FolderTreeComponent`** — renders the project tree; emits open-file events; shows dirty markers.
- **`SidebarComponent`** — composes salience toggle, variant list, fixture picker. Updates project-service signals which drive editor + preview panes.
- **`EditorComponent`** — Monaco wrapper with the custom Prosaic DSL language registered (slot/pipe/conditional/partial syntax highlighting, autocomplete for known pipes, hover docs, inline error markers from the parser).
- **`PreviewSingleComponent`** — renders the current template against the selected fixture using `StudioEngine.renderTemplateExplained`. Updates on every editor keystroke (debounced 50ms). Toggleable explain overlay shows variant chosen, salience, reference form, faithfulness.
- **`PreviewFlowComponent`** — renders the currently-selected scenario through one Session. Updates on every editor keystroke (debounced 100ms — slightly more expensive). Explain overlay shows transition classification, connectives, pronoun substitutions, faithfulness per event.
- **`ExplainOverlayComponent`** — shared rendering for explain data; tag chips with the colour palette from the layout mockup.
- **`TestRunnerComponent`** — list of all scenarios with pass/fail badges; click a failure to see expected vs actual diff and discourse assertion failures.
- **`NewProjectDialogComponent`** — wizard for `File → New Project`: pick directory, choose starter template (blank, changelog starter, vocab-pack starter), set name + language.

### State Model (signals)

```typescript
// project.service.ts (sketch)
class ProjectService {
  // Loaded project state
  project = signal<LoadedProject | null>(null);

  // Selection state
  currentTemplateKey = signal<string | null>(null);
  currentSalience = signal<Salience>('medium');
  currentVariantIndex = signal<number>(0);
  currentFixtureName = signal<string | null>(null);
  currentScenarioName = signal<string | null>(null);

  // Derived signals
  currentTemplate = computed(() => {
    const p = this.project(), key = this.currentTemplateKey();
    return p && key ? p.templates.get(key) : null;
  });

  currentVariant = computed(() => {
    const t = this.currentTemplate();
    if (!t) return null;
    const matching = t.variants.filter(v => v.salience === this.currentSalience());
    return matching[this.currentVariantIndex()] ?? null;
  });

  currentFixture = computed(() => {
    const p = this.project(), name = this.currentFixtureName();
    return p && name ? p.fixtures.get(name) : null;
  });

  // Dirty tracking
  dirtyFiles = signal<Set<string>>(new Set());
  isDirty = computed(() => this.dirtyFiles().size > 0);

  // Actions
  openTemplate(key: string) { ... }
  setSalience(s: Salience) { ... }
  setVariant(i: number) { ... }
  saveCurrentTemplate() { ... }
  // ...
}
```

The renderer signals subscribe to `currentTemplate`/`currentVariant`/`currentFixture`/`currentScenario` and re-render automatically when any of those change. Editor saves debounced 1s after last keystroke.

## Backend additions to the Prosaic workspace

### New crate: `prosaic-project`

Lives in the prosaic workspace at `~/Documents/development/@wildmason/oss/nlg/prosaic-project/`. Public surface:

```rust
pub struct Project {
    pub manifest: Manifest,
    pub templates: HashMap<String, TemplateFile>,
    pub partials: HashMap<String, PartialFile>,
    pub fixtures: HashMap<String, Context>,
    pub scenarios: HashMap<String, Scenario>,
}

pub struct Manifest { /* parsed prosaic.toml */ }
pub struct TemplateFile { pub key: String, pub variants: Vec<Variant>, /* ... */ }
pub struct Variant { pub salience: Salience, pub language: String, pub body: String, /* ... */ }
pub struct PartialFile { pub name: String, pub body: String, /* ... */ }
pub struct Scenario { pub events: Vec<ScenarioEvent>, pub expected: Option<Expected>, /* ... */ }

impl Project {
    pub fn load_from_dir(path: impl AsRef<Path>) -> Result<Self, ProjectError>;
    pub fn save_template(&self, key: &str, path: impl AsRef<Path>) -> Result<(), ProjectError>;
    pub fn validate(&self) -> Vec<ValidationIssue>;
    pub fn into_engine(&self) -> Result<Engine, ProjectError>;
}

pub fn build_bundle(project: &Project, target: BuildTarget) -> Result<BuildOutput, ProjectError>;

pub enum BuildTarget { JsonManifest, RustModule, Both }
pub struct BuildOutput { pub json: Option<String>, pub rust: Option<String> }
```

The crate has its own integration tests (load real project fixtures, validate, render). It's reusable outside Studio — anyone can `prosaic_project::Project::load_from_dir(...)` to consume a Studio-authored project at runtime.

### Additions to `prosaic-cli`

- `prosaic build [--target=json|rust|both] [--out=<dir>] [<project_dir>]` — runs the bundler; default target is `json`.
- `prosaic test [<project_dir>]` — runs all scenarios; produces TAP-style output for CI use.
- `prosaic new <name> [--starter=blank|changelog|vocab-pack]` — scaffolds a new project.

### Additions to `prosaic-wasm`

Expand the wasm bindings to cover everything Studio needs:

```rust
#[wasm_bindgen]
impl ProsaicEngine {
    // Existing
    pub fn new() -> Self;
    pub fn render(&self, key: &str, ctx: JsValue) -> Result<String, JsError>;

    // New
    pub fn load_project_toml(&mut self, prosaic_toml: &str, files: JsValue) -> Result<(), JsError>;
    pub fn render_explained(&self, session: &mut ProsaicSession, key: &str, ctx: JsValue) -> Result<JsValue, JsError>;
    pub fn render_scenario(&self, scenario: JsValue) -> Result<String, JsError>;
    pub fn render_scenario_explained(&self, scenario: JsValue) -> Result<JsValue, JsError>;
    pub fn score_variants(&self, key: &str, ctx: JsValue) -> Result<JsValue, JsError>;
    pub fn score_faithfulness(&self, output: &str, ctx: JsValue) -> Result<JsValue, JsError>;
    pub fn validate_template(&self, body: &str) -> Result<JsValue, JsError>;
}
```

A typed TypeScript shim wraps these with proper types and exposes the same API to Angular code.

### Additions to `prosaic-core`

- Confirm `Engine` has all setters needed by the project loader (most already exist; verify and add any gaps for `salience_thresholds`, `faithfulness_threshold`, etc., as builder methods).
- Add `Engine::register_template_with_language` (or similar) so the project loader can register `language="es"` variants alongside `"en"` ones cleanly.
- Multi-language template selection: the `render` path needs to know which language variant to pick. Add `Engine::language_preference()` setter and per-render override.

## Build & Distribute Pipeline

- `npm --prefix ../ui run build` produces the Angular dist that Tauri bundles (per the Crucible pattern; non-empty `beforeBuildCommand` is mandatory per global Tauri rules).
- `cargo tauri build` bundles per-platform installers.
- Code-signed Windows installer, macOS dmg with notarization, Linux AppImage.
- Versions tracked in `prosaic-studio/Cargo.toml` (Tauri side) and `prosaic-studio/ui/package.json` (Angular side); kept in lockstep via release script.
- Auto-updater endpoint: `https://releases.wildmason.com/prosaic-studio/latest.json`.

## Testing Strategy

### prosaic-project (Rust)

- Unit tests for TOML parsing edge cases (missing required fields, malformed variants, unknown salience tiers).
- Integration tests using real fixture projects in `prosaic-project/tests/fixtures/`: load-roundtrip-validate-render flows.
- Property tests (proptest) for project → engine → render fidelity (any valid project produces a valid Engine that renders all its scenarios without error).

### prosaic-wasm (Rust + JS smoke tests)

- Existing Rust tests remain; new methods get matching coverage.
- A small node-based smoke harness in `prosaic-wasm/tests/js/` confirms the wasm artifact exposes the expected JS surface.

### prosaic-studio (Tauri Rust)

- Unit tests for each Tauri command (open_project, save_template, watch_project).
- Integration tests using test projects on a temp filesystem.

### prosaic-studio/ui (Angular)

- **Vitest browser-mode** for component tests (matches Crucible's stack).
- Unit tests for `StudioEngine`, `StudioFs` (mocked Tauri), `ProjectService` (signal state transitions).
- Component tests for editor, sidebar, preview panes.
- E2E via WebdriverIO (matches Helm/Crucible stack, mind WDIO v9 caveats from corrections log: prefer `data-testid` selectors over `*=` partial-text).

### Coverage matrix

Every new public function gets at least one test. Edge cases covered:
- Empty project (no templates).
- Template with no variants at requested salience (fallback chain).
- Scenario with mismatched expected output (test fails, diff renders).
- Scenario with discourse assertion failure (test fails, assertion details render).
- Template with parse error (editor shows error marker; preview shows fallback).
- Vocab pack dependency missing on disk (clear error message).
- Multi-language project (correct variant selected per `language` setting).
- File watcher externally-modified file (reload prompt, no silent overwrite).

## Out-of-scope deferred items (with notes)

- **Insight detection module** — separate workstream. Studio's narrative-flow preview will consume insight events the same way it consumes any event sequence.
- **Style guide enforcement** — separate workstream. When it lands, Studio adds a "Style" section to the sidebar.
- **Tone/brand presets** — separate workstream. Studio displays available presets in the variant picker once the prosaic-core layer ships.
- **Translation memory UI** — schema supports it (per-variant `language` field). UI deferred to v2.
- **Vocab pack publishing pipeline** — Studio v1 *consumes* vocab packs but does not publish them. v2 adds `prosaic publish`.
- **Multi-user / SaaS** — architecture is portable (StudioEngine + StudioFs adapters), v2.
- **BI plugins** — separate product surface.

## Key risks and mitigations

| Risk | Mitigation |
|---|---|
| Monaco custom language definition is non-trivial | Start with regex-based highlighting (sufficient for v1); add semantic features incrementally. Crucible already uses Monaco + monaco-yaml as reference. |
| prosaic-wasm needs significant new exports | Treat this as a strict prerequisite; expand the wasm crate first, with tests, before any Angular work. |
| Multi-language variant selection not yet in prosaic-core | Add the engine plumbing as part of Studio prep; doesn't break existing English-only callers. |
| Folder-of-files plus file watcher complexity on Windows | Use `notify` crate (already standard); test with mixed CRLF/LF, OneDrive paths (Matt's environment), and rapid-edit scenarios. |
| Monaco bundle size in Tauri | Monaco is heavy (~3MB). Same constraint as Crucible; tolerable for desktop. |
| Aegis MCP offline this session | Reading aegis source directly from `~/Documents/development/@wildmason/aegis/src/` for component APIs. Studio code uses `import from '@wildmason/aegis/components'` matching Helm. |

## Acceptance Criteria for v1

1. Open a folder containing a valid prosaic project; folder tree, sidebar, editor populate correctly.
2. Edit a template body in the editor; the single-render preview updates within 100ms.
3. Toggle salience tier in the sidebar; preview re-renders with the appropriate variant.
4. Switch fixtures via the sidebar dropdown; preview updates.
5. Open a scenario; flow preview renders the full narrative.
6. Edit any template that participates in the flow; flow preview re-renders within 200ms.
7. Toggle explain overlay on either preview; metadata badges render correctly.
8. Save edits; the TOML file on disk matches the editor state.
9. External file changes prompt a reload (no silent overwrite).
10. `prosaic build --target=json` produces a valid manifest the engine can load.
11. `prosaic test` passes against a project's scenarios.
12. New-project wizard creates a runnable starter project.
13. Auto-updater check works against the staging release endpoint.
14. App is signed for Windows; dmg + AppImage build on respective hosts.

This list is the gate for "v1 complete." Polish items (keyboard shortcuts, theming, settings panel scope) come after the gate is met.
