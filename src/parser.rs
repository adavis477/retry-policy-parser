use std::time::Duration;

use crate::policy::{validate, Backoff, Jitter, RetryPolicy};

/// Why a piece of retry-policy text failed to parse or validate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    MalformedPair(String),
    MalformedBackoff(String),
    DuplicateField(String),
    UnknownField(String),
    UnknownBackoffKind(String),
    MissingField(&'static str),
    InvalidNumber { field: String, value: String },
    InvalidDuration { field: String, value: String },
    InvalidJitter(String),
    OutOfRange(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Empty => write!(f, "policy text is empty"),
            ParseError::MalformedPair(s) => write!(f, "expected `key=value`, got `{s}`"),
            ParseError::MalformedBackoff(s) => {
                write!(f, "expected `kind(field=value, ...)`, got `{s}`")
            }
            ParseError::DuplicateField(k) => write!(f, "field `{k}` was set more than once"),
            ParseError::UnknownField(k) => write!(f, "unknown field `{k}`"),
            ParseError::UnknownBackoffKind(k) => write!(f, "unknown backoff kind `{k}`"),
            ParseError::MissingField(k) => write!(f, "missing required field `{k}`"),
            ParseError::InvalidNumber { field, value } => {
                write!(f, "field `{field}` expected a number, got `{value}`")
            }
            ParseError::InvalidDuration { field, value } => write!(
                f,
                "field `{field}` expected a duration like `200ms` or `30s`, got `{value}`"
            ),
            ParseError::InvalidJitter(v) => {
                write!(f, "jitter must be `none`, `full`, or `equal`, got `{v}`")
            }
            ParseError::OutOfRange(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses and validates a retry policy from its text form.
///
/// ```
/// use retryspec::parse;
///
/// let policy = parse("max_attempts=3, backoff=fixed(delay=1s)").unwrap();
/// assert_eq!(policy.max_attempts, 3);
/// ```
pub fn parse(input: &str) -> Result<RetryPolicy, ParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseError::Empty);
    }

    let mut max_attempts: Option<u32> = None;
    let mut backoff: Option<Backoff> = None;
    let mut jitter: Option<Jitter> = None;

    for segment in split_top_level(trimmed) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let (key, value) = split_kv(segment)?;
        match key {
            "max_attempts" => {
                if max_attempts.is_some() {
                    return Err(ParseError::DuplicateField(key.to_string()));
                }
                max_attempts = Some(parse_u32("max_attempts", value)?);
            }
            "backoff" => {
                if backoff.is_some() {
                    return Err(ParseError::DuplicateField(key.to_string()));
                }
                backoff = Some(parse_backoff(value)?);
            }
            "jitter" => {
                if jitter.is_some() {
                    return Err(ParseError::DuplicateField(key.to_string()));
                }
                jitter = Some(parse_jitter(value)?);
            }
            other => return Err(ParseError::UnknownField(other.to_string())),
        }
    }

    let policy = RetryPolicy {
        max_attempts: max_attempts.ok_or(ParseError::MissingField("max_attempts"))?,
        backoff: backoff.ok_or(ParseError::MissingField("backoff"))?,
        jitter: jitter.unwrap_or(Jitter::None),
    };

    validate(&policy)?;
    Ok(policy)
}

/// Splits `key=value` pairs on top-level commas, i.e. commas that are not
/// nested inside a `backoff(...)` argument list.
fn split_top_level(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, c) in input.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&input[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&input[start..]);
    parts
}

fn split_kv(pair: &str) -> Result<(&str, &str), ParseError> {
    let idx = pair
        .find('=')
        .ok_or_else(|| ParseError::MalformedPair(pair.to_string()))?;
    let key = pair[..idx].trim();
    let value = pair[idx + 1..].trim();
    if key.is_empty() {
        return Err(ParseError::MalformedPair(pair.to_string()));
    }
    Ok((key, value))
}

fn parse_backoff(value: &str) -> Result<Backoff, ParseError> {
    let value = value.trim();
    let open = value
        .find('(')
        .ok_or_else(|| ParseError::MalformedBackoff(value.to_string()))?;
    if !value.ends_with(')') {
        return Err(ParseError::MalformedBackoff(value.to_string()));
    }
    let kind = value[..open].trim();
    let args_str = &value[open + 1..value.len() - 1];

    let mut args: Vec<(String, String)> = Vec::new();
    for segment in split_top_level(args_str) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let (k, v) = split_kv(segment)?;
        args.push((k.to_string(), v.to_string()));
    }

    let backoff = match kind {
        "fixed" => {
            let delay = parse_duration("delay", &take_field(&mut args, "delay")?)?;
            ensure_no_extra_fields(&args)?;
            Backoff::Fixed { delay }
        }
        "linear" => {
            let base = parse_duration("base", &take_field(&mut args, "base")?)?;
            let increment = parse_duration("increment", &take_field(&mut args, "increment")?)?;
            let max = parse_duration("max", &take_field(&mut args, "max")?)?;
            ensure_no_extra_fields(&args)?;
            Backoff::Linear {
                base,
                increment,
                max,
            }
        }
        "exponential" => {
            let base = parse_duration("base", &take_field(&mut args, "base")?)?;
            let factor = parse_f64("factor", &take_field(&mut args, "factor")?)?;
            let max = parse_duration("max", &take_field(&mut args, "max")?)?;
            ensure_no_extra_fields(&args)?;
            Backoff::Exponential { base, factor, max }
        }
        other => return Err(ParseError::UnknownBackoffKind(other.to_string())),
    };

    Ok(backoff)
}

fn take_field(args: &mut Vec<(String, String)>, name: &'static str) -> Result<String, ParseError> {
    match args.iter().position(|(k, _)| k == name) {
        Some(i) => Ok(args.remove(i).1),
        None => Err(ParseError::MissingField(name)),
    }
}

fn ensure_no_extra_fields(args: &[(String, String)]) -> Result<(), ParseError> {
    if let Some((k, _)) = args.first() {
        return Err(ParseError::UnknownField(k.clone()));
    }
    Ok(())
}

fn parse_u32(field: &str, value: &str) -> Result<u32, ParseError> {
    value.trim().parse::<u32>().map_err(|_| ParseError::InvalidNumber {
        field: field.to_string(),
        value: value.to_string(),
    })
}

fn parse_f64(field: &str, value: &str) -> Result<f64, ParseError> {
    let trimmed = value.trim();
    let parsed: f64 = trimmed.parse().map_err(|_| ParseError::InvalidNumber {
        field: field.to_string(),
        value: value.to_string(),
    })?;
    if !parsed.is_finite() {
        return Err(ParseError::InvalidNumber {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    Ok(parsed)
}

/// Parses a whole-number duration like `200ms`, `30s`, `2m`, or `1h`.
///
/// Durations are deliberately restricted to integer amounts so that the
/// pretty printer can pick a canonical unit without any floating-point
/// rounding to worry about.
fn parse_duration(field: &str, value: &str) -> Result<Duration, ParseError> {
    let value = value.trim();
    let split_at = value.find(|c: char| !c.is_ascii_digit());
    let split_at = match split_at {
        Some(i) if i > 0 => i,
        _ => {
            return Err(ParseError::InvalidDuration {
                field: field.to_string(),
                value: value.to_string(),
            })
        }
    };
    let (digits, unit) = value.split_at(split_at);
    let amount: u64 = digits.parse().map_err(|_| ParseError::InvalidDuration {
        field: field.to_string(),
        value: value.to_string(),
    })?;

    match unit {
        "ms" => Ok(Duration::from_millis(amount)),
        "s" => Ok(Duration::from_secs(amount)),
        "m" => Ok(Duration::from_secs(amount.saturating_mul(60))),
        "h" => Ok(Duration::from_secs(amount.saturating_mul(3600))),
        _ => Err(ParseError::InvalidDuration {
            field: field.to_string(),
            value: value.to_string(),
        }),
    }
}

fn parse_jitter(value: &str) -> Result<Jitter, ParseError> {
    match value.trim() {
        "none" => Ok(Jitter::None),
        "full" => Ok(Jitter::Full),
        "equal" => Ok(Jitter::Equal),
        other => Err(ParseError::InvalidJitter(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_policy() {
        let policy = parse(
            "max_attempts=5, backoff=exponential(base=200ms, factor=2.0, max=30s), jitter=full",
        )
        .unwrap();
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.jitter, Jitter::Full);
        match policy.backoff {
            Backoff::Exponential { base, factor, max } => {
                assert_eq!(base, Duration::from_millis(200));
                assert_eq!(factor, 2.0);
                assert_eq!(max, Duration::from_secs(30));
            }
            other => panic!("unexpected backoff: {other:?}"),
        }
    }

    #[test]
    fn defaults_jitter_to_none() {
        let policy = parse("max_attempts=3, backoff=fixed(delay=1s)").unwrap();
        assert_eq!(policy.jitter, Jitter::None);
    }

    #[test]
    fn rejects_zero_attempts() {
        let err = parse("max_attempts=0, backoff=fixed(delay=1s)").unwrap_err();
        assert!(matches!(err, ParseError::OutOfRange(_)));
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let err = parse("max_attempts=1, backoff=fixed(delay=1s), retries=1").unwrap_err();
        assert!(matches!(err, ParseError::UnknownField(_)));
    }

    #[test]
    fn rejects_unknown_backoff_field() {
        let err = parse("max_attempts=1, backoff=fixed(delay=1s, wat=2s)").unwrap_err();
        assert!(matches!(err, ParseError::UnknownField(_)));
    }

    #[test]
    fn rejects_missing_backoff() {
        let err = parse("max_attempts=1").unwrap_err();
        assert_eq!(err, ParseError::MissingField("backoff"));
    }

    #[test]
    fn rejects_duplicate_field() {
        let err =
            parse("max_attempts=1, max_attempts=2, backoff=fixed(delay=1s)").unwrap_err();
        assert!(matches!(err, ParseError::DuplicateField(_)));
    }

    #[test]
    fn rejects_shrinking_exponential_factor() {
        let err =
            parse("max_attempts=1, backoff=exponential(base=1s, factor=1.0, max=10s)")
                .unwrap_err();
        assert!(matches!(err, ParseError::OutOfRange(_)));
    }

    #[test]
    fn rejects_bad_jitter() {
        let err = parse("max_attempts=1, backoff=fixed(delay=1s), jitter=lots").unwrap_err();
        assert!(matches!(err, ParseError::InvalidJitter(_)));
    }
}
