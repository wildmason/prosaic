# prosaic

Command-line front end for the Prosaic natural language generation engine.

The `prosaic` package installs the `prosaic` binary. It reads JSON-lines events
from stdin, renders them through a configured Prosaic engine, and writes prose to
stdout.

## Install

```bash
cargo install prosaic
```

The CLI is also distributed through the Wildmason Homebrew tap and Scoop bucket
when binary release assets are available.

## Usage

```bash
echo '{"key":"code.renamed","entity_type":"class","old_name":"Foo","new_name":"Bar","consumer_count":3}' \
  | prosaic --strategy sequential
```

Each input line is a JSON object with a required `key` plus context fields used
by the selected template. Strings, integers, and arrays of strings are supported.

## Common Options

- `--vocab code|git|both|none`: load built-in vocabularies.
- `--strategy sequential|by-entity|by-action`: choose batch grouping behavior.
- `--smart-quotes`: enable typographic quote substitution.
- `--max-length <N>`: split long sentences at natural boundaries.
- `--style <name>`: prefer variants tagged with a style.
- `--explain`: emit render explanations as JSON instead of plain text.
- `--strict`, `--lenient`, `--silent`: control missing-slot behavior.

## Project Commands

```bash
prosaic new my-changelog --starter=changelog
prosaic build my-changelog --target=json --out=dist
prosaic test my-changelog
```

Project commands use the `prosaic-project` folder format: `prosaic.toml`,
`templates/`, `partials/`, `fixtures/`, and `tests/`.

## License

MIT OR Apache-2.0
