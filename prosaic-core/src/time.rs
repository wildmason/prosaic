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

/// Format a **positive-is-later** inter-event delta in seconds as a
/// narrative inter-event phrase.
///
/// Use when you want "the next day" / "moments later" style prose rather
/// than "3 days ago" style absolute relative phrases.
///
/// Zero or negative input (same moment or earlier) returns
/// `"at the same time"` — the caller is responsible for ordering.
pub fn format_since_last(diff_secs: i64) -> String {
    if diff_secs <= 0 {
        return "at the same time".to_string();
    }

    if diff_secs < 60 {
        return "moments later".to_string();
    }

    if diff_secs < HOUR {
        let n = (diff_secs + MINUTE / 2) / MINUTE;
        let n = n.max(1);
        return match n {
            1 => "a minute later".to_string(),
            _ => format!("{n} minutes later"),
        };
    }

    if diff_secs < DAY {
        let n = (diff_secs + HOUR / 2) / HOUR;
        let n = n.max(1);
        // Below 6h → "N hours later", 6h..DAY → "later that day".
        if n < 6 {
            return match n {
                1 => "an hour later".to_string(),
                _ => format!("{n} hours later"),
            };
        }
        return "later that day".to_string();
    }

    if diff_secs < 2 * DAY {
        return "the next day".to_string();
    }

    if diff_secs < WEEK {
        let n = diff_secs / DAY;
        return format!("{n} days later");
    }

    if diff_secs < 2 * WEEK {
        return "the following week".to_string();
    }

    if diff_secs < MONTH {
        let n = diff_secs / WEEK;
        return format!("{n} weeks later");
    }

    if diff_secs < 2 * MONTH {
        return "the following month".to_string();
    }

    if diff_secs < YEAR {
        let n = diff_secs / MONTH;
        return format!("{n} months later");
    }

    if diff_secs < 2 * YEAR {
        return "the following year".to_string();
    }

    let n = diff_secs / YEAR;
    format!("{n} years later")
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

    // ── format_since_last ─────────────────────────────────────────────────────

    #[test]
    fn since_last_zero_or_negative_is_at_the_same_time() {
        assert_eq!(format_since_last(0), "at the same time");
        assert_eq!(format_since_last(-1), "at the same time");
        assert_eq!(format_since_last(-3600), "at the same time");
    }

    #[test]
    fn since_last_sub_minute_is_moments_later() {
        assert_eq!(format_since_last(1), "moments later");
        assert_eq!(format_since_last(59), "moments later");
    }

    #[test]
    fn since_last_one_minute() {
        assert_eq!(format_since_last(60), "a minute later");
        // 89s rounds down to 1 minute; 90s rounds up to 2.
        assert_eq!(format_since_last(89), "a minute later");
        assert_eq!(format_since_last(90), "2 minutes later");
    }

    #[test]
    fn since_last_minutes() {
        assert_eq!(format_since_last(2 * MINUTE), "2 minutes later");
        assert_eq!(format_since_last(3599), "60 minutes later");
    }

    #[test]
    fn since_last_one_hour() {
        assert_eq!(format_since_last(HOUR), "an hour later");
        assert_eq!(format_since_last(HOUR + MINUTE * 25), "an hour later"); // rounds to 1
    }

    #[test]
    fn since_last_hours() {
        assert_eq!(format_since_last(2 * HOUR), "2 hours later");
        assert_eq!(format_since_last(5 * HOUR), "5 hours later");
    }

    #[test]
    fn since_last_later_that_day() {
        assert_eq!(format_since_last(6 * HOUR), "later that day");
        assert_eq!(format_since_last(12 * HOUR), "later that day");
        assert_eq!(format_since_last(23 * HOUR), "later that day");
    }

    #[test]
    fn since_last_the_next_day() {
        assert_eq!(format_since_last(DAY + 1), "the next day");
        assert_eq!(format_since_last(2 * DAY - 1), "the next day");
    }

    #[test]
    fn since_last_days() {
        assert_eq!(format_since_last(3 * DAY), "3 days later");
        assert_eq!(format_since_last(6 * DAY), "6 days later");
    }

    #[test]
    fn since_last_the_following_week() {
        assert_eq!(format_since_last(WEEK + 1), "the following week");
        assert_eq!(format_since_last(13 * DAY), "the following week");
    }

    #[test]
    fn since_last_weeks() {
        assert_eq!(format_since_last(3 * WEEK), "3 weeks later");
    }

    #[test]
    fn since_last_the_following_month() {
        assert_eq!(format_since_last(MONTH + 1), "the following month");
    }

    #[test]
    fn since_last_months() {
        assert_eq!(format_since_last(3 * MONTH), "3 months later");
    }

    #[test]
    fn since_last_the_following_year() {
        assert_eq!(format_since_last(YEAR + 1), "the following year");
    }

    #[test]
    fn since_last_years() {
        assert_eq!(format_since_last(3 * YEAR), "3 years later");
    }
}
