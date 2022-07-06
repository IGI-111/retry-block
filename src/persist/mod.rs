//! Tools for persistent retries that save the retry status to be continued on a restart
//!
//! # Usage
//!
//! To use this persistent retry module, you need to create a `RetryHandle` associated to your
//! implementation of the `RetryInjector` trait.
//!
//! ```
//! # use retry_block::persist::{RetryHandle, RetryInjector, Status};
//! # use retry_block::RetryConfig;
//! # use async_trait::async_trait;
//! # use std::collections::HashMap;
//! # use std::sync::Arc;
//! # use tokio::sync::Mutex;
//!
//! struct Injector {
//!     ops: HashMap<u64, (Status<i64, ()>, i64)>,
//! }
//!
//! #[async_trait]
//! impl<'a> RetryInjector<'a> for Injector {
//!     type Input = i64;
//!     type Output = i64;
//!     type Error = ();
//!     type Id = u64;
//!     type Res = Result<i64, ()>;
//!     async fn load_pending(&mut self) -> Vec<(u64, i64)> {
//!         self.ops
//!             .iter()
//!             .filter(|(_, (state, _))| matches!(state, Status::Pending))
//!             .map(|(id, (_, val))| (id.clone(), val.clone()))
//!             .collect()
//!     }
//!     async fn save_status(&mut self, id: u64, input: i64, status: Status<i64, ()>) {
//!         self.ops.insert(id, (status, input));
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let counter = Arc::new(Mutex::new(0));
//!
//!     let increment = |input| {
//!         let counter = counter.clone();
//!         async move {
//!             let ref mut counter = *counter.lock().await;
//!             *counter += input;
//!             Ok(*counter)
//!         }
//!     };
//!
//!     let mut handle = RetryHandle::new(
//!         Injector {
//!             ops: HashMap::from([(0u64, (Status::Pending, 3))]),
//!         },
//!         RetryConfig {
//!             count: 10,
//!             min_backoff: 500,
//!             max_backoff: 1000,
//!         },
//!     );
//!     assert_eq!(*counter.lock().await, 0);
//!
//!     handle.retry_pending(1, &increment).await;
//!     assert_eq!(*counter.lock().await, 3);
//!
//!     handle.retry(1u64, 6, &increment).await;
//!     assert_eq!(*counter.lock().await, 9);
//!
//!     let multiply = |input| {
//!         let counter = counter.clone();
//!         async move {
//!             let ref mut counter = *counter.lock().await;
//!             *counter *= input;
//!             Ok(*counter)
//!         }
//!     };
//!     handle.retry(2u64, 2, &multiply).await;
//!     assert_eq!(*counter.lock().await, 18);
//! }
//! ```
//!
use crate::OperationResult;
use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(test)]
mod test;

/// Status of a persistent retry
pub enum Status<O, E> {
    Pending,
    Success(O),
    Failure(E),
}

impl<O, E> std::fmt::Debug for Status<O, E>
where
    O: std::fmt::Debug,
    E: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Success(o) => write!(f, "Success({:?})", o),
            Self::Failure(e) => write!(f, "Failure({:?})", e),
        }
    }
}

/// A trait to specify how to save and retrieve the status of a retried operation
#[async_trait]
pub trait RetryInjector<'a>: Sized {
    /// The input value of a retry operation
    ///
    /// Will be saved to repeat the operation
    type Input: Serialize + Deserialize<'a> + Clone;
    /// The positive output value of a retry operation
    ///
    /// Will be saved if the operation succeeds
    type Output;
    /// The negative output value of a retry operation
    ///
    /// Will be saved if the operation fails permanently
    type Error;
    /// An identifier for a given input
    ///
    /// Will be saved to repeat the operation
    type Id: Clone;
    /// A `Result` type for the output of the retry operation
    ///
    /// typically either:
    /// * `OperationResult<Self::Ouput, Self::Error>`
    /// * `Result<Self::Output, Self::Error>`
    type Res: Into<OperationResult<Self::Output, Self::Error>>;

    /// Return the stored inputs with a status of `Status::Pending`
    async fn load_pending(&mut self) -> Vec<(Self::Id, Self::Input)>;

    /// Save the status of a given operation
    async fn save_status(
        &mut self,
        id: Self::Id,
        input: Self::Input,
        status: Status<Self::Output, Self::Error>,
    );
}

/// Persistent retry handle
pub struct RetryHandle<Inj, Dur> {
    injector: Inj,
    durations: Dur,
}

impl<'a, Inj, Dur> RetryHandle<Inj, Dur>
where
    Inj: RetryInjector<'a>,
    Dur: IntoIterator<Item = std::time::Duration> + Clone,
{
    /// Create a new persistent retry handle from an injector and a cloneable delay iterator
    pub fn new(injector: Inj, durations: Dur) -> Self {
        Self {
            injector,
            durations,
        }
    }

    /// Start concurrent persistent retry of pending input loaded from the injector using the given
    /// operation and concurrency limit
    pub async fn retry_pending<F>(
        &mut self,
        concurrency_limit: usize,
        operation: &dyn Fn(Inj::Input) -> F,
    ) where
        F: Future<Output = Inj::Res>,
    {
        let pending = self.injector.load_pending().await;
        self.retry_stream(tokio_stream::iter(pending), concurrency_limit, operation)
            .await;
    }

    /// Start concurrent persistent retry of input loaded from the given stream using the given
    /// operation and concurrency limit
    pub async fn retry_stream<F, S>(
        &mut self,
        stream: S,
        concurrency_limit: usize,
        operation: &dyn Fn(Inj::Input) -> F,
    ) where
        F: Future<Output = Inj::Res>,
        S: Stream<Item = (Inj::Id, Inj::Input)>,
    {
        let handle = Arc::new(Mutex::new(self));
        stream
            .for_each_concurrent(concurrency_limit, |(id, input)| async {
                handle.lock().await.retry(id, input, operation).await;
            })
            .await;
    }

    /// Persistently retry a given input (uniquely identified by the given id) using the given
    /// operation
    pub async fn retry<F>(
        &mut self,
        id: Inj::Id,
        input: Inj::Input,
        operation: &dyn Fn(Inj::Input) -> F,
    ) where
        F: Future<Output = Inj::Res>,
    {
        self.injector
            .save_status(id.clone(), input.clone(), Status::Pending)
            .await;
        let mut it = self.durations.clone().into_iter();
        let res = loop {
            match operation(input.clone()).await.into() {
                OperationResult::Ok(res) => break Ok(res),
                OperationResult::Err(e) => break Err(e),
                OperationResult::Retry(e) => {
                    if let Some(duration) = it.next() {
                        tokio::time::sleep(duration).await;
                    } else {
                        break Err(e);
                    }
                }
            }
        };

        let status = match res {
            Ok(ok) => Status::Success(ok),
            Err(err) => Status::Failure(err),
        };
        self.injector
            .save_status(id.clone(), input.clone(), status)
            .await
    }
}
