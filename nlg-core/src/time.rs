//! Time-aware framing helpers.
//!
//! Convert a signed duration (in seconds) — typically `now - timestamp`
//! — into a natural English phrase like `"yesterday"`, `"3 weeks ago"`,
//! or `"in 2 months"`. Language-agnostic math, English-flavoured
//! wording; move to the `Language` trait if we grow multilingual.
//!
//! The surface is purely functional: callers compute a diff and pass
//! it in. The engine stores a reference time (default: `SystemTime::now()`)
//! and feeds the `{timestamp|relative}` pipe from it.

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const WEEK: i64 = 7 * DAY;
const MONTH: i64 = 30 * DAY; // approximate — fine for relative framing
const YEAR: i64 = 365 * DAY;

/// Format a positive-is-past difference in seconds as a natural English
/// phrase. Positive values mean the target is in the past (`"yesterday"`),
/// negative values mean the future (`"tomorrow"`), zero means right now.
pub fn format_relative(diff_secs: i64) -> String {
    let past = diff_secs >= 0;
    let abs = diff_secs.abs();

    // Very recent: "just now" covers ~45 seconds in either direction.
    if abs < 45 {
        return if past {
            "just now".to_string()
        } else {
            "any moment now".to_string()
        };
    }

    // Minutes
    if abs < HOUR {
        let n = (abs + MINUTE / 2) / MINUTE;
        let n = n.max(1);
        return phrase(past, &format!("{n} minute{s} ago", s = s(n)), &format!("in {n} minute{s}", s = s(n)));
    }

    // Hours
    if abs < DAY {
        let n = (abs + HOUR / 2) / HOUR;
        let n = n.max(1);
        return phrase(
            past,
            &match n {
                1 => "an hour ago".to_string(),
                _ => format!("{n} hours ago"),
            },
            &match n {
                1 => "in an hour".to_string(),
                _ => format!("in {n} hours"),
            },
        );
    }

    // Yesterday / tomorrow
    if abs < 2 * DAY {
        return phrase(past, "yesterday", "tomorrow");
    }

    // Days
    if abs < WEEK {
        let n = abs / DAY;
        return phrase(
            past,
            &format!("{n} days ago"),
            &format!("in {n} days"),
        );
    }

    // Last/next week
    if abs < 2 * WEEK {
        return phrase(past, "last week", "next week");
    }

    // Weeks
    if abs < MONTH {
        let n = abs / WEEK;
        return phrase(
            past,
            &format!("{n} weeks ago"),
            &format!("in {n} weeks"),
        );
    }

    // Last/next month
    if abs < 2 * MONTH {
        return phrase(past, "last month", "next month");
    }

    // Months
    if abs < YEAR {
        let n = abs / MONTH;
        return phrase(
            past,
            &format!("{n} months ago"),
            &format!("in {n} months"),
        );
    }

    // Last/next year
    if abs < 2 * YEAR {
        return phrase(past, "last year", "next year");
    }

    // Years
    let n = abs / YEAR;
    phrase(past, &format!("{n} years ago"), &format!("in {n} years"))
}

fn s(n: i64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn phrase(past: bool, past_form: &str, future_form: &str) -> String {
    if past {
        past_form.to_string()
    } else {
        future_form.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn just_now_for_small_past_and_future() {
        assert_eq!(format_relative(0), "just now");
        assert_eq!(format_relative(30), "just now");
        assert_eq!(format_relative(-30), "any moment now");
    }

    #[test]
    fn minutes() {
        assert_eq!(format_relative(60), "1 minute ago");
        assert_eq!(format_relative(300), "5 minutes ago");
        assert_eq!(format_relative(-600), "in 10 minutes");
    }

    #[test]
    fn hours() {
        assert_eq!(format_relative(3600), "an hour ago");
        assert_eq!(format_relative(3 * 3600), "3 hours ago");
        assert_eq!(format_relative(-3600), "in an hour");
    }

    #[test]
    fn yesterday_and_tomorrow() {
        assert_eq!(format_relative(DAY + 3600), "yesterday");
        assert_eq!(format_relative(-(DAY + 3600)), "tomorrow");
    }

    #[test]
    fn days() {
        assert_eq!(format_relative(3 * DAY), "3 days ago");
        assert_eq!(format_relative(-5 * DAY), "in 5 days");
    }

    #[test]
    fn last_week() {
        assert_eq!(format_relative(WEEK + DAY), "last week");
        assert_eq!(format_relative(-(WEEK + DAY)), "next week");
    }

    #[test]
    fn weeks() {
        assert_eq!(format_relative(3 * WEEK), "3 weeks ago");
    }

    #[test]
    fn months_and_years() {
        assert_eq!(format_relative(2 * MONTH), "2 months ago");
        assert_eq!(format_relative(3 * YEAR), "3 years ago");
        assert_eq!(format_relative(-(2 * YEAR + DAY)), "in 2 years");
    }

    #[test]
    fn last_month_and_next_month() {
        assert_eq!(format_relative(MONTH + DAY), "last month");
        assert_eq!(format_relative(-(MONTH + DAY)), "next month");
    }
}
