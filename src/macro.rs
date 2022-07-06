/// Retry a block with `std::thread::sleep`
///
/// Retry a block that returns an `Into<OperationResult<O, E>>` until it succeeds, or until the given `Duration`
/// iterator ends; and return a `Result<O, E>`.
///
/// ```
/// # use retry_block::retry;
/// # use retry_block::delay::Fixed;
/// # use std::time::Duration;
/// let mut tried = false;
///
/// let value = retry!(
///     // an `IntoIterator<Item = Duration>`
///     Fixed::new(Duration::from_millis(1)),
///
///     // a block that returns an `Into<OperationResult<_, _>>`
///     {
///         if tried {
///             Ok(42)
///         } else {
///             tried = true;
///             Err("try again")
///         }
///     }
/// ).unwrap();
/// assert_eq!(value, 42);
/// ```
///
#[macro_export]
macro_rules! retry {
    ($durations:expr, $block:block) => {{
        let mut it = $durations.into_iter();
        loop {
            match $block.into() {
                $crate::OperationResult::Ok(res) => break Ok(res),
                $crate::OperationResult::Err(e) => break Err(e),
                $crate::OperationResult::Retry(e) => {
                    if let Some(duration) = it.next() {
                        std::thread::sleep(duration)
                    } else {
                        break Err(e);
                    }
                }
            }
        }
    }};
}

/// Retry a block with `tokio::time::sleep`
///
/// Retry a block that returns an `Into<OperationResult<O, E>>` until it succeeds, or until the given `Duration`
/// iterator ends; and return a `Result<O, E>`.
///
/// This macro uses `.await` and is only suitable in an async context.
///
/// ```
/// # use retry_block::async_retry;
/// # use retry_block::delay::Fixed;
/// # use std::time::Duration;
/// #[tokio::main]
/// async fn main() {
///     let mut tried = false;
///
///     let value = async_retry!(
///         // an `IntoIterator<Item = Duration>`
///         Fixed::new(Duration::from_millis(1)),
///
///         // a block that returns an `Into<OperationResult<_, _>>`
///         {
///             if tried {
///                 Ok(42)
///             } else {
///                 tried = true;
///                 Err("try again")
///             }
///         }
///     );
///     assert_eq!(value, Ok(42));
/// }
/// ```
#[cfg(feature = "future")]
#[macro_export]
macro_rules! async_retry {
    ($durations:expr, $block:block) => {{
        let mut it = $durations.into_iter();
        loop {
            match $block.into() {
                $crate::OperationResult::Ok(res) => break Ok(res),
                $crate::OperationResult::Err(e) => break Err(e),
                $crate::OperationResult::Retry(e) => {
                    if let Some(duration) = it.next() {
                        tokio::time::sleep(duration).await;
                    } else {
                        break Err(e);
                    }
                }
            }
        }
    }};
}

/// Retry an operation forever with exponential delay until it succeeds
///
/// ```
/// # use retry_block::{retry_perpetual, retry};
/// # use retry_block::delay::Exponential;
/// # use std::time::Duration;
/// retry_perpetual!({
///     // ...
/// #   Ok::<(), ()>(())
/// });
/// // is equivalent to
/// retry!(Exponential::new(Duration::from_millis(100)), {
///     // ...
/// #   Ok::<(), ()>(())
/// }).unwrap();
/// ```
#[macro_export]
macro_rules! retry_perpetual {
    ($block:block) => {{
        let mut it = $crate::delay::Exponential::new(std::time::Duration::from_millis(100))
            .bounded(std::time::Duration::from_secs(3600))
            .into_iter();
        loop {
            match $block {
                Ok(res) => break res,
                Err(_) => {
                    let duration = it.next().unwrap();
                    std::thread::sleep(duration);
                }
            }
        }
    }};
}

/// Retry an operation forever with exponential delay until it succeeds
///
/// ```
/// # use retry_block::{async_retry_perpetual, async_retry};
/// # use retry_block::delay::Exponential;
/// # use std::time::Duration;
/// # #[tokio::main]
/// # async fn main() {
/// async_retry_perpetual!({
///     // ...
/// #   Ok::<(), ()>(())
/// });
/// // is equivalent to
/// async_retry!(Exponential::new(Duration::from_millis(100)), {
///     // ...
/// #   Ok::<(), ()>(())
/// }).unwrap();
/// # }
/// ```
#[cfg(feature = "future")]
#[macro_export]
macro_rules! async_retry_perpetual {
    ($block:block) => {{
        let mut it = $crate::delay::Exponential::new(std::time::Duration::from_millis(100))
            .bounded(std::time::Duration::from_secs(3600))
            .into_iter();
        loop {
            match $block {
                Ok(res) => break res,
                Err(_) => {
                    let duration = it.next().unwrap();
                    tokio::time::sleep(duration).await;
                }
            }
        }
    }};
}
