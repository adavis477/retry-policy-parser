# retryspec

Retry policies usually live as scattered constants: a `max_retries = 5`
here, a `time.sleep(2 ** attempt)` there, a jitter formula copy-pasted
between services until nobody remembers if it's "full" or "equal" jitter
anymore. There's no single artifact you can read, diff, or validate.

retryspec is a small text format for writing down a retry policy as one
line, plus a parser that validates it and a printer that renders it back to
a canonical form. The idea is that a policy is small enough to live in a
config file or a code comment, but structured enough that you can catch
"factor=0.5 makes the delay shrink" or "max is less than base" before it
ever runs.

## The format

```
max_attempts=5, backoff=exponential(base=200ms, factor=2.0, max=30s), jitter=full
```

Three fields, in any order in the source text:

- `max_attempts` — a positive integer, at most 1000.
- `backoff` — one of:
  - `fixed(delay=<duration>)`
  - `linear(base=<duration>, increment=<duration>, max=<duration>)`
  - `exponential(base=<duration>, factor=<number>, max=<duration>)`
- `jitter` — `none`, `full`, or `equal`. Optional, defaults to `none`.

Durations are a whole number followed by a unit: `ms`, `s`, `m`, or `h`
(for example `200ms`, `30s`, `2m`, `1h`). Whole numbers only — the printer
relies on that to pick a clean canonical unit without floating-point
rounding.

## Usage

```rust
use retryspec::{parse, pretty_print, ParseError};

fn main() {
    let text = "max_attempts=5, backoff=exponential(base=200ms, factor=2.0, max=30s), jitter=full";

    match parse(text) {
        Ok(policy) => {
            println!("{}", pretty_print(&policy));
            // max_attempts=5, backoff=exponential(base=200ms, factor=2, max=30s), jitter=full
        }
        Err(err) => eprintln!("invalid policy: {err}"),
    }

    // Validation catches policies that don't make sense, not just ones
    // that fail to parse:
    let backwards = "max_attempts=5, backoff=exponential(base=30s, factor=2.0, max=1s)";
    assert!(matches!(parse(backwards), Err(ParseError::OutOfRange(_))));
}
```

`parse` and `pretty_print` are both pure: no I/O, no globals, same input
always gives the same output. That was the point of building this the way
it's built — every parsing and formatting decision should be checkable with
a plain `assert_eq!` and a string, which is also why the test modules next
to each source file lean on exactly that.

## CLI

A `retryspec` binary validates a policy file and prints its canonical form:

```
$ retryspec policy.txt
max_attempts=5, backoff=exponential(base=200ms, factor=2, max=30s), jitter=full
```

On invalid input it prints the parse error to stderr and exits with a
non-zero status, so it can be dropped into a pre-commit hook or a CI step
that checks a policy file in.

## Status

This is a first pass at the format and the two functions that read and
write it. See the tests in `src/parser.rs`, `src/policy.rs`, and
`src/printer.rs` for the behavior that's locked in so far.

## License

MIT, see `LICENSE`.
