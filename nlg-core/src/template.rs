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
    pub fn parse(source: &str) -> Result<Self, NlgError> {
        let mut segments = Vec::new();
        let mut chars = source.char_indices().peekable();
        let mut literal_start = 0;

        while let Some(&(i, ch)) = chars.peek() {
            if ch == '{' {
                // Flush any accumulated literal
                if i > literal_start {
                    segments.push(Segment::Literal(source[literal_start..i].to_string()));
                }

                // Skip the opening brace
                chars.next();

                // Find the closing brace
                let slot_start = i + 1;
                let mut slot_end = None;
                let mut depth = 1;

                for (j, c) in chars.by_ref() {
                    if c == '{' {
                        depth += 1;
                    } else if c == '}' {
                        depth -= 1;
                        if depth == 0 {
                            slot_end = Some(j);
                            break;
                        }
                    }
                }

                let slot_end = slot_end.ok_or_else(|| NlgError::TemplateParseError {
                    template: source.to_string(),
                    position: i,
                    reason: "unclosed `{`".to_string(),
                })?;

                let slot_content = &source[slot_start..slot_end];
                let segment = parse_slot(slot_content, source, i)?;
                segments.push(segment);

                literal_start = slot_end + 1;
            } else {
                chars.next();
            }
        }

        // Flush trailing literal
        if literal_start < source.len() {
            segments.push(Segment::Literal(source[literal_start..].to_string()));
        }

        Ok(Template {
            source: source.to_string(),
            segments,
        })
    }
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
}
