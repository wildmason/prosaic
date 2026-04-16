# Codex Review - 2026-04-16

Scope: Rust workspace review for the Prosaic natural language generation engine, with focus on `prosaic-core`, the proc macros, session/discourse behavior, feature flags, CLI/WASM/tracing boundaries, and documentation promises.

## Executive Summary

The project has a broad test suite and the main workspace builds cleanly under `clippy -D warnings`. The core design is coherent: immutable `Engine`, mutable `Session`, language-specific grammar crates, optional vocabulary crates, and feature-gated polish/time/REG behavior.

The main risks I found are not broad architectural problems. They are correctness edge cases at API boundaries: streaming iterator errors, unchecked numeric conversions, proc-macro validation drift, recursive partials, and state mutation from APIs documented as diagnostic or inline-only. These can be fixed without reworking the whole engine.

## Findings

### High: `RenderIter` can return the same error forever on aggregated runs

References:
- `prosaic-core/src/engine.rs:165`
- `prosaic-core/src/engine.rs:171`
- `prosaic-core/src/engine.rs:179`
- `prosaic-core/src/engine.rs:185`
- `prosaic-core/src/engine.rs:195`
- `prosaic-core/src/engine.rs:201`

`RenderIter::next` advances `self.i` only after an aggregated action/gapping/same-entity run succeeds. If `render_aggregated_subjects` fails, or if any render inside the gapping/same-entity loops fails, the iterator returns `Some(Err(e))` without advancing.

Impact:
- A caller that continues after an error gets the same failing run again.
- In gapping/same-entity runs, successful renders before the failing event already mutated the session, so retrying can compound discourse state.
- This contradicts the public docs on `render_iter`, which say the iterator remains usable for subsequent events after an error.

Suggested fix:
- Decide whether iterator errors are terminal or skippable.
- If terminal, set `self.i = self.events.len()` before returning the error.
- If skippable, advance to the failing event plus one, and snapshot/restore any successful partial renders inside the run before returning.
- Add tests for action aggregation, gapping, and same-entity reduction where the second event fails.

### High: Numeric conversions silently wrap large unsigned values

References:
- `prosaic-core/src/context.rs:113`
- `prosaic-core/src/context.rs:140`
- `prosaic-core/src/context.rs:144`
- `prosaic-derive/src/lib.rs:14`
- `prosaic-derive/src/lib.rs:123`
- `prosaic-derive/src/lib.rs:146`

The core `IntoValue` docs explicitly exclude `u64` because converting to `i64` can overflow, but the same unchecked `as i64` conversion is still used for `usize`. The derive macro is more dangerous: it advertises and accepts `u64`, then generates `Value::Number(#accessor as i64)`.

Impact:
- `u64::MAX` derived into context becomes `-1`.
- Large `usize` values on 64-bit platforms can also become negative.
- The CLI path handles this correctly with `i64::try_from`, so behavior differs by ingestion path.

Suggested fix:
- Remove `u64` from the derive-supported numeric list, or generate a checked conversion returning a compile-time/runtime error pattern.
- Consider removing `usize` from `IntoValue`, or use `i64::try_from` where the API can return `Result`.
- Add derive tests for `u64::MAX` and `usize::MAX` so this cannot regress silently.

### High: Proc-macro pipe validation is out of sync with runtime pipes

References:
- `prosaic-core/src/engine.rs:601`
- `prosaic-core/src/engine.rs:620`
- `prosaic-core/src/engine.rs:622`
- `prosaic-core/src/engine.rs:624`
- `prosaic-derive/src/lib.rs:195`
- `prosaic-derive/src/lib.rs:207`
- `prosaic-derive/src/lib.rs:208`

The runtime supports `demonstrative`, but `prosaic_template!` does not whitelist it. Conversely, the macro always accepts `relative` and `since_last`, even though those runtime pipes only exist when the `time` feature is enabled.

Impact:
- Valid templates such as `{noun|demonstrative}` fail at compile time when using `prosaic_template!`.
- Templates using `{ts|relative}` may compile through the macro but fail at runtime in a no-time engine.

Suggested fix:
- Move pipe metadata into one shared source, or expose a core function/table for proc macro validation.
- Add a compile-pass test for `demonstrative`.
- Gate `relative` and `since_last` validation according to the feature configuration, or document that the macro validates the superset and add runtime feature tests.

### High: Recursive partials can stack overflow at render time

References:
- `prosaic-core/src/engine.rs:536`
- `prosaic-core/src/engine.rs:544`
- `prosaic-core/src/engine.rs:1440`

Partials are expanded recursively by looking up `{>name}` and calling `render_segments_into` on the partial's cloned segments. There is no cycle detection or recursion depth limit.

Impact:
- `a = "{>a}"` or `a = "{>b}", b = "{>a}"` can recurse until stack overflow.
- In Rust, stack overflow usually aborts the process rather than returning a recoverable `ProsaicError`.
- This is especially risky for user-authored template systems or service-facing template registration.

Suggested fix:
- Detect cycles at registration time by walking partial references, or track an expansion stack during rendering.
- Return `TemplateParseError` or a new `RecursivePartial` error with the cycle path.
- Add tests for direct and indirect recursion.

### Medium: `render_inline` mutates session state despite docs saying it does not participate in discourse tracking

References:
- `prosaic-core/src/engine.rs:777`
- `prosaic-core/src/engine.rs:785`
- `prosaic-core/src/engine.rs:899`
- `prosaic-core/src/engine.rs:1731`
- `prosaic-core/src/engine.rs:1740`
- `prosaic-core/src/engine.rs:1741`

`render_inline` calls the same `render_template_into` path as registered templates. Some pipes are stateful: `join` advances the list-style cycle, and plural `refer` calls `mention_entity` and sets plural focus. `render_inline` also does not snapshot/restore the session if expansion fails after a stateful pipe.

Impact:
- Inline rendering can change later registered-template output.
- A failed inline render can leak partial state.
- The docs promise "no connectives, no entity tracking", but plural `refer` does track entities.

Suggested fix:
- Either make `render_inline` transactional and state-isolated, or update the docs to say inline templates may consume stateful pipe cycles.
- If inline output should still feed repetition scoring, record only `output` after a successful isolated render.
- Add tests for `render_inline("{items|truncate:1|join}")` not advancing the next registered render's list style, and for failed inline renders after a stateful pipe.

### Medium: RST relation rendering can double-prepend discourse markers

References:
- `prosaic-core/src/engine.rs:1872`
- `prosaic-core/src/engine.rs:1893`
- `prosaic-core/src/engine.rs:1908`

`render_batch_with_relations` prepends a marker from `Language::discourse_marker`, then calls `render`, which can independently prepend a discourse connective such as `Similarly,` or `However,`.

Impact:
- Output can become awkward or contradictory, for example `Furthermore, Similarly, ...`.
- The current tests cover marker insertion and determiner lowercasing, but not interaction with the automatic connective system.

Suggested fix:
- Add an internal render mode that suppresses automatic connectives when an explicit RST marker is supplied.
- Alternatively, strip any leading automatic connective before applying the RST marker.
- Add a test with two different entities using the same action and `Some(RstRelation::Elaboration)` on the second event.

### Medium: `It also` subject replacement corrupts multi-word entity names

References:
- `prosaic-core/src/engine.rs:332`
- `prosaic-core/src/engine.rs:3098`
- `prosaic-core/src/engine.rs:3103`

`prepend_replacing_subject_in_place` assumes a full NP subject is exactly `The <type> <name> ...` where `<name>` is one whitespace-delimited token. It strips two words after `The`. For a name like `Login flow`, the remaining sentence starts at `flow ...`, producing malformed output.

Impact:
- General-purpose NLG contexts with human names, feature names, release titles, or product names can produce invalid prose.
- The code-oriented examples mostly use single-token identifiers, so the current tests do not expose this.

Suggested fix:
- Avoid string surgery against rendered prose. Carry the rendered subject span from the `refer` pipe or from the template expansion phase.
- Short-term: only apply the subject-replacement optimization when the extracted entity name has no whitespace; otherwise fall back to comma-style connective prepending.
- Add tests for same-entity different-action renders with `name = "Login flow"`.

### Medium: Diagnostics report fields that are never actually populated

References:
- `prosaic-core/src/engine.rs:108`
- `prosaic-core/src/engine.rs:2053`
- `prosaic-core/src/engine.rs:2062`
- `prosaic-core/src/engine.rs:2065`

`RenderExplanation` exposes `list_style` and `cleanup_stripped_tail`, but `render_explained` always returns `list_style: None` and `cleanup_stripped_tail: false`.

Impact:
- Consumers relying on diagnostics get misleading data.
- Template-author tooling built on `render_explained` cannot debug list style selection or silent cleanup, even though the struct says it can.

Suggested fix:
- Thread a diagnostics collector through `RenderCtx`, set it from `pipe_join` and `cleanup_artifacts_in_place`, and return the actual values.
- If these fields are not ready, remove or clearly document them as reserved.

### Low: Formatting is not clean

Reference:
- `cargo fmt --all --check`

The formatter check currently fails across many files. The diffs are mostly mechanical line wrapping and import ordering.

Impact:
- If formatting is part of CI, CI will fail.
- If formatting is not part of CI, later contributors will create noisy formatting-only diffs.

Suggested fix:
- Run `cargo fmt --all` once and commit it as a standalone formatting change.

## Additional Notes

- `prosaic-core/src/engine.rs` is doing a lot: rendering, scoring, aggregation, reduction, cleanup, REG surface construction, diagnostics, and tests. It is still understandable, but the stateful render path is now complex enough that a smaller internal `render_state` or diagnostics structure would reduce future mistakes.
- The workspace has good coverage volume, including property tests and feature-specific behavior. The gaps are mostly cross-feature and API-contract tests rather than missing happy-path tests.
- The grammar crates intentionally keep their scope small. That is fine for v1, but the docs should keep distinguishing "implemented minimal grammar" from "natural language quality" for Spanish and German.

## Verification Performed

Commands run from the repository root:

- `cargo test --workspace --all-features`
  - Result: passed.
- `cargo check -p prosaic-core --no-default-features`
  - Result: passed.
- `cargo check -p prosaic-core --no-default-features --features serde`
  - Result: passed.
- `cargo check -p prosaic-wasm --target wasm32-unknown-unknown`
  - Result: passed.
- `cargo clippy --workspace --all-features -- -D warnings`
  - Result: passed.
- `cargo test -p prosaic-core --no-default-features`
  - Result: passed, but emitted one unused-import warning in `prosaic-core/tests/faithfulness_scorer.rs`.
- `cargo fmt --all --check`
  - Result: failed with formatting diffs.

