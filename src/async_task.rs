pub trait LocalTaskRunner: 'static {
    fn spawn<T: LocalAsyncTask>(&self, task: T);
}

pub trait LocalAsyncTask: Send + Sync + 'static {
    async fn run(&mut self);

    fn pre_run(&mut self) {
        let ty_name = std::any::type_name::<Self>();
        tracing::debug!(ty_name, "local async task started")
    }

    fn post_run(&mut self) {
        let ty_name = std::any::type_name::<Self>();
        tracing::debug!(ty_name, "local async task ended")
    }
}

#[cfg(feature = "tokio")]
pub mod tokio_impl {
    use std::sync::OnceLock;

    use anyhow::Context;
    use futures::{future::LocalBoxFuture, FutureExt};

    use super::LocalAsyncTask;

    #[derive(Clone)]
    pub struct TokioLocalTaskRunner {
        sender: tokio::sync::mpsc::UnboundedSender<Box<dyn LocalAsyncTaskBoxed>>,
    }

    pub struct TokioLocalTaskExecutor {
        receiver: tokio::sync::mpsc::UnboundedReceiver<Box<dyn LocalAsyncTaskBoxed>>,
    }

    pub trait LocalAsyncTaskBoxed: Send + Sync + 'static {
        fn run(&mut self) -> LocalBoxFuture<()>;

        fn pre_run(&mut self);

        fn post_run(&mut self);
    }

    impl<T> LocalAsyncTaskBoxed for T
    where
        T: LocalAsyncTask,
    {
        fn run(&mut self) -> LocalBoxFuture<()> {
            async { LocalAsyncTask::run(self).await }.boxed_local()
        }

        fn pre_run(&mut self) {
            LocalAsyncTask::pre_run(self);
        }

        fn post_run(&mut self) {
            LocalAsyncTask::post_run(self);
        }
    }

    impl super::LocalTaskRunner for TokioLocalTaskRunner {
        fn spawn<T: LocalAsyncTaskBoxed>(&self, task: T) {
            self.sender
                .send(Box::new(task))
                .expect("Failed to spawn task. Is the `TokioTaskExecutor` running?");
        }
    }

    impl TokioLocalTaskRunner {
        pub fn get_or_init() -> anyhow::Result<Self> {
            static TASK_HANDLE: OnceLock<TokioLocalTaskRunner> = OnceLock::new();
            Self::get_or_init_with(&TASK_HANDLE)
        }

        pub fn get_or_init_with(
            handler: &'static OnceLock<TokioLocalTaskRunner>,
        ) -> anyhow::Result<Self> {
            if let Some(this) = handler.get() {
                return Ok(this.clone());
            }

            let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
            TokioLocalTaskExecutor { receiver }
                .start_in_new_thread()
                .context("failed to start TokioLocalTaskExecutor")?;

            let this = handler.get_or_init(|| TokioLocalTaskRunner { sender });

            Ok(this.clone())
        }
    }

    impl TokioLocalTaskExecutor {
        pub fn start_in_new_thread(mut self) -> std::io::Result<()> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;

            std::thread::spawn(move || {
                let local = tokio::task::LocalSet::new();

                local.spawn_local(async move {
                    while let Some(mut task) = self.receiver.recv().await {
                        tokio::task::spawn_local(async move {
                            task.pre_run();
                            task.run().await;
                            task.post_run();
                        });
                    }
                });

                rt.block_on(local);
            });

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use tokio::sync::oneshot;

        use crate::async_task::{LocalAsyncTask, LocalTaskRunner};

        struct IncreaseOneTask {
            count: u32,
            reporter: Option<tokio::sync::oneshot::Sender<u32>>,
        }

        impl LocalAsyncTask for IncreaseOneTask {
            async fn run(&mut self) {
                self.reporter.take().unwrap().send(self.count + 1).unwrap();
            }
        }

        #[tokio::test]
        async fn test_async_task_runner() -> anyhow::Result<()> {
            let runner = super::TokioLocalTaskRunner::get_or_init()?;
            let (tx, rx) = oneshot::channel();
            runner.spawn(IncreaseOneTask {
                count: 1,
                reporter: Some(tx),
            });

            let count = rx.await?;
            assert_eq!(count, 2);

            Ok(())
        }
    }
}
