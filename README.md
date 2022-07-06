# `retry-block`
<a href="https://crates.io/crates/retry-block"><img src="https://img.shields.io/crates/v/retry-block.svg" alt="Crate status"/></a>
<a href="https://docs.rs/retry-block"><img src="https://img.shields.io/docsrs/retry-block" alt="Crate docs"/></a>

`retry-block` provides utilities to retry operations that may fail with
configurable backoff behavior using macros over blocks of code

# Usage

Retry an operation using the corresponding `retry` macro or `retry_fn` function. The macro
accepts an iterator over `Duration`s and a block that returns a `Result` (or `OperationResult`;
see below). The iterator is used to determine how long to wait after each unsuccessful try and
how many times to try before giving up and returning `Result::Err`. The block determines either
the final successful value, or an error value, which can either be returned immediately or used
to indicate that the operation should be retried.

Any type that implements `IntoIterator<Item = Duration>` can be used to determine retry behavior,
though a few useful implementations are provided in the `delay` module, including a fixed delay
and exponential back-off.


The `Iterator` API can be used to limit or modify the delay strategy. For example, to limit the
number of retries to 1:

```
# use retry_block::retry;
# use retry_block::delay::Fixed;
# use std::time::Duration;
# use retry_block::OperationResult;

let mut collection = vec![1, 2, 3].into_iter();

let result = retry!(Fixed::new(Duration::from_millis(100)).take(1), {
    match collection.next() {
        Some(n) if n == 3 => Ok("n is 3!"),
        Some(_) => Err("n must be 3!"),
        None => Err("n was never 3!"),
    }
});

assert!(result.is_err());
```

