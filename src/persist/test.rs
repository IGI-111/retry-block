use crate::persist::{RetryHandle, RetryInjector, Status};
use crate::RetryConfig;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

type OpsStorage = Arc<Mutex<HashMap<u64, (Status<i64, ()>, i64)>>>;
struct Injector {
    ops: OpsStorage,
}

#[async_trait]
impl<'a> RetryInjector<'a> for Injector {
    type Input = i64;
    type Output = i64;
    type Error = ();
    type Id = u64;
    type Res = Result<i64, ()>;
    async fn load_pending(&mut self) -> Vec<(u64, i64)> {
        self.ops
            .lock()
            .await
            .iter()
            .filter(|(_, (state, _))| matches!(state, Status::Pending))
            .map(|(id, (_, val))| (*id, *val))
            .collect()
    }
    async fn save_status(&mut self, id: u64, input: i64, status: Status<i64, ()>) {
        self.ops.lock().await.insert(id, (status, input));
    }
}

#[tokio::test]
async fn persistent_retry() {
    let counter = Arc::new(Mutex::new(0));
    let ops = Arc::new(Mutex::new(HashMap::from([(0, (Status::Pending, 3))])));

    let increment = |input| {
        let counter = counter.clone();
        async move {
            let counter = &mut (*counter.lock().await);
            *counter += input;
            Ok(*counter)
        }
    };

    let mut handle = RetryHandle::new(
        Injector { ops: ops.clone() },
        RetryConfig {
            count: 10,
            min_backoff: 500,
            max_backoff: 1000,
        },
    );

    let mut id = 0;

    assert_eq!(*counter.lock().await, 0);
    assert!(matches!(
        ops.lock().await.get(&0).unwrap(),
        (Status::Pending, 3)
    ));

    handle.retry_pending(1, &increment).await;
    assert_eq!(*counter.lock().await, 3);
    assert!(matches!(
        ops.lock().await.get(&id).unwrap(),
        (Status::Success(3), 3)
    ));
    id += 1;

    handle.retry(id, 6, &increment).await;
    assert_eq!(*counter.lock().await, 9);
    assert!(matches!(
        ops.lock().await.get(&id).unwrap(),
        (Status::Success(9), 6)
    ));
    id += 1;

    handle.retry(id, 4, &increment).await;
    assert_eq!(*counter.lock().await, 13);
    assert!(matches!(
        ops.lock().await.get(&id).unwrap(),
        (Status::Success(13), 4)
    ));
    id += 1;

    handle.retry(id, -1, &increment).await;
    assert_eq!(*counter.lock().await, 12);
    assert!(matches!(
        ops.lock().await.get(&id).unwrap(),
        (Status::Success(12), -1)
    ));
    id += 1;

    handle
        .retry(id, 2, &|input| {
            let counter = counter.clone();
            async move {
                let counter = &mut (*counter.lock().await);
                *counter *= input;
                Ok(*counter)
            }
        })
        .await;
    assert_eq!(*counter.lock().await, 24);
    assert!(matches!(
        ops.lock().await.get(&id).unwrap(),
        (Status::Success(24), 2)
    ));
    // id += 1;
}
