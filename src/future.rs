//! Asychronous versions of the common retry functions
//!
//!
//! # Usage
//!
//! ```
//! use retry_block::async_retry;
//! use retry_block::OperationResult;
//! use retry_block::delay::Fixed;
//! use tokio::sync::Mutex;
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut collection = vec![1, 2, 3].into_iter();
//!
//!     let result = async_retry!(Fixed::new(Duration::from_millis(100)), {
//!         match collection.next() {
//!             Some(n) if n == 3 => Ok("n is 3!"),
//!             Some(_) => Err("n must be 3!"),
//!             None => Err("n was never 3!"),
//!         }
//!     });
//!
//!     assert!(result.is_ok());
//! }
//! ```
//!
//! ```
//! use retry_block::future::async_retry_fn;
//! use retry_block::OperationResult;
//! use retry_block::delay::Fixed;
//! use tokio::sync::Mutex;
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let collection = Arc::new(Mutex::new(vec![1, 2, 3].into_iter()));
//!
//!     let result = async_retry_fn(Fixed::new(Duration::from_millis(100)), || async {
//!         match collection.clone().lock().await.next() {
//!             Some(n) if n == 3 => Ok("n is 3!"),
//!             Some(_) => Err("n must be 3!"),
//!             None => Err("n was never 3!"),
//!         }
//!     }).await;
//!
//!     assert!(result.is_ok());
//! }
//! ```
//!
//! ```
//! use retry_block::async_retry;
//! use retry_block::OperationResult;
//! use retry_block::delay::Fixed;
//! use retry_block::RetryConfig;
//! use tokio::sync::Mutex;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = RetryConfig {
//!         count: 1,
//!         min_backoff: 100,
//!         max_backoff: 300,
//!     };
//!     let mut collection = vec![1, 2, 3].into_iter();
//!
//!     let result = async_retry!(config, {
//!         match collection.next() {
//!             Some(n) if n == 3 => Ok("n is 3!"),
//!             Some(_) => Err("n must be 3!"),
//!             None => Err("n was never 3!"),
//!         }
//!     });
//!
//!     assert!(result.is_err());
//! }
//! ```

use crate::async_retry;
use crate::OperationResult;
use std::time::Duration;

/// Retry the given operation until it succeeds, or until the given `Duration`
/// iterator ends.
///
/// <div class="example-wrap" style="display:inline-block">
/// <pre class="ignore" style="white-space:normal;font:inherit;">
///
/// **Warning**: Capturing outside values in async blocks of `FnMut`s will not work all the
/// time because async blocks may create references that outlive their scope.
///
/// You may have to wrap your data with `Arc<Mutex<_>>` or use `futures::Stream`
///
/// </pre></div>
pub async fn async_retry_fn<D, O, F, OR, R, E>(durations: D, mut operation: O) -> Result<R, E>
where
    D: IntoIterator<Item = Duration>,
    O: FnMut() -> F,
    F: std::future::Future<Output = OR>,
    OR: Into<OperationResult<R, E>>,
{
    async_retry!(durations, { operation().await })
}
