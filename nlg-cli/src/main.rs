//! `nlg` — command-line front-end to the NLG engine.
//!
//! Reads JSON-lines events from stdin, renders them through a configured
//! engine, and writes the output to stdout one sentence (or aggregated
//! run) per line.
//!
//! # Event format
//!
//! Each line of stdin is a JSON object with:
//!
//! - `"key"`: the template key to render (required)
//! - any other keys: template context slots, as JSON values. Strings,
//!   integers, and arrays of strings are supported.
//!
//! Example input (one event per line):
//!
//! ```json
//! {"key": "code.renamed", "entity_type": "class", "old_name": "Foo", "new_name": "Bar", "consumer_count": 3}
//! {"key": "code.modified", "entity_type": "class", "name": "Foo", "consumer_count": 2}
//! {"key": "git.pr_merged", "number": 42, "title": "Add retry", "merger": "Alice"}
//! ```
//!
//! # Usage
//!
//! ```text
//! nlg [OPTIONS] < events.jsonl
//!
//! --vocab <list>         Comma-separated vocab modules to load: code, git, both (default: code,git)
//! --strategy <strategy>  Batch strategy: sequential (default), by-entity, by-action
//! --smart-quotes         Enable typographic quote substitution
//! --max-length <N>       Cap each sentence at N characters, splitting at natural boundaries
//! --explain              Emit a JSON RenderExplanation per line instead of plain text
//! --strict | --lenient | --silent
//!                         Missing-slot behavior (default: strict)
//! -h, --help             Print usage
//! ```

use std::io::{self, BufRead, Write};

use nlg_core::{
    Context, DocumentPlan, Engine, GroupingStrategy, NlgError, Session, Strictness, Value,
};
use nlg_grammar_en::English;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RawEvent {
    key: String,
    #[serde(flatten)]
    slots: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy)]
enum Strategy {
    /// Render events in input order. Streaming-friendly.
    Sequential,
    /// Collect all events then run DocumentPlan with ByEntity grouping.
    ByEntity,
    /// Collect all events then run DocumentPlan with ByAction grouping.
    ByAction,
}

struct Config {
    vocab_code: bool,
    vocab_git: bool,
    vocab_release: bool,
    vocab_pr: bool,
    strategy: Strategy,
    smart_quotes: bool,
    max_length: Option<usize>,
    explain: bool,
    strictness: Strictness,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vocab_code: true,
            vocab_git: true,
            vocab_release: false,
            vocab_pr: false,
            strategy: Strategy::Sequential,
            smart_quotes: false,
            max_length: None,
            explain: false,
            strictness: Strictness::Strict,
        }
    }
}

fn main() {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nlg: {e}");
            std::process::exit(2);
        }
    };

    if let Err(e) = run(cfg) {
        eprintln!("nlg: {e}");
        std::process::exit(1);
    }
}

fn parse_args() -> Result<Config, String> {
    let mut cfg = Config::default();
    // Expand `--flag=value` into `["--flag", "value"]` so the rest of the
    // parser can use uniform space-separated matching.
    let args: Vec<String> = std::env::args()
        .skip(1)
        .flat_map(|a| {
            if let Some(stripped) = a.strip_prefix("--")
                && let Some((key, val)) = stripped.split_once('=')
            {
                return vec![format!("--{key}"), val.to_string()];
            }
            vec![a]
        })
        .collect();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--smart-quotes" => cfg.smart_quotes = true,
            "--explain" => cfg.explain = true,
            "--strict" => cfg.strictness = Strictness::Strict,
            "--lenient" => cfg.strictness = Strictness::Lenient,
            "--silent" => cfg.strictness = Strictness::Silent,
            "--vocab" => {
                i += 1;
                let list = args.get(i).ok_or("--vocab requires a value")?.clone();
                let (mut want_code, mut want_git, mut want_release, mut want_pr) =
                    (false, false, false, false);
                for v in list.split(',') {
                    match v.trim() {
                        "code" => want_code = true,
                        "git" => want_git = true,
                        "release" => want_release = true,
                        "pr" => want_pr = true,
                        "both" | "all" => {
                            want_code = true;
                            want_git = true;
                            want_release = true;
                            want_pr = true;
                        }
                        "none" => {}
                        other => {
                            return Err(format!(
                                "unknown vocab `{other}` — expected code, git, release, pr, all, or none"
                            ));
                        }
                    }
                }
                cfg.vocab_code = want_code;
                cfg.vocab_git = want_git;
                cfg.vocab_release = want_release;
                cfg.vocab_pr = want_pr;
            }
            "--strategy" => {
                i += 1;
                let v = args.get(i).ok_or("--strategy requires a value")?.clone();
                cfg.strategy = match v.as_str() {
                    "sequential" | "seq" => Strategy::Sequential,
                    "by-entity" | "entity" => Strategy::ByEntity,
                    "by-action" | "action" => Strategy::ByAction,
                    other => {
                        return Err(format!(
                            "unknown strategy `{other}` — expected sequential, by-entity, by-action"
                        ));
                    }
                };
            }
            "--max-length" => {
                i += 1;
                let v = args.get(i).ok_or("--max-length requires a value")?;
                cfg.max_length = Some(
                    v.parse()
                        .map_err(|_| format!("--max-length: `{v}` is not a valid number"))?,
                );
            }
            "--preset" => {
                i += 1;
                let v = args.get(i).ok_or("--preset requires a value")?.clone();
                match v.as_str() {
                    "changelog" => {
                        cfg.vocab_code = true;
                        cfg.vocab_git = true;
                        cfg.vocab_release = true;
                        cfg.strategy = Strategy::ByAction;
                        cfg.max_length = Some(120);
                    }
                    "release-notes" => {
                        cfg.vocab_release = true;
                        cfg.strategy = Strategy::ByAction;
                        cfg.smart_quotes = true;
                    }
                    "digest" => {
                        cfg.vocab_code = true;
                        cfg.vocab_git = true;
                        cfg.vocab_release = true;
                        cfg.vocab_pr = true;
                        cfg.strategy = Strategy::ByEntity;
                        cfg.max_length = Some(100);
                    }
                    other => {
                        return Err(format!(
                            "unknown preset `{other}` — expected changelog, release-notes, or digest"
                        ));
                    }
                }
            }
            other => return Err(format!("unknown argument `{other}` (try --help)")),
        }
        i += 1;
    }
    Ok(cfg)
}

fn print_help() {
    let help = "nlg — natural language generation from JSON-lines events

USAGE:
    nlg [OPTIONS] < events.jsonl

OPTIONS:
    --preset <name>        Apply a named preset (changelog, release-notes, digest).
                           Sets vocab, strategy, and other defaults; explicit flags override.
                             changelog     — vocab: code,git,release  strategy: by-action  max-length: 120
                             release-notes — vocab: release           strategy: by-action  smart-quotes: on
                             digest        — vocab: code,git,release,pr  strategy: by-entity  max-length: 100
    --vocab <list>         Vocab modules: code, git, release, pr, all, none (default: code,git)
    --strategy <mode>      Batch mode: sequential, by-entity, by-action (default: sequential)
    --smart-quotes         Enable typographic quote substitution
    --max-length <N>       Cap each sentence at N characters
    --explain              Emit a JSON RenderExplanation per line instead of text
    --strict | --lenient | --silent
                           Missing-slot behavior (default: strict)
    -h, --help             Print this help

INPUT:
    Each line of stdin is one JSON event object with a `key` field naming
    the template, plus slot fields matching the template's placeholders.
    Integers, strings, and arrays of strings are supported.
";
    print!("{help}");
}

fn run(cfg: Config) -> Result<(), String> {
    let mut engine = build_engine(&cfg);
    if cfg.vocab_code {
        nlg_vocab_code::register(&mut engine).map_err(|e| format!("vocab-code: {e}"))?;
    }
    if cfg.vocab_git {
        nlg_vocab_git::register(&mut engine).map_err(|e| format!("vocab-git: {e}"))?;
    }
    if cfg.vocab_release {
        nlg_vocab_release::register(&mut engine).map_err(|e| format!("vocab-release: {e}"))?;
    }
    if cfg.vocab_pr {
        nlg_vocab_pr::register(&mut engine).map_err(|e| format!("vocab-pr: {e}"))?;
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    match cfg.strategy {
        Strategy::Sequential => run_sequential(&engine, &mut out, stdin.lock(), &cfg),
        Strategy::ByEntity => {
            run_document_plan(&engine, &mut out, stdin.lock(), GroupingStrategy::ByEntity, &cfg)
        }
        Strategy::ByAction => {
            run_document_plan(&engine, &mut out, stdin.lock(), GroupingStrategy::ByAction, &cfg)
        }
    }
}

fn build_engine(cfg: &Config) -> Engine {
    let mut e = Engine::new(English::new()).strictness(cfg.strictness);
    if cfg.smart_quotes {
        e = e.smart_quotes(true);
    }
    if let Some(n) = cfg.max_length {
        e = e.max_sentence_length(n);
    }
    e
}

fn run_sequential<R: BufRead, W: Write>(
    engine: &Engine,
    out: &mut W,
    input: R,
    cfg: &Config,
) -> Result<(), String> {
    let mut session = Session::new();
    for line in input.lines() {
        let line = line.map_err(|e| format!("reading stdin: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }

        let raw: RawEvent = serde_json::from_str(&line)
            .map_err(|e| format!("parsing line `{line}`: {e}"))?;
        let ctx = context_from_slots(&raw.slots)?;

        if cfg.explain {
            let exp = engine
                .render_explained(&mut session, &raw.key, &ctx)
                .map_err(|e| format!("rendering `{}`: {e}", raw.key))?;
            let json = serde_json::to_string(&exp)
                .map_err(|e| format!("serializing explanation: {e}"))?;
            writeln!(out, "{json}").map_err(|e| format!("writing stdout: {e}"))?;
        } else {
            let rendered = engine
                .render(&mut session, &raw.key, &ctx)
                .map_err(|e| format!("rendering `{}`: {e}", raw.key))?;
            writeln!(out, "{rendered}").map_err(|e| format!("writing stdout: {e}"))?;
        }
    }
    Ok(())
}

fn run_document_plan<R: BufRead, W: Write>(
    engine: &Engine,
    out: &mut W,
    input: R,
    strategy: GroupingStrategy,
    _cfg: &Config,
) -> Result<(), String> {
    let mut events: Vec<(String, Context)> = Vec::new();
    for line in input.lines() {
        let line = line.map_err(|e| format!("reading stdin: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let raw: RawEvent = serde_json::from_str(&line)
            .map_err(|e| format!("parsing line `{line}`: {e}"))?;
        let ctx = context_from_slots(&raw.slots)?;
        events.push((raw.key, ctx));
    }

    let event_refs: Vec<(&str, Context)> = events
        .iter()
        .map(|(k, c)| (k.as_str(), c.clone()))
        .collect();

    let plan = DocumentPlan::from_events_grouped(&event_refs, engine, strategy);
    let mut session = Session::new();
    let narrative = plan
        .render(engine, &mut session)
        .map_err(|e: NlgError| format!("rendering plan: {e}"))?;
    writeln!(out, "{narrative}").map_err(|e| format!("writing stdout: {e}"))?;
    Ok(())
}

fn context_from_slots(
    slots: &std::collections::HashMap<String, serde_json::Value>,
) -> Result<Context, String> {
    let mut ctx = Context::new();
    for (k, v) in slots {
        let value = match v {
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Number(n) if n.is_i64() => Value::Number(n.as_i64().unwrap()),
            serde_json::Value::Number(n) if n.is_u64() => {
                let u = n.as_u64().unwrap();
                Value::Number(i64::try_from(u).map_err(|_| {
                    format!("field `{k}`: number {u} exceeds i64 range")
                })?)
            }
            serde_json::Value::Bool(b) => Value::Number(if *b { 1 } else { 0 }),
            serde_json::Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for (idx, item) in items.iter().enumerate() {
                    let s = item.as_str().ok_or_else(|| {
                        format!("field `{k}[{idx}]`: only string arrays are supported")
                    })?;
                    out.push(s.to_string());
                }
                Value::List(out)
            }
            serde_json::Value::Null => continue,
            other => {
                return Err(format!(
                    "field `{k}`: unsupported JSON type `{other:?}` (use string, integer, or array of strings)"
                ));
            }
        };
        ctx.insert(k.clone(), value);
    }
    Ok(ctx)
}
