//! `retry-block` provides utilities to retry operations that may fail with configurable backoff behavior.
//!
//! # Usage
//!
//! Retry an operation using the corresponding `retry` macro or `retry_fn` function. The macro
//! accepts an iterator over `Duration`s and a block that returns a `Result` (or `OperationResult`;
//! see below). The iterator is used to determine how long to wait after each unsuccessful try and
//! how many times to try before giving up and returning `Result::Err`. The block determines either
//! the final successful value, or an error value, which can either be returned immediately or used
//! to indicate that the operation should be retried.
//!
//! Any type that implements `IntoIterator<Item = Duration>` can be used to determine retry behavior,
//! though a few useful implementations are provided in the `delay` module, including a fixed delay
//! and exponential back-off.
//!
//!
//! The `Iterator` API can be used to limit or modify the delay strategy. For example, to limit the
//! number of retries to 1:
//!
//! ```
//! # use retry_block::retry;
//! # use retry_block::delay::Fixed;
//! # use std::time::Duration;
//! # use retry_block::OperationResult;
//!
//! let mut collection = vec![1, 2, 3].into_iter();
//!
//! let result = retry!(Fixed::new(Duration::from_millis(100)).take(1), {
//!     match collection.next() {
//!         Some(n) if n == 3 => Ok("n is 3!"),
//!         Some(_) => Err("n must be 3!"),
//!         None => Err("n was never 3!"),
//!     }
//! });
//!
//! assert!(result.is_err());
//! ```
//!
#![cfg_attr(
    feature = "config",
    doc = r##"
The RetryConfig struct can be used to retry an operation with a serializable retry config
that specifies an amount of retries and a random backoff interval.

```
# use retry_block::OperationResult;
# use retry_block::RetryConfig;
# use retry_block::delay::Fixed;
# use retry_block::retry;

let config = RetryConfig {
    count: 1,
    min_backoff: 100,
    max_backoff: 300,
};
let mut collection = vec![1, 2, 3].into_iter();

let result = retry!(config, {
    match collection.next() {
        Some(n) if n == 3 => Ok("n is 3!"),
        Some(_) => Err("n must be 3!"),
        None => Err("n was never 3!"),
    }
});

assert!(result.is_err());
```
"##
)]
#![cfg_attr(
    feature = "random",
    doc = r##"
Random jitter is applied by default to any delay strategy, but you can make it fixed using `exact`
or add random jitter to any delay strategy using the `jitter` function:

```
# use retry_block::retry_fn;
# use retry_block::OperationResult;
# use retry_block::delay::{Exponential, jitter};
# use std::time::Duration;

let mut collection = vec![1, 2, 3].into_iter();

let result = retry_fn(Exponential::exact(Duration::from_millis(10)).map(jitter).take(3), || {
    match collection.next() {
        Some(n) if n == 3 => Ok("n is 3!"),
        Some(_) => Err("n must be 3!"),
        None => Err("n was never 3!"),
    }
});

assert!(result.is_ok());
```
"##
)]
//!
//! To deal with fatal errors, return `retry_block::OperationResult`, which is like std's `Result`, but
//! with a third case to distinguish between errors that should cause a retry and errors that
//! should immediately return, halting retry behavior. (Internally, `OperationResult` is always
//! used, and closures passed to `retry` that return plain `Result` are converted into
//! `OperationResult`.)
//!
//! ```
//! # use retry_block::retry;
//! # use retry_block::delay::Fixed;
//! # use retry_block::OperationResult;
//! # use std::time::Duration;
//!
//! let mut collection = vec![1, 2].into_iter();
//! let value = retry!(Fixed::new(Duration::from_millis(1)), {
//!     match collection.next() {
//!         Some(n) if n == 2 => OperationResult::Ok(n),
//!         Some(_) => OperationResult::Retry("not 2"),
//!         None => OperationResult::Err("not found"),
//!     }
//! }).unwrap();
//!
//! assert_eq!(value, 2);
//! ```
//!
//! # Features
//!
//! - `random`: offer some random delay utilities (on by default)
//! - `config`: offer serializable retry config (on by default)
//! - `future`: offer asynchronous retry mechanisms (on by default)

use serde::Deserialize;
use std::time::Duration;

pub mod delay;
#[cfg(feature = "future")]
pub mod future;
mod r#macro;
pub mod persist;

pub use future::*;

/// A serializable retry configuration for a random range and finite retry count
#[derive(Debug, Deserialize, Clone)]
pub struct RetryConfig {
    /// how many times will we retry the operation
    pub count: usize,
    /// the minimum amount of milliseconds to wait before retrying
    pub min_backoff: u64,
    /// the maximum amount of milliseconds to wait before retrying
    pub max_backoff: u64,
}

impl IntoIterator for RetryConfig {
    type Item = Duration;
    type IntoIter = std::iter::Take<delay::Range>;
    fn into_iter(self) -> Self::IntoIter {
        delay::Range::from_millis_inclusive(self.min_backoff, self.max_backoff).take(self.count)
    }
}

#[derive(Debug)]
pub enum OperationResult<T, E> {
    /// Contains the success value.
    Ok(T),
    /// Contains the error value if duration is exceeded.
    Retry(E),
    /// Contains an error value to return immediately.
    Err(E),
}

impl<T, E> From<Result<T, E>> for OperationResult<T, E> {
    fn from(item: Result<T, E>) -> Self {
        match item {
            Ok(v) => OperationResult::Ok(v),
            Err(e) => OperationResult::Retry(e),
        }
    }
}

/// Retry the given operation until it succeeds, or until the given `Duration`
/// iterator ends.
pub fn retry_fn<D, O, OR, R, E>(durations: D, mut operation: O) -> Result<R, E>
where
    D: IntoIterator<Item = Duration>,
    O: FnMut() -> OR,
    OR: Into<OperationResult<R, E>>,
{
    retry!(durations, { operation() })
}
