use crate::error::NlgError;

/// An argument passed to a pipe transform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeArg {
    String(String),
    Number(usize),
}

/// A pipe transform applied to a slot value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipe {
    pub name: String,
    pub arg: Option<PipeArg>,
}

/// A segment of a parsed template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Literal text, rendered as-is.
    Literal(String),
    /// A slot referencing a context key, with optional pipe transforms.
    Slot { key: String, pipes: Vec<Pipe> },
    /// A conditional section: only renders if the condition key is truthy.
    /// Truthy means: non-zero number, non-empty list, non-empty string.
    Conditional {
        condition_key: String,
        inner: Vec<Segment>,
    },
    /// A partial inclusion `{>name}` — expands at render time using a
    /// partial registered via `engine.register_partial`. Enables sharing
    /// template fragments across many templates (e.g. a trailing
    /// "affecting N consumers" clause reused across vocab entries).
    Partial { name: String },
}

/// A parsed template ready for rendering.
#[derive(Debug, Clone)]
pub struct Template {
    pub source: String,
    pub segments: Vec<Segment>,
}

impl Template {
    /// Parse a template string into segments.
    ///
    /// Syntax:
    /// - `{key}` — substitute value from context
    /// - `{key|pipe}` — apply a pipe transform
    /// - `{key|pipe:arg}` — pipe with an argument
    /// - `{key|pipe1|pipe2:arg}` — chained pipes
    /// - `{?key}...{/?}` — conditional section (renders only if `key` is truthy)
    pub fn parse(source: &str) -> Result<Self, NlgError> {
        let segments = parse_segments(source, 0, source.len())?;
        Ok(Template {
            source: source.to_string(),
            segments,
        })
    }

    /// Return the text of every literal segment in this template.
    ///
    /// Walks the segment tree recursively, collecting text from
    /// literal nodes at every nesting depth (including
    /// inside conditional sections). Partial-inclusion nodes are
    /// treated as opaque — their literals are only reachable after the
    /// engine expands them at render time. Callers that need partial
    /// content to contribute to faithfulness scoring should pre-expand
    /// partials or disable the gate for partial-heavy templates.
    ///
    /// Used by faithfulness scoring to include template boilerplate in
    /// the entailment source set alongside context values.
    pub fn literal_tokens(&self) -> Vec<&str> {
        let mut out = Vec::new();
        collect_literals(&self.segments, &mut out);
        out
    }

    /// Every slot key referenced by this template, including condition keys
    /// from conditional sections (`{?key}...{/?}`).
    ///
    /// Walks the segment tree recursively. Partial nodes are skipped — their
    /// slot keys are only reachable after the engine expands them at render time.
    /// The returned list may contain duplicates (e.g. when a key appears in both
    /// a conditional guard and its body). Used by the `nlg_template!` proc macro
    /// for compile-time slot validation.
    pub fn slot_keys(&self) -> Vec<String> {
        let mut out = Vec::new();
        collect_slot_keys(&self.segments, &mut out);
        out
    }

    /// Every pipe name referenced by any slot in this template.
    ///
    /// Walks the segment tree recursively. Returns the pipe name only (not any
    /// argument, e.g. `"pluralize"` for `{count|pluralize:item}`). May contain
    /// duplicates if the same pipe appears more than once. Used by the
    /// `nlg_template!` proc macro for compile-time pipe validation.
    pub fn pipe_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        collect_pipe_names(&self.segments, &mut out);
        out
    }
}

/// Recursively collect literal text from a segment list into `out`.
/// `Segment::Partial` nodes are skipped (opaque at parse time).
fn collect_literals<'a>(segments: &'a [Segment], out: &mut Vec<&'a str>) {
    for seg in segments {
        match seg {
            Segment::Literal(s) => out.push(s.as_str()),
            Segment::Slot { .. } => {}
            Segment::Conditional { inner, .. } => collect_literals(inner, out),
            Segment::Partial { .. } => {}
        }
    }
}

/// Recursively collect slot keys (and conditional condition keys) from a segment
/// list into `out`. `Segment::Partial` nodes are skipped (opaque at parse time).
fn collect_slot_keys(segments: &[Segment], out: &mut Vec<String>) {
    for seg in segments {
        match seg {
            Segment::Slot { key, .. } => out.push(key.clone()),
            Segment::Conditional { condition_key, inner } => {
                out.push(condition_key.clone());
                collect_slot_keys(inner, out);
            }
            Segment::Literal(_) | Segment::Partial { .. } => {}
        }
    }
}

/// Recursively collect pipe names from all slot segments into `out`.
/// `Segment::Partial` nodes are skipped (opaque at parse time).
fn collect_pipe_names(segments: &[Segment], out: &mut Vec<String>) {
    for seg in segments {
        match seg {
            Segment::Slot { pipes, .. } => {
                for pipe in pipes {
                    out.push(pipe.name.clone());
                }
            }
            Segment::Conditional { inner, .. } => collect_pipe_names(inner, out),
            Segment::Literal(_) | Segment::Partial { .. } => {}
        }
    }
}

/// Parse a range of source into segments. Handles nested conditionals.
fn parse_segments(source: &str, start: usize, end: usize) -> Result<Vec<Segment>, NlgError> {
    let mut segments = Vec::new();
    let slice = &source[start..end];
    let bytes = slice.as_bytes();
    let mut i: usize = 0;
    let mut literal_start: usize = 0;

    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }

        // Flush accumulated literal
        if i > literal_start {
            segments.push(Segment::Literal(slice[literal_start..i].to_string()));
        }

        let content_start = i + 1;
        let is_conditional = content_start < bytes.len() && bytes[content_start] == b'?';
        let is_partial = content_start < bytes.len() && bytes[content_start] == b'>';
        let is_closing = content_start + 1 < bytes.len()
            && bytes[content_start] == b'/'
            && bytes[content_start + 1] == b'?';

        if is_closing {
            return Err(NlgError::TemplateParseError {
                template: source.to_string(),
                position: start + i,
                reason: "unexpected closing `{/?}` without opening".to_string(),
            });
        }

        if is_partial {
            let name_start = content_start + 1; // after `>`
            let name_end = slice[name_start..]
                .find('}')
                .map(|rel| name_start + rel)
                .ok_or_else(|| NlgError::TemplateParseError {
                    template: source.to_string(),
                    position: start + i,
                    reason: "unclosed `{>`".to_string(),
                })?;

            let name = slice[name_start..name_end].trim().to_string();
            if name.is_empty() {
                return Err(NlgError::TemplateParseError {
                    template: source.to_string(),
                    position: start + i,
                    reason: "empty partial name".to_string(),
                });
            }

            segments.push(Segment::Partial { name });
            i = name_end + 1;
            literal_start = i;
            continue;
        }

        if is_conditional {
            // Parse {?key}...{/?}
            let key_start = content_start + 1; // after `?`
            let key_end = slice[key_start..]
                .find('}')
                .map(|rel| key_start + rel)
                .ok_or_else(|| NlgError::TemplateParseError {
                    template: source.to_string(),
                    position: start + i,
                    reason: "unclosed `{?`".to_string(),
                })?;

            let condition_key = slice[key_start..key_end].trim().to_string();
            if condition_key.is_empty() {
                return Err(NlgError::TemplateParseError {
                    template: source.to_string(),
                    position: start + i,
                    reason: "empty condition key".to_string(),
                });
            }

            let inner_start = key_end + 1;
            let inner_end = find_matching_close(slice, inner_start).ok_or_else(|| {
                NlgError::TemplateParseError {
                    template: source.to_string(),
                    position: start + i,
                    reason: format!("unclosed conditional `{{?{condition_key}}}`"),
                }
            })?;

            let inner_segments =
                parse_segments(source, start + inner_start, start + inner_end)?;

            segments.push(Segment::Conditional {
                condition_key,
                inner: inner_segments,
            });

            // Advance past `{/?}` (4 chars)
            i = inner_end + 4;
            literal_start = i;
        } else {
            // Regular slot — find matching `}` (with `{` nesting support)
            let mut slot_end: Option<usize> = None;
            let mut depth: i32 = 1;
            let mut j = content_start;
            while j < bytes.len() {
                match bytes[j] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            slot_end = Some(j);
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let slot_end = slot_end.ok_or_else(|| NlgError::TemplateParseError {
                template: source.to_string(),
                position: start + i,
                reason: "unclosed `{`".to_string(),
            })?;

            let slot_content = &slice[content_start..slot_end];
            let segment = parse_slot(slot_content, source, start + i)?;
            segments.push(segment);

            i = slot_end + 1;
            literal_start = i;
        }
    }

    // Flush trailing literal
    if literal_start < slice.len() {
        segments.push(Segment::Literal(slice[literal_start..].to_string()));
    }

    Ok(segments)
}

/// Find the position of the matching `{/?}` for a conditional opened at `start`.
/// Handles nested conditionals.
fn find_matching_close(slice: &str, start: usize) -> Option<usize> {
    let mut depth: i32 = 1;
    let bytes = slice.as_bytes();
    let mut i = start;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' {
            if bytes[i + 1] == b'?' {
                depth += 1;
                i += 2;
                continue;
            }
            if i + 2 < bytes.len() && bytes[i + 1] == b'/' && bytes[i + 2] == b'?' {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 3;
                continue;
            }
        }
        i += 1;
    }

    None
}

fn parse_slot(content: &str, source: &str, position: usize) -> Result<Segment, NlgError> {
    let parts: Vec<&str> = content.split('|').collect();

    let key = parts[0].trim();
    if key.is_empty() {
        return Err(NlgError::TemplateParseError {
            template: source.to_string(),
            position,
            reason: "empty slot key".to_string(),
        });
    }

    let mut pipes = Vec::new();
    for part in &parts[1..] {
        let pipe = parse_pipe(part.trim(), source, position)?;
        pipes.push(pipe);
    }

    Ok(Segment::Slot {
        key: key.to_string(),
        pipes,
    })
}

fn parse_pipe(content: &str, source: &str, position: usize) -> Result<Pipe, NlgError> {
    if content.is_empty() {
        return Err(NlgError::TemplateParseError {
            template: source.to_string(),
            position,
            reason: "empty pipe name".to_string(),
        });
    }

    if let Some((name, arg_str)) = content.split_once(':') {
        let name = name.trim();
        let arg_str = arg_str.trim();

        let arg = if let Ok(n) = arg_str.parse::<usize>() {
            PipeArg::Number(n)
        } else {
            PipeArg::String(arg_str.to_string())
        };

        Ok(Pipe {
            name: name.to_string(),
            arg: Some(arg),
        })
    } else {
        Ok(Pipe {
            name: content.to_string(),
            arg: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_literal_only() {
        let t = Template::parse("hello world").unwrap();
        assert_eq!(t.segments, vec![Segment::Literal("hello world".to_string())]);
    }

    #[test]
    fn parse_single_slot() {
        let t = Template::parse("{name}").unwrap();
        assert_eq!(
            t.segments,
            vec![Segment::Slot {
                key: "name".to_string(),
                pipes: vec![],
            }]
        );
    }

    #[test]
    fn parse_slot_with_surrounding_text() {
        let t = Template::parse("Hello {name}!").unwrap();
        assert_eq!(
            t.segments,
            vec![
                Segment::Literal("Hello ".to_string()),
                Segment::Slot {
                    key: "name".to_string(),
                    pipes: vec![],
                },
                Segment::Literal("!".to_string()),
            ]
        );
    }

    #[test]
    fn parse_slot_with_pipe() {
        let t = Template::parse("{name|capitalize}").unwrap();
        assert_eq!(
            t.segments,
            vec![Segment::Slot {
                key: "name".to_string(),
                pipes: vec![Pipe {
                    name: "capitalize".to_string(),
                    arg: None,
                }],
            }]
        );
    }

    #[test]
    fn parse_slot_with_pipe_and_string_arg() {
        let t = Template::parse("{count|pluralize:item}").unwrap();
        assert_eq!(
            t.segments,
            vec![Segment::Slot {
                key: "count".to_string(),
                pipes: vec![Pipe {
                    name: "pluralize".to_string(),
                    arg: Some(PipeArg::String("item".to_string())),
                }],
            }]
        );
    }

    #[test]
    fn parse_slot_with_pipe_and_number_arg() {
        let t = Template::parse("{items|truncate:3}").unwrap();
        assert_eq!(
            t.segments,
            vec![Segment::Slot {
                key: "items".to_string(),
                pipes: vec![Pipe {
                    name: "truncate".to_string(),
                    arg: Some(PipeArg::Number(3)),
                }],
            }]
        );
    }

    #[test]
    fn parse_chained_pipes() {
        let t = Template::parse("{items|truncate:3|join}").unwrap();
        assert_eq!(
            t.segments,
            vec![Segment::Slot {
                key: "items".to_string(),
                pipes: vec![
                    Pipe {
                        name: "truncate".to_string(),
                        arg: Some(PipeArg::Number(3)),
                    },
                    Pipe {
                        name: "join".to_string(),
                        arg: None,
                    },
                ],
            }]
        );
    }

    #[test]
    fn parse_multiple_slots() {
        let t = Template::parse("{a} and {b}").unwrap();
        assert_eq!(
            t.segments,
            vec![
                Segment::Slot {
                    key: "a".to_string(),
                    pipes: vec![],
                },
                Segment::Literal(" and ".to_string()),
                Segment::Slot {
                    key: "b".to_string(),
                    pipes: vec![],
                },
            ]
        );
    }

    #[test]
    fn parse_unclosed_brace_is_error() {
        let result = Template::parse("hello {name");
        assert!(matches!(result, Err(NlgError::TemplateParseError { .. })));
    }

    #[test]
    fn parse_empty_slot_is_error() {
        let result = Template::parse("hello {}");
        assert!(matches!(result, Err(NlgError::TemplateParseError { .. })));
    }

    #[test]
    fn parse_empty_pipe_name_is_error() {
        let result = Template::parse("{name|}");
        assert!(matches!(result, Err(NlgError::TemplateParseError { .. })));
    }

    #[test]
    fn parse_complex_template() {
        let t = Template::parse(
            "The {entity_type} {old_name} was renamed to {new_name} \
             which impacts {count} direct {count|pluralize:consumer} \
             [{consumers|truncate:3|join}]"
        ).unwrap();

        // "The " {entity_type} " " {old_name} " was renamed to " {new_name}
        // " which impacts " {count} " direct " {count|pluralize:consumer}
        // " [" {consumers|truncate:3|join} "]"
        assert_eq!(t.segments.len(), 13);
    }

    // ── Conditional tests ───────────────────────────────────────────────

    #[test]
    fn parse_conditional_section() {
        let t = Template::parse("foo{?count} bar{/?} baz").unwrap();
        assert_eq!(t.segments.len(), 3);
        assert_eq!(t.segments[0], Segment::Literal("foo".into()));
        assert!(matches!(t.segments[1], Segment::Conditional { .. }));
        assert_eq!(t.segments[2], Segment::Literal(" baz".into()));
    }

    #[test]
    fn parse_conditional_with_inner_slot() {
        let t = Template::parse("{name}{?count}, {count} items{/?}").unwrap();
        assert_eq!(t.segments.len(), 2);
        if let Segment::Conditional { condition_key, inner } = &t.segments[1] {
            assert_eq!(condition_key, "count");
            assert_eq!(inner.len(), 3); // ", ", {count}, " items"
        } else {
            panic!("Expected Conditional segment");
        }
    }

    #[test]
    fn parse_unclosed_conditional_is_error() {
        let result = Template::parse("{?count} never closed");
        assert!(matches!(result, Err(NlgError::TemplateParseError { .. })));
    }

    #[test]
    fn parse_empty_conditional_key_is_error() {
        let result = Template::parse("{?}content{/?}");
        assert!(matches!(result, Err(NlgError::TemplateParseError { .. })));
    }

    // ── Partial tests ───────────────────────────────────────────────────

    #[test]
    fn parse_partial_reference() {
        let t = Template::parse("start {>tail} end").unwrap();
        assert_eq!(t.segments.len(), 3);
        assert!(matches!(&t.segments[1], Segment::Partial { name } if name == "tail"));
    }

    #[test]
    fn parse_empty_partial_name_is_error() {
        let result = Template::parse("{>}");
        assert!(matches!(result, Err(NlgError::TemplateParseError { .. })));
    }

    #[test]
    fn parse_unclosed_partial_is_error() {
        let result = Template::parse("{>tail");
        assert!(matches!(result, Err(NlgError::TemplateParseError { .. })));
    }

    // ── literal_tokens tests ────────────────────────────────────────────

    #[test]
    fn literal_tokens_simple() {
        let t = Template::parse("The {type} {name} was modified").unwrap();
        let lits = t.literal_tokens();
        assert_eq!(lits, vec!["The ", " ", " was modified"]);
    }

    #[test]
    fn literal_tokens_from_conditional_sections() {
        let t = Template::parse("{name}{?count}, impacting {count} consumers{/?}").unwrap();
        let lits = t.literal_tokens();
        assert!(lits.iter().any(|l| l.contains("impacting")));
        assert!(lits.iter().any(|l| l.contains("consumers")));
    }

    #[test]
    fn literal_tokens_empty_for_all_slots() {
        let t = Template::parse("{a}{b}{c}").unwrap();
        assert!(t.literal_tokens().is_empty());
    }

    #[test]
    fn literal_tokens_skips_partial_nodes() {
        // Partial nodes are opaque at parse time; their literals are only
        // reachable after engine expansion.
        let t = Template::parse("prefix {>partial_name} suffix").unwrap();
        let lits = t.literal_tokens();
        assert_eq!(lits, vec!["prefix ", " suffix"]);
    }

    #[test]
    fn literal_tokens_nested_conditional_recursion() {
        // Conditional inside conditional should surface literals at all depths.
        let t = Template::parse("{?a}outer{?b} inner{/?}{/?}").unwrap();
        let lits = t.literal_tokens();
        assert!(lits.iter().any(|l| l.contains("outer")));
        assert!(lits.iter().any(|l| l.contains("inner")));
    }

    // ── slot_keys tests ─────────────────────────────────────────────────

    #[test]
    fn slot_keys_simple() {
        let t = Template::parse("{a} and {b}").unwrap();
        let mut keys = t.slot_keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn slot_keys_includes_condition_key() {
        let t = Template::parse("{name}{?count}, {count} items{/?}").unwrap();
        let keys = t.slot_keys();
        assert!(keys.contains(&"name".to_string()));
        // count appears as condition key and as inner slot key
        assert!(keys.iter().filter(|k| k.as_str() == "count").count() >= 2);
    }

    #[test]
    fn slot_keys_skips_partials() {
        let t = Template::parse("start {>partial_name} {slot} end").unwrap();
        let keys = t.slot_keys();
        assert_eq!(keys, vec!["slot"]);
        assert!(!keys.contains(&"partial_name".to_string()));
    }

    #[test]
    fn slot_keys_empty_for_literal_only() {
        let t = Template::parse("just a string").unwrap();
        assert!(t.slot_keys().is_empty());
    }

    #[test]
    fn slot_keys_nested_conditional() {
        let t = Template::parse("{?a}outer{?b} inner{/?}{/?}").unwrap();
        let mut keys = t.slot_keys();
        keys.sort();
        keys.dedup();
        assert_eq!(keys, vec!["a", "b"]);
    }

    // ── pipe_names tests ─────────────────────────────────────────────────

    #[test]
    fn pipe_names_simple() {
        let t = Template::parse("{count|pluralize:item}").unwrap();
        assert_eq!(t.pipe_names(), vec!["pluralize"]);
    }

    #[test]
    fn pipe_names_chained() {
        let t = Template::parse("{items|truncate:3|join}").unwrap();
        assert_eq!(t.pipe_names(), vec!["truncate", "join"]);
    }

    #[test]
    fn pipe_names_empty_when_no_pipes() {
        let t = Template::parse("{name} and {other}").unwrap();
        assert!(t.pipe_names().is_empty());
    }

    #[test]
    fn pipe_names_inside_conditional() {
        let t = Template::parse("{?count}{count|pluralize:item}{/?}").unwrap();
        assert_eq!(t.pipe_names(), vec!["pluralize"]);
    }

    #[test]
    fn pipe_names_arg_not_included_in_name() {
        // pipe name is "truncate", not "truncate:3"
        let t = Template::parse("{items|truncate:3}").unwrap();
        let names = t.pipe_names();
        assert_eq!(names, vec!["truncate"]);
        assert!(!names.iter().any(|n| n.contains(':')));
    }
}
