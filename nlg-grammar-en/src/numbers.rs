/// Return the ordinal string for a number (e.g., 1 → "1st", 2 → "2nd").
pub fn ordinal(n: usize) -> String {
    let suffix = match n % 100 {
        11 | 12 | 13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{n}{suffix}")
}

const ONES: &[&str] = &[
    "", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen",
    "sixteen", "seventeen", "eighteen", "nineteen",
];

const TENS: &[&str] = &[
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

const SCALES: &[&str] = &[
    "", "thousand", "million", "billion", "trillion", "quadrillion",
];

/// Spell out a number as English words.
///
/// Examples:
/// - 0 → "zero"
/// - 42 → "forty-two"
/// - 100 → "one hundred"
/// - 1_000 → "one thousand"
/// - 1_234_567 → "one million, two hundred thirty-four thousand, five hundred sixty-seven"
pub fn to_words(n: usize) -> String {
    if n == 0 {
        return "zero".to_string();
    }

    let mut parts: Vec<String> = Vec::new();
    let mut remaining = n;
    let mut scale_index = 0;

    while remaining > 0 {
        let chunk = remaining % 1000;
        if chunk > 0 {
            let chunk_words = chunk_to_words(chunk);
            if scale_index > 0 && scale_index < SCALES.len() {
                parts.push(format!("{} {}", chunk_words, SCALES[scale_index]));
            } else {
                parts.push(chunk_words);
            }
        }
        remaining /= 1000;
        scale_index += 1;
    }

    parts.reverse();
    parts.join(", ")
}

fn chunk_to_words(n: usize) -> String {
    debug_assert!(n > 0 && n < 1000);

    let hundreds = n / 100;
    let remainder = n % 100;

    let mut parts = Vec::new();

    if hundreds > 0 {
        parts.push(format!("{} hundred", ONES[hundreds]));
    }

    if remainder > 0 {
        if remainder < 20 {
            parts.push(ONES[remainder].to_string());
        } else {
            let tens = remainder / 10;
            let ones = remainder % 10;
            if ones > 0 {
                parts.push(format!("{}-{}", TENS[tens], ONES[ones]));
            } else {
                parts.push(TENS[tens].to_string());
            }
        }
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(2), "2nd");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(4), "4th");
        assert_eq!(ordinal(10), "10th");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(ordinal(13), "13th");
        assert_eq!(ordinal(21), "21st");
        assert_eq!(ordinal(22), "22nd");
        assert_eq!(ordinal(23), "23rd");
        assert_eq!(ordinal(100), "100th");
        assert_eq!(ordinal(101), "101st");
        assert_eq!(ordinal(111), "111th");
        assert_eq!(ordinal(112), "112th");
        assert_eq!(ordinal(113), "113th");
    }

    #[test]
    fn zero() {
        assert_eq!(to_words(0), "zero");
    }

    #[test]
    fn single_digits() {
        assert_eq!(to_words(1), "one");
        assert_eq!(to_words(5), "five");
        assert_eq!(to_words(9), "nine");
    }

    #[test]
    fn teens() {
        assert_eq!(to_words(10), "ten");
        assert_eq!(to_words(11), "eleven");
        assert_eq!(to_words(15), "fifteen");
        assert_eq!(to_words(19), "nineteen");
    }

    #[test]
    fn tens() {
        assert_eq!(to_words(20), "twenty");
        assert_eq!(to_words(30), "thirty");
        assert_eq!(to_words(42), "forty-two");
        assert_eq!(to_words(99), "ninety-nine");
    }

    #[test]
    fn hundreds() {
        assert_eq!(to_words(100), "one hundred");
        assert_eq!(to_words(200), "two hundred");
        assert_eq!(to_words(123), "one hundred twenty-three");
        assert_eq!(to_words(501), "five hundred one");
        assert_eq!(to_words(999), "nine hundred ninety-nine");
    }

    #[test]
    fn thousands() {
        assert_eq!(to_words(1_000), "one thousand");
        assert_eq!(to_words(2_500), "two thousand, five hundred");
        assert_eq!(to_words(10_000), "ten thousand");
        assert_eq!(to_words(42_000), "forty-two thousand");
    }

    #[test]
    fn large_numbers() {
        assert_eq!(
            to_words(1_234_567),
            "one million, two hundred thirty-four thousand, five hundred sixty-seven"
        );
    }

    #[test]
    fn million() {
        assert_eq!(to_words(1_000_000), "one million");
    }

    #[test]
    fn billion() {
        assert_eq!(to_words(1_000_000_000), "one billion");
    }
}
