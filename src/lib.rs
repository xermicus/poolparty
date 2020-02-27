use std::io;

use async_std::sync::{
    channel,
    Receiver,
    Sender,
};

use futures::{
    executor::ThreadPool,
    future::{Future,FutureExt},
    pin_mut,
    select,
};

/// Added functionality for the `futures::executor::ThreadPool` futures executor.
/// 
/// Futures will be spawned to and executed by the internal and exchangeable `ThreadPool` instance, but in such a way that *all* spawned futures are asked to stop on user request or in case any of them returns an error.
///
/// A notable difference to `futures:executor::ThreadPool` is that the user spawns futures of type `Output<Result(),T>` here instead of type `Output<()>`.
///
/// Caveats: If you do not call `observe().await` once all desired futures are spawned or if you spawn additional futures after the first `observe().await` the stopping mechanism won't work. In other words, instances cannot be "reused" after they were being observed for the first time.
/// For now no measures are in place to prevent a user from doing this (maybe in a future version).
/// 
/// Also note that spawned tasks *can not* be cancelled instantly. They will stop executing the next time they yield to the executor.
pub struct StoppableThreadPool<PoolError>
    where
        PoolError: Send + Sync + 'static,
    {
    pool: ThreadPool,
    control_sender: Sender<Result<(),PoolError>>,
    control_receiver: Receiver<Result<(),PoolError>>,
    stop_senders: Vec<Sender<()>>,
}

impl<PoolError> StoppableThreadPool<PoolError> 
    where
        PoolError: Send + Sync + 'static,
    {
    /// Create a new `StoppableThreadPool` instance using a default futures `ThreadPool` executor instance.
    pub fn new() -> Result<StoppableThreadPool<PoolError>,io::Error> {
        Ok(StoppableThreadPool::new_with_pool(
            ThreadPool::new()?
        ))
    }

    /// Create a new `StoppableThreadPool` instance using a user supplied futures `ThreadPool` executor instance.
    pub fn new_with_pool(pool: ThreadPool) -> StoppableThreadPool<PoolError> {
        let (control_sender, control_receiver) = channel::<Result<(),PoolError>>(1);
        StoppableThreadPool::<PoolError> {
            pool,
            control_sender,
            control_receiver,
            stop_senders: Vec::new(),
        }
    }

    /// Change the underlying futures `ThreadPool` executor instance. 
    pub fn with_pool(&mut self, pool: ThreadPool) -> &mut Self {
        self.pool = pool;
        self
    }

    /// Start executing a future right away.
    pub fn spawn<Fut>(&mut self, future: Fut) -> &mut Self
    where
        Fut: Future<Output = Result<(),PoolError>> + Send + 'static,
    {
        let (tx, rx) = channel::<()>(1);
        self.stop_senders.push(tx);
        let control = self.control_sender.clone();
        self.pool.spawn_ok(async move {
            let future = future.fuse();
            let stopped = rx.recv().fuse();
            pin_mut!(future, stopped);
            select! {
                output = future => control.send(output).await,
                _ = stopped => control.send(Ok(())).await
            };
        });
        self
    }

    /// Ensure that all spawned tasks are canceled on individual task error or any ` stop()` request issued by the user.
    /// Call this function once all tasks are spawned.
    /// A task that fails before a call to `observe()` is being awaited will still trigger a stop as soon as you actually start awaiting here.
    pub async fn observe(&self) -> Result<(),PoolError> {
        let mut completed: usize = 0;
        while let Some(output) = self.control_receiver.recv().await {
            completed += 1;
            if output.is_err() {
                for tx in self.stop_senders.iter() {
                    tx.send(()).await
                }
                return output
            }
            if completed == self.stop_senders.len() {
                break
            }
        }
        Ok(())
    }

    /// Stop the execution of all spawned tasks.
    pub async fn stop(&self, why: PoolError) {
        self.control_sender.send(Err(why)).await
    }
}

#[cfg(test)]
mod tests {
    use futures::{
        join,
        executor::block_on,
        executor::ThreadPool,
    };

    use crate::StoppableThreadPool;

    async fn ok() -> Result<(),String> {
        Ok(())
    }

    async fn forever() -> Result<(),String> {
        loop {}
    }

    async fn fail(msg: String) -> Result<(),String> {
        Err(msg)
    }

    #[test]
    fn observe_ok() {
        let mut pool = StoppableThreadPool::new().unwrap();
        for _ in 0..1000 {
            pool.spawn(ok());
        }

        block_on(async {
            assert_eq!(
                pool.observe().await.unwrap(),
                (),
            )
        });
    }

    #[test]
    fn observe_err() {
        let mut pool = StoppableThreadPool::new().unwrap();
        let err = "fail_function_called".to_string();
        pool.spawn(fail(err.clone()));
        pool.spawn(forever());

        block_on(async {
            assert_eq!(
                pool.observe().await.unwrap_err(),
                err
            )
        });
    }

    #[test]
    fn user_stopped() {
        let mut pool = StoppableThreadPool::new().unwrap();
        pool
            .spawn(forever())
            .spawn(forever());
        let stop_reason = "stopped by user".to_string();

        block_on(async {
            join!(
                async { 
                    assert_eq!(
                        pool.observe().await.unwrap_err(),
                        stop_reason.clone()
                    )
                },
                pool.stop(stop_reason.clone())
            )
        });
    }

    #[test]
    fn change_pool() {
        let mut pool = StoppableThreadPool::new().unwrap();
        pool.spawn(forever());
        pool.with_pool(ThreadPool::new().unwrap());
        pool.spawn(fail("fail function called".to_string()));

        block_on(async {
            assert_eq!(
                pool.observe().await.unwrap_err(),
                "fail function called".to_string(),
            )
        })
    }
}
