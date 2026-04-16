//! Sentence-length budgeting.
//!
//! When a rendered sentence exceeds the engine's configured budget,
//! split it at a natural boundary (subordinate clause, list prefix,
//! em-dash) and wrap the tail as its own follow-up sentence. The tail
//! gets a lightweight transform so it stands on its own grammatically
//! (`, which impacts 6 consumers` → `This impacts 6 consumers.`).

/// Try to split `sentence` so each piece fits within `max_chars`.
///
/// Returns the input unchanged if it's already short enough, or if no
/// natural split point exists inside the budget. The returned string
/// joins fragments with `" "`, so downstream sentence-termination and
/// cleanup still work.
pub fn split_long(sentence: &str, max_chars: usize) -> String {
    let mut s = sentence.to_string();
    split_long_in_place(&mut s, max_chars);
    s
}

/// In-place version of [`split_long`]. Mutates `output` so each piece fits
/// within `max_chars`, splicing tail sentences back onto the buffer.
pub(crate) fn split_long_in_place(output: &mut String, max_chars: usize) {
    if output.chars().count() <= max_chars {
        return;
    }

    // Upper bound on where we'll look for a split — allow slight
    // overflow rather than aggressively shrinking below budget.
    let search_end = (max_chars + 40).min(output.len());
    let window = &output[..search_end];

    // Ordered by priority: longer/more-specific markers first.
    let candidates: &[(&str, ContinuationKind)] = &[
        (", which ", ContinuationKind::Which),
        (", affecting ", ContinuationKind::Affecting),
        (", impacting ", ContinuationKind::Impacting),
        (", requiring ", ContinuationKind::Requiring),
        (" including ", ContinuationKind::Including),
        (" — ", ContinuationKind::Dash),
        (". ", ContinuationKind::Sentence),
    ];

    // Find the latest acceptable split point (highest byte index) so we
    // keep the first sentence as substantive as possible.
    let mut best: Option<(usize, usize, ContinuationKind)> = None;
    for (marker, kind) in candidates {
        if let Some(idx) = window.rfind(marker) {
            // Don't split if the marker is at the very start — that
            // would leave the first half empty.
            if idx == 0 {
                continue;
            }
            let end = idx + marker.len();
            match best {
                Some((prev_idx, _, _)) if prev_idx >= idx => {}
                _ => best = Some((idx, end, *kind)),
            }
        }
    }

    let (split_at, tail_start, kind) = match best {
        Some(b) => b,
        None => return,
    };

    // Split the tail off the buffer.
    let tail_raw = output[tail_start..].trim_start().to_string();
    // Trim the head.
    let head_end = output[..split_at].trim_end_matches([',', ' ']).len();
    output.truncate(head_end);

    // Rewrite the tail so it stands alone grammatically.
    let mut tail_buf = rewrite_tail(&tail_raw, kind);

    // Recursively split the tail in case it's still too long.
    split_long_in_place(&mut tail_buf, max_chars);

    // Append ". " + tail onto the head.
    output.push('.');
    output.push(' ');
    output.push_str(&tail_buf);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContinuationKind {
    /// ", which <verb> …" — rewrite so the tail starts with "This <verb>".
    Which,
    /// ", affecting …" — tail becomes "This affects …".
    Affecting,
    /// ", impacting …" — tail becomes "This impacts …".
    Impacting,
    /// ", requiring …" — tail becomes "This requires …".
    Requiring,
    /// " including …" — tail becomes "Including …" capitalized.
    Including,
    /// " — …" em-dash — tail capitalized on its own.
    Dash,
    /// Already sentence-terminated; just take the tail as the next sentence.
    Sentence,
}

fn rewrite_tail(tail: &str, kind: ContinuationKind) -> String {
    match kind {
        ContinuationKind::Which => {
            // "which impacts 6 consumers" → "This impacts 6 consumers"
            // The verb right after "which" stays as-is; we just replace
            // the relative pronoun with "This".
            format!("This {tail}")
        }
        ContinuationKind::Affecting => format!("This affects {tail}"),
        ContinuationKind::Impacting => format!("This impacts {tail}"),
        ContinuationKind::Requiring => format!("This requires {tail}"),
        ContinuationKind::Including => {
            // "ProfileComponent, SettingsComponent, …" → "Including
            // ProfileComponent, …." Keeps the marker word as the
            // sentence head so the list still reads naturally.
            format!("Including {tail}")
        }
        ContinuationKind::Dash | ContinuationKind::Sentence => capitalize_first(tail),
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let mut out = String::with_capacity(s.len());
            for upper in c.to_uppercase() {
                out.push(upper);
            }
            out.extend(chars);
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_budget_returns_unchanged() {
        let s = "short sentence";
        assert_eq!(split_long(s, 80), s);
    }

    #[test]
    fn splits_on_which_marker() {
        let s = "The class UserService was renamed to AccountService, \
                 which impacts 6 consumers";
        let out = split_long(s, 60);
        assert!(out.contains("This impacts 6 consumers"), "got: {out}");
        assert!(
            out.starts_with("The class UserService was renamed"),
            "got: {out}"
        );
    }

    #[test]
    fn splits_on_including_marker() {
        let s = "The method processOrder was modified, which may affect \
                 5 consumers including CartComponent, CheckoutFlow, \
                 OrderHistory, ProfilePage, AdminView";
        let out = split_long(s, 80);
        assert!(out.contains("Including"), "got: {out}");
    }

    #[test]
    fn splits_on_affecting_marker() {
        let s = "AuthGuard was modified, affecting 3 routes Dashboard, \
                 Settings, Admin";
        let out = split_long(s, 35);
        assert!(out.contains("This affects 3 routes"), "got: {out}");
    }

    #[test]
    fn no_split_when_no_natural_boundary() {
        let s = "Averyverylongrunningstringwithnospacesandnowordsseparated";
        let out = split_long(s, 20);
        // Nowhere natural to split — we return unchanged rather than
        // chop mid-word.
        assert_eq!(out, s);
    }

    #[test]
    fn recursive_split_handles_multi_long_sentence() {
        let s = "The class UserService was renamed to AccountService, \
                 which impacts 12 consumers including Alpha, Bravo, \
                 Charlie, Delta, Echo, Foxtrot, Golf, Hotel, India, Juliet";
        let out = split_long(s, 60);
        // Should split at least once at a natural boundary.
        assert!(out.matches(". ").count() >= 1, "got: {out}");
    }
}
