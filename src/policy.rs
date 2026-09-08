use std::time::Duration;

use crate::parser::ParseError;

/// A fully specified retry policy: how many attempts to make, how long to
/// wait between them, and whether to randomize those waits.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff: Backoff,
    pub jitter: Jitter,
}

/// The shape of the delay between attempts.
#[derive(Debug, Clone, PartialEq)]
pub enum Backoff {
    /// The same delay every time.
    Fixed { delay: Duration },
    /// Delay grows by a constant amount each attempt, capped at `max`.
    Linear {
        base: Duration,
        increment: Duration,
        max: Duration,
    },
    /// Delay is multiplied by `factor` each attempt, capped at `max`.
    Exponential {
        base: Duration,
        factor: f64,
        max: Duration,
    },
}

/// How much randomness to add on top of the computed backoff delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jitter {
    None,
    Full,
    Equal,
}

/// Checks that a policy's fields are internally consistent.
///
/// This is separate from parsing so it can be reused if a `RetryPolicy` is
/// ever built by hand instead of through [`crate::parse`].
pub(crate) fn validate(policy: &RetryPolicy) -> Result<(), ParseError> {
    if policy.max_attempts == 0 {
        return Err(ParseError::OutOfRange(
            "max_attempts must be at least 1".to_string(),
        ));
    }
    if policy.max_attempts > 1000 {
        return Err(ParseError::OutOfRange(
            "max_attempts must be at most 1000".to_string(),
        ));
    }

    match &policy.backoff {
        Backoff::Fixed { .. } => {}
        Backoff::Linear {
            base,
            increment,
            max,
        } => {
            if increment.is_zero() {
                return Err(ParseError::OutOfRange(
                    "linear backoff increment must be greater than zero".to_string(),
                ));
            }
            if max < base {
                return Err(ParseError::OutOfRange(
                    "linear backoff max must be greater than or equal to base".to_string(),
                ));
            }
        }
        Backoff::Exponential { base, factor, max } => {
            if !factor.is_finite() || *factor <= 1.0 {
                return Err(ParseError::OutOfRange(
                    "exponential backoff factor must be greater than 1".to_string(),
                ));
            }
            if max < base {
                return Err(ParseError::OutOfRange(
                    "exponential backoff max must be greater than or equal to base".to_string(),
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_policy(backoff: Backoff) -> RetryPolicy {
        RetryPolicy {
            max_attempts: 3,
            backoff,
            jitter: Jitter::None,
        }
    }

    #[test]
    fn accepts_a_reasonable_fixed_policy() {
        let policy = base_policy(Backoff::Fixed {
            delay: Duration::from_secs(1),
        });
        assert!(validate(&policy).is_ok());
    }

    #[test]
    fn rejects_zero_attempts() {
        let mut policy = base_policy(Backoff::Fixed {
            delay: Duration::from_secs(1),
        });
        policy.max_attempts = 0;
        assert!(validate(&policy).is_err());
    }

    #[test]
    fn rejects_linear_backoff_with_zero_increment() {
        let policy = base_policy(Backoff::Linear {
            base: Duration::from_secs(1),
            increment: Duration::from_secs(0),
            max: Duration::from_secs(5),
        });
        assert!(validate(&policy).is_err());
    }

    #[test]
    fn rejects_max_below_base() {
        let policy = base_policy(Backoff::Linear {
            base: Duration::from_secs(10),
            increment: Duration::from_secs(1),
            max: Duration::from_secs(5),
        });
        assert!(validate(&policy).is_err());
    }

    #[test]
    fn rejects_exponential_factor_that_does_not_grow() {
        let policy = base_policy(Backoff::Exponential {
            base: Duration::from_secs(1),
            factor: 1.0,
            max: Duration::from_secs(10),
        });
        assert!(validate(&policy).is_err());
    }
}
