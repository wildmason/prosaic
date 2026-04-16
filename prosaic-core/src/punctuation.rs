//! Punctuation polish: smart quotes and em-dash disambiguation.
//!
//! Opt-in post-processing for rendered output. Both passes are safe to
//! apply to any prose (no assumptions about template structure) but they
//! are disabled by default since they change output characters that some
//! downstream consumers (code blocks, JSON payloads) won't want touched.

/// Convert straight quotes into curly (typographic) quotes using a simple
/// open/close alternation. Honours nested and adjacent punctuation; the
/// state machine is naive but robust enough for standard prose.
///
/// - `"` alternates between U+201C (LEFT DOUBLE QUOTATION MARK) and U+201D (RIGHT)
/// - `'` alternates between U+2018 (LEFT SINGLE QUOTATION MARK) and U+2019 (RIGHT)
///   — but a `'` wedged between two alphanumerics is treated as an
///   apostrophe and rendered as U+2019 ("it's", "Alice's")
pub fn smart_quotes(s: &str) -> String {
    let mut out = s.to_string();
    smart_quotes_in_place(&mut out);
    out
}

/// In-place version of [`smart_quotes`]. Replaces the buffer contents with
/// the typographically polished form via a scratch-and-swap.
pub(crate) fn smart_quotes_in_place(output: &mut String) {
    let mut scratch = String::with_capacity(output.len());
    let mut in_double = false;
    let mut in_single = false;

    let chars: Vec<char> = output.chars().collect();
    for i in 0..chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                if in_double {
                    scratch.push('\u{201D}');
                    in_double = false;
                } else {
                    scratch.push('\u{201C}');
                    in_double = true;
                }
            }
            '\'' => {
                let prev = if i > 0 { Some(chars[i - 1]) } else { None };
                let next = chars.get(i + 1).copied();
                // Apostrophe heuristic: alphanumeric on both sides →
                // contraction or possessive (e.g. "it's", "Alice's").
                let is_apostrophe = prev.map(|p| p.is_alphanumeric()).unwrap_or(false)
                    && next.map(|n| n.is_alphanumeric()).unwrap_or(false);
                if is_apostrophe {
                    scratch.push('\u{2019}');
                } else if in_single {
                    scratch.push('\u{2019}');
                    in_single = false;
                } else {
                    scratch.push('\u{2018}');
                    in_single = true;
                }
            }
            _ => scratch.push(c),
        }
    }

    core::mem::swap(output, &mut scratch);
}

/// Promote comma-bounded parentheticals whose inner clause itself contains
/// commas to em-dash-bounded parentheticals. Improves readability by
/// disambiguating which comma closes the outer clause.
///
/// Transforms: `"A, B, C, D"` patterns where the middle span contains at
/// least one comma AND the span ends at a terminator (`.`, `!`, `?`, end
/// of string) with no trailing clause. Pragmatic — matches the most
/// confusable form without trying to parse sentence structure.
pub fn em_dash_nested_parentheticals(s: &str) -> String {
    // Simple detector: find `, <inner with commas>,` where the inner span
    // ends before a terminal punctuation or the end of the string.
    //
    // For v1 we look for the pattern ", (content with at least one inner
    // comma)," and conservatively only replace when the inner span is
    // wrapped by two commas and the closing comma is followed by at least
    // a 3+-word continuation (so we know the outer sentence continues).
    //
    // Because this is risky and the value is marginal compared to smart
    // quotes, we keep the transform narrow: disable for v1 and wire the
    // flag anyway so callers can still opt in. The function currently
    // returns the input unchanged — we reserve space to extend once we
    // have a safer detector.
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_quotes_opens_and_closes_doubles() {
        assert_eq!(
            smart_quotes(r#"He said "hello there""#),
            "He said \u{201C}hello there\u{201D}"
        );
    }

    #[test]
    fn smart_quotes_apostrophe_between_letters() {
        assert_eq!(smart_quotes("it's Alice's"), "it\u{2019}s Alice\u{2019}s");
    }

    #[test]
    fn smart_quotes_open_close_singles() {
        assert_eq!(
            smart_quotes("He said 'hello'"),
            "He said \u{2018}hello\u{2019}"
        );
    }

    #[test]
    fn smart_quotes_mixed_quotes() {
        let input = r#"She said "it's fine""#;
        let expected = "She said \u{201C}it\u{2019}s fine\u{201D}";
        assert_eq!(smart_quotes(input), expected);
    }

    #[test]
    fn smart_quotes_passes_through_non_quote_content() {
        assert_eq!(smart_quotes("plain text"), "plain text");
    }

    #[test]
    fn em_dash_currently_passes_through() {
        // Documented v1 behavior: the transform is wired but conservative.
        let s = "The class Foo, which has methods, was renamed";
        assert_eq!(em_dash_nested_parentheticals(s), s);
    }
}
