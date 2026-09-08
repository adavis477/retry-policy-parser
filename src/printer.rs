use std::time::Duration;

use crate::policy::{Backoff, Jitter, RetryPolicy};

/// Renders a policy back to its canonical text form.
///
/// The output always lists fields in the same order (`max_attempts`,
/// `backoff`, `jitter`) and picks the largest whole unit for each duration,
/// so two equivalent policies always print identically regardless of how
/// their source text was written.
///
/// ```
/// use retryspec::{parse, pretty_print};
///
/// let policy = parse("max_attempts=3, backoff=fixed(delay=3600s)").unwrap();
/// assert_eq!(pretty_print(&policy), "max_attempts=3, backoff=fixed(delay=1h), jitter=none");
/// ```
pub fn pretty_print(policy: &RetryPolicy) -> String {
    format!(
        "max_attempts={}, backoff={}, jitter={}",
        policy.max_attempts,
        format_backoff(&policy.backoff),
        format_jitter(policy.jitter)
    )
}

fn format_backoff(backoff: &Backoff) -> String {
    match backoff {
        Backoff::Fixed { delay } => format!("fixed(delay={})", format_duration(*delay)),
        Backoff::Linear {
            base,
            increment,
            max,
        } => format!(
            "linear(base={}, increment={}, max={})",
            format_duration(*base),
            format_duration(*increment),
            format_duration(*max)
        ),
        Backoff::Exponential { base, factor, max } => format!(
            "exponential(base={}, factor={}, max={})",
            format_duration(*base),
            factor,
            format_duration(*max)
        ),
    }
}

fn format_jitter(jitter: Jitter) -> &'static str {
    match jitter {
        Jitter::None => "none",
        Jitter::Full => "full",
        Jitter::Equal => "equal",
    }
}

/// Picks the largest unit (`h`, `m`, `s`, `ms`) that divides the duration
/// evenly. Parsed durations are always a whole number of milliseconds, so
/// this never loses precision.
fn format_duration(d: Duration) -> String {
    let total_ms = d.as_millis();
    if total_ms == 0 {
        return "0ms".to_string();
    }
    if total_ms % 3_600_000 == 0 {
        format!("{}h", total_ms / 3_600_000)
    } else if total_ms % 60_000 == 0 {
        format!("{}m", total_ms / 60_000)
    } else if total_ms % 1_000 == 0 {
        format!("{}s", total_ms / 1_000)
    } else {
        format!("{total_ms}ms")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn prints_canonical_form() {
        let policy = parse(
            "max_attempts=5, backoff=exponential(base=200ms, factor=2.0, max=30s), jitter=full",
        )
        .unwrap();
        assert_eq!(
            pretty_print(&policy),
            "max_attempts=5, backoff=exponential(base=200ms, factor=2, max=30s), jitter=full"
        );
    }

    #[test]
    fn round_trip_is_stable() {
        let policy = parse(
            "max_attempts=3, backoff=linear(base=100ms, increment=50ms, max=2s), jitter=equal",
        )
        .unwrap();
        let printed = pretty_print(&policy);
        let reparsed = parse(&printed).unwrap();
        assert_eq!(policy, reparsed);
        assert_eq!(printed, pretty_print(&reparsed));
    }

    #[test]
    fn formats_hours_when_exact() {
        let policy = parse("max_attempts=1, backoff=fixed(delay=3600s)").unwrap();
        assert_eq!(
            pretty_print(&policy),
            "max_attempts=1, backoff=fixed(delay=1h), jitter=none"
        );
    }

    #[test]
    fn falls_back_to_milliseconds_when_not_evenly_divisible() {
        let policy = parse("max_attempts=1, backoff=fixed(delay=1500ms)").unwrap();
        assert_eq!(
            pretty_print(&policy),
            "max_attempts=1, backoff=fixed(delay=1500ms), jitter=none"
        );
    }
}
