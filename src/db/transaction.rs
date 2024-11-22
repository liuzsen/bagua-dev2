use std::{cell::RefCell, future::Future, rc::Rc};

use futures::future::LocalBoxFuture;
use tracing::debug;

use crate::{provider::Provider, result::BizResult};

pub trait TransactionMaker: 'static {
    async fn do_transaction<F, T, E>(&mut self, tx: F) -> BizResult<T, E>
    where
        F: Future<Output = BizResult<T, E>>;

    fn register_callback<H>(&mut self, callback: H)
    where
        H: TxCallback;
}

pub trait LocalAsyncTask: Send + Sync + 'static {
    fn run(&mut self) -> LocalBoxFuture<()>;
}

pub trait LocalTaskRunner: 'static {
    fn spawn<T: LocalAsyncTask>(&mut self, task: T);
}

pub trait TxCallback: 'static {
    fn call(self: Box<Self>, tx_result: TxResult);
}

#[derive(Debug, Clone, Copy)]
pub enum TxResult {
    Committed,
    RolledBack,
}

pub struct BasicTxCallback<Tx, Task, Runner> {
    task_runner: Runner,
    tasks: Rc<RefCell<Option<Vec<Task>>>>,
    phantom: std::marker::PhantomData<Tx>,
}

impl<Tx, Task, Runner> Provider for BasicTxCallback<Tx, Task, Runner>
where
    Runner: Provider + Clone + LocalTaskRunner,
    Tx: TransactionMaker + Provider,
    Task: LocalAsyncTask,
{
    fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
        let mut tx = Tx::build(ctx)?;
        let this = Self {
            task_runner: Runner::build(ctx)?,
            tasks: Rc::new(RefCell::new(None)),
            phantom: std::marker::PhantomData,
        };
        tx.register_callback(this.clone());

        Ok(this)
    }
}

impl<Tx, T, Runner> BasicTxCallback<Tx, T, Runner> {
    pub fn push_task(&mut self, task: T) {
        let mut tasks = self.tasks.borrow_mut();
        let tasks = tasks.get_or_insert_with(Default::default);
        tasks.push(task);
    }
}

impl<Tx, T, Runner> Clone for BasicTxCallback<Tx, T, Runner>
where
    Runner: Clone,
{
    fn clone(&self) -> Self {
        Self {
            tasks: self.tasks.clone(),
            phantom: self.phantom.clone(),
            task_runner: self.task_runner.clone(),
        }
    }
}

impl<Tx, Task, Runner> TxCallback for BasicTxCallback<Tx, Task, Runner>
where
    Runner: LocalTaskRunner,
    Task: LocalAsyncTask,
    Tx: 'static,
{
    fn call(mut self: Box<Self>, tx_result: TxResult) {
        match tx_result {
            TxResult::Committed => {
                let mut tasks = self.tasks.borrow_mut();
                if let Some(tasks) = tasks.take() {
                    for task in tasks {
                        self.task_runner.spawn(task);
                    }
                }
            }
            TxResult::RolledBack => {
                debug!("tx rollback");
            }
        }
    }
}

#[cfg(feature = "tokio")]
pub mod task_runner {
    use std::sync::OnceLock;

    use crate::provider::Provider;

    use super::LocalAsyncTask;

    pub struct TokioTaskExecutor {
        receiver: tokio::sync::mpsc::UnboundedReceiver<Box<dyn LocalAsyncTask>>,
    }

    #[derive(Clone)]
    pub struct TokioLocalTaskRunner {
        sender: tokio::sync::mpsc::UnboundedSender<Box<dyn LocalAsyncTask>>,
    }

    static TASK_HANDLE: OnceLock<TokioLocalTaskRunner> = OnceLock::new();

    impl Provider for TokioLocalTaskRunner {
        fn build(_ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
            let this = TASK_HANDLE.get_or_init(|| {
                let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
                TokioTaskExecutor { receiver }.start();
                TokioLocalTaskRunner { sender }
            });
            Ok(this.clone())
        }
    }

    impl TokioLocalTaskRunner {
        pub fn spawn(&self, task: Box<dyn LocalAsyncTask>) {
            if let Err(_err) = self.sender.send(task) {
                tracing::error!("failed to spawn task");
                panic!("Failed to spawn task. Is the `TokioTaskExecutor` running?");
            }
        }
    }

    impl TokioTaskExecutor {
        pub fn start(mut self) {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(); // TODO: handle error

            std::thread::spawn(move || {
                let local = tokio::task::LocalSet::new();

                local.spawn_local(async move {
                    while let Some(mut task) = self.receiver.recv().await {
                        tokio::task::spawn_local(async move {
                            task.run().await;
                        });
                    }
                });

                rt.block_on(local);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use application::TxMq;
    use futures::FutureExt;
    use mocks::{MockTaskRunner, MockTxMaker};

    use crate::{provider::Provider, usecase::UseCase};

    use super::*;

    mod application {
        use crate::{biz_ok, provider::Provider, usecase::UseCase};

        pub trait TxMq {
            async fn send(&mut self, message: String) -> anyhow::Result<()>;
        }

        pub struct TestUserCase<TxMq> {
            tx_mq: TxMq,
        }

        impl<Mq> UseCase for TestUserCase<Mq>
        where
            Mq: TxMq,
        {
            type Params = String;

            type Output = ();

            type Error = ();

            async fn execute(
                &mut self,
                params: Self::Params,
            ) -> super::BizResult<Self::Output, Self::Error> {
                println!("[UseCase] user created, send message to mq");
                self.tx_mq.send(params).await?;

                biz_ok!(())
            }
        }

        impl<Mq> Provider for TestUserCase<Mq>
        where
            Mq: Provider,
        {
            fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
                Ok(Self {
                    tx_mq: Mq::build(ctx)?,
                })
            }
        }
    }

    mod mocks {
        use std::{cell::RefCell, rc::Rc};

        use tokio::{runtime::Builder, task::LocalSet};

        use crate::provider::Provider;

        use super::{LocalAsyncTask, LocalTaskRunner, TransactionMaker, TxCallback, TxResult};

        #[derive(Clone)]
        pub struct MockTaskRunner {}

        impl Provider for MockTaskRunner {
            fn build(_ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
                Ok(MockTaskRunner {})
            }
        }

        impl LocalTaskRunner for MockTaskRunner {
            fn spawn<T: LocalAsyncTask>(&mut self, mut task: T) {
                let rt = Builder::new_current_thread().enable_all().build().unwrap();

                std::thread::spawn(move || {
                    let local = LocalSet::new();

                    local.spawn_local(async move {
                        task.run().await;
                    });

                    rt.block_on(local);
                })
                .join()
                .unwrap();
            }
        }

        #[derive(Clone)]
        pub struct MockTxMaker {
            callbacks: Rc<RefCell<Vec<Box<dyn TxCallback>>>>,
        }

        impl Provider for MockTxMaker {
            /// Returns the same instance of MockTxMaker if it already exists in the context
            /// Otherwise, creates a new instance and inserts it into the context.
            fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
                if let Some(this) = ctx.get::<Self>() {
                    Ok(this.clone())
                } else {
                    let this = Self {
                        callbacks: Default::default(),
                    };
                    ctx.insert(this.clone());
                    Ok(this)
                }
            }
        }

        impl TransactionMaker for MockTxMaker {
            async fn do_transaction<F, T, E>(&mut self, tx: F) -> super::BizResult<T, E>
            where
                F: std::future::Future<Output = super::BizResult<T, E>>,
            {
                match tx.await {
                    Ok(Ok(out)) => {
                        for callback in self.callbacks.borrow_mut().drain(..) {
                            callback.call(TxResult::Committed);
                        }
                        Ok(Ok(out))
                    }
                    Ok(Err(e)) => {
                        for callback in self.callbacks.borrow_mut().drain(..) {
                            callback.call(TxResult::RolledBack);
                        }
                        Ok(Err(e))
                    }
                    Err(err) => {
                        for callback in self.callbacks.borrow_mut().drain(..) {
                            callback.call(TxResult::RolledBack);
                        }
                        Err(err)
                    }
                }
            }

            fn register_callback<H>(&mut self, callback: H)
            where
                H: super::TxCallback,
            {
                self.callbacks.borrow_mut().push(Box::new(callback));
            }
        }
    }

    pub struct TxMqImplByDeveloper<Tx, Runner> {
        callback: BasicTxCallback<Tx, MqTask, Runner>,
    }

    impl<Tx, Runner> Provider for TxMqImplByDeveloper<Tx, Runner>
    where
        BasicTxCallback<Tx, MqTask, Runner>: Provider,
    {
        fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
            Ok(Self {
                callback: BasicTxCallback::build(ctx)?,
            })
        }
    }

    impl<Tx, Runner> TxMq for TxMqImplByDeveloper<Tx, Runner> {
        async fn send(&mut self, message: String) -> anyhow::Result<()> {
            // 1. save to db
            println!("[Mq Adapter] save to db. wait for commit");
            // 2. create a task which will be executed after commit
            self.callback.push_task(MqTask { message });

            Ok(())
        }
    }

    pub struct MqTask {
        message: String,
    }

    impl LocalAsyncTask for MqTask {
        fn run(&mut self) -> LocalBoxFuture<()> {
            println!("[Mq Adapter] tx committed. run task");
            async {
                self.run().await.unwrap();
            }
            .boxed_local()
        }
    }

    impl MqTask {
        async fn run(&mut self) -> anyhow::Result<()> {
            println!("[Mq Adapter] send message to mq. message: {}", self.message);

            Ok(())
        }
    }

    struct TxUseCase<Tx, Uc> {
        tx: Tx,
        uc: Uc,
    }

    impl<Tx, UC> TxUseCase<Tx, UC>
    where
        Tx: TransactionMaker,
        UC: UseCase<Params = String, Output = (), Error = ()>,
    {
        async fn execute(&mut self, params: String) -> anyhow::Result<()> {
            self.tx
                .do_transaction(self.uc.execute(params))
                .await?
                .unwrap();

            Ok(())
        }
    }

    impl<Tx, UC> Provider for TxUseCase<Tx, UC>
    where
        UC: Provider,
        Tx: Provider,
    {
        fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
            Ok(Self {
                tx: Tx::build(ctx)?,
                uc: UC::build(ctx)?,
            })
        }
    }

    #[tokio::test]
    async fn main() -> anyhow::Result<()> {
        type UC = application::TestUserCase<TxMqImplByDeveloper<MockTxMaker, MockTaskRunner>>;
        type TxUc = TxUseCase<MockTxMaker, UC>;

        let mut uc = TxUc::provide()?;
        uc.execute("hello".to_string()).await?;

        Ok(())
    }
}
