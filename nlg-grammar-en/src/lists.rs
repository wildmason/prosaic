use nlg_core::Conjunction;

/// Join a list of items with Oxford comma formatting.
///
/// - 0 items: `""`
/// - 1 item: `"a"`
/// - 2 items: `"a and b"`
/// - 3+ items: `"a, b, and c"` (Oxford comma)
pub fn join_list(items: &[&str], conjunction: Conjunction) -> String {
    let conj = match conjunction {
        Conjunction::And => "and",
        Conjunction::Or => "or",
    };

    match items.len() {
        0 => String::new(),
        1 => items[0].to_string(),
        2 => format!("{} {} {}", items[0], conj, items[1]),
        _ => {
            let all_but_last = &items[..items.len() - 1];
            let last = items[items.len() - 1];
            format!("{}, {} {}", all_but_last.join(", "), conj, last)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_list() {
        assert_eq!(join_list(&[], Conjunction::And), "");
    }

    #[test]
    fn single_item() {
        assert_eq!(join_list(&["alpha"], Conjunction::And), "alpha");
    }

    #[test]
    fn two_items_and() {
        assert_eq!(join_list(&["alpha", "beta"], Conjunction::And), "alpha and beta");
    }

    #[test]
    fn two_items_or() {
        assert_eq!(join_list(&["alpha", "beta"], Conjunction::Or), "alpha or beta");
    }

    #[test]
    fn three_items_oxford_comma() {
        assert_eq!(
            join_list(&["alpha", "beta", "gamma"], Conjunction::And),
            "alpha, beta, and gamma"
        );
    }

    #[test]
    fn three_items_or_oxford_comma() {
        assert_eq!(
            join_list(&["alpha", "beta", "gamma"], Conjunction::Or),
            "alpha, beta, or gamma"
        );
    }

    #[test]
    fn four_items() {
        assert_eq!(
            join_list(&["a", "b", "c", "d"], Conjunction::And),
            "a, b, c, and d"
        );
    }

    #[test]
    fn many_items() {
        assert_eq!(
            join_list(&["a", "b", "c", "d", "e", "f"], Conjunction::And),
            "a, b, c, d, e, and f"
        );
    }
}
