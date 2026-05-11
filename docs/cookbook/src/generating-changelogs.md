# Generating Changelogs

The prosaic CLI reads JSON-lines events from stdin and writes prose to stdout.
For changelogs, a single preset wires up the right vocabulary and grouping
strategy.

## Install

```sh
cargo install prosaic
```

## One-liner

```sh
echo '{"key":"code.renamed","old_name":"AuthHelper","new_name":"AuthService","consumer_count":14}' \
  | prosaic --preset=changelog
```

Output:

```
AuthHelper was renamed to AuthService, affecting 14 direct consumers.
```

## Realistic five-event input

Save the following as `changes.jsonl`, one JSON object per line:

```jsonl
{"key":"code.added","entity_type":"function","name":"batchExport","consumer_count":0}
{"key":"code.modified","entity_type":"class","name":"InvoiceProcessor","consumer_count":6}
{"key":"code.deleted","entity_type":"function","name":"legacyParseXML","consumer_count":0}
{"key":"git.dependency_updated","name":"tokio","old_version":"1.35","new_version":"1.38"}
{"key":"git.tagged","version":"2.4.0","author":"alice"}
```

Run:

```sh
prosaic --preset=changelog < changes.jsonl
```

Example output (exact phrasing varies by variation mode):

```
A new function batchExport was added. InvoiceProcessor was modified, affecting
6 direct consumers. The function legacyParseXML was removed. The dependency
tokio was updated from 1.35 to 1.38. Version 2.4.0 was tagged by alice.
```

## What `--preset=changelog` does

The preset is a shorthand for these explicit flags:

```sh
prosaic \
  --vocab code,git,release \
  --strategy by-action \
  --max-length 120
```

- `--vocab code,git,release` loads the three vocabulary modules that cover
  code-change, git, and release events. Each module registers templates for
  its event keys.
- `--strategy by-action` groups events by rhetorical category — removals lead,
  then additions, then modifications. This produces the conventional changelog
  order regardless of the input event sequence.
- `--max-length 120` caps each sentence at 120 characters, splitting at
  natural clause boundaries rather than mid-word.

## Override the grouping strategy

```sh
# Sequential — render in input order, streaming-friendly
prosaic --preset=changelog --strategy=sequential < changes.jsonl

# By entity — group events that share an entity name into the same sentence
prosaic --preset=changelog --strategy=by-entity < changes.jsonl
```

`by-entity` is useful for per-PR summaries where you want all changes to
`InvoiceProcessor` in one sentence: "InvoiceProcessor was renamed, modified,
and its signature changed."

## Available presets

| Preset | Vocab | Strategy | Max length | Smart quotes |
|--------|-------|----------|------------|--------------|
| `changelog` | code, git, release | by-action | 120 | off |
| `release-notes` | release | by-action | — | on |
| `digest` | code, git, release, pr | by-entity | 100 | off |

## Pipe through jq for structured data

If your CI pipeline produces structured JSON rather than JSONL, convert it
first:

```sh
cat deploy-events.json | jq -c '.events[]' | prosaic --preset=changelog
```

The only requirement is that each line is a self-contained JSON object with
a `key` field naming the template.

## Using the Rust API directly

The CLI is a thin wrapper. For in-process generation:

```rust
use prosaic_core::{ctx, DocumentPlan, Engine, GroupingStrategy, Session, Strictness};
use prosaic_grammar_en::English;

fn main() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .max_sentence_length(120);

    prosaic_vocab_code::register(&mut engine).unwrap();
    prosaic_vocab_git::register(&mut engine).unwrap();
    prosaic_vocab_release::register(&mut engine).unwrap();

    let events = vec![
        ("code.added", ctx! {
            entity_type: "function",
            name: "batchExport",
            consumer_count: 0,
        }),
        ("code.modified", ctx! {
            entity_type: "class",
            name: "InvoiceProcessor",
            consumer_count: 6,
        }),
        ("git.tagged", ctx! {
            version: "2.4.0",
            author: "alice",
        }),
    ];

    let plan = DocumentPlan::from_events_grouped(
        &events,
        &engine,
        GroupingStrategy::ByAction,
    );

    let mut session = Session::new();
    let changelog = plan.render(&engine, &mut session).unwrap();
    println!("{changelog}");
}
```
