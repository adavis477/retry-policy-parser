//! A small text format for describing retry policies, plus a validating
//! parser and a canonical pretty printer.
//!
//! Every public function here is pure: given the same input it always
//! produces the same output, and none of them touch the filesystem, the
//! clock, or any other outside state. That is deliberate. Retry policies end
//! up compared, diffed, and round-tripped through config files a lot, so the
//! parser and printer need to be predictable enough to reason about with
//! nothing but the types.
//!
//! ```
//! use retryspec::{parse, pretty_print};
//!
//! let policy = parse(
//!     "max_attempts=5, backoff=exponential(base=200ms, factor=2.0, max=30s), jitter=full",
//! )
//! .unwrap();
//!
//! assert_eq!(policy.max_attempts, 5);
//! assert_eq!(
//!     pretty_print(&policy),
//!     "max_attempts=5, backoff=exponential(base=200ms, factor=2, max=30s), jitter=full"
//! );
//! ```

mod parser;
mod policy;
mod printer;

pub use parser::{parse, ParseError};
pub use policy::{Backoff, Jitter, RetryPolicy};
pub use printer::pretty_print;
