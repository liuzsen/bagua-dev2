use std::{cell::RefCell, future::Future, rc::Rc};

use tracing::debug;

use crate::{
    async_task::{LocalAsyncTask, LocalTaskRunner},
    provider::Provider,
    result::BizResult,
};

pub trait TransactionMaker: 'static + Clone {
    async fn do_transaction<F, T, E>(&mut self, tx: F) -> BizResult<T, E>
    where
        F: Future<Output = BizResult<T, E>>;

    fn register_callback<H>(&mut self, callback: H)
    where
        H: TxCallback;
}

pub trait TxCallback: 'static {
    fn call(self: Box<Self>, tx_result: TxResult);
}

#[derive(Debug, Clone, Copy)]
pub enum TxResult {
    Committed,
    RolledBack,
}

pub struct AsyncTxCallbacks<Tx, Task, Runner> {
    task_runner: Rc<Runner>,
    tasks: Rc<RefCell<Option<Vec<Task>>>>,
    phantom: std::marker::PhantomData<Tx>,
}

impl<Tx, Task, Runner> Provider for AsyncTxCallbacks<Tx, Task, Runner>
where
    Runner: Provider + LocalTaskRunner,
    Tx: TransactionMaker + Provider,
    Task: LocalAsyncTask,
{
    fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
        let mut tx = Tx::build(ctx)?;
        let this = Self {
            task_runner: Rc::new(Runner::build(ctx)?),
            tasks: Rc::new(RefCell::new(None)),
            phantom: std::marker::PhantomData,
        };
        tx.register_callback(this.clone());

        Ok(this)
    }
}

impl<Tx, T, Runner> AsyncTxCallbacks<Tx, T, Runner> {
    pub fn push_task(&mut self, task: T) {
        let mut tasks = self.tasks.borrow_mut();
        let tasks = tasks.get_or_insert_with(Default::default);
        tasks.push(task);
    }
}

impl<Tx, T, Runner> Clone for AsyncTxCallbacks<Tx, T, Runner> {
    fn clone(&self) -> Self {
        Self {
            tasks: self.tasks.clone(),
            phantom: self.phantom.clone(),
            task_runner: self.task_runner.clone(),
        }
    }
}

impl<Tx, Task, Runner> TxCallback for AsyncTxCallbacks<Tx, Task, Runner>
where
    Runner: LocalTaskRunner,
    Task: LocalAsyncTask,
    Tx: 'static,
{
    fn call(self: Box<Self>, tx_result: TxResult) {
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
                debug!("tx rolled back. skip all tasks");
            }
        }
    }
}

#[cfg(feature = "tokio")]
#[cfg(test)]
mod tests {
    use application::TxMq;
    use mocks::{MockTaskRunner, MockTxMaker};
    use tokio::sync::oneshot;

    use crate::{provider::Provider, usecase::UseCase};

    use super::*;

    mod application {
        use crate::{biz_ok, provider::Provider, usecase::UseCase};

        pub trait TxMq {
            async fn send(&mut self, message: String) -> anyhow::Result<()>;
        }

        pub struct TestUserCase<TxMq> {
            pub tx_mq: TxMq,
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

        use crate::{async_task::tokio_impl::TokioLocalTaskRunner, provider::Provider};

        use super::{LocalAsyncTask, LocalTaskRunner, TransactionMaker, TxCallback, TxResult};

        pub struct MockTaskRunner {
            inner: TokioLocalTaskRunner,
        }

        impl LocalTaskRunner for MockTaskRunner {
            fn spawn<T: LocalAsyncTask>(&self, task: T) {
                self.inner.spawn(task);
            }
        }

        impl Provider for MockTaskRunner {
            fn build(_ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
                Ok(Self {
                    inner: TokioLocalTaskRunner::get_or_init()?,
                })
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
        callback: AsyncTxCallbacks<Tx, MqTask, Runner>,
        result: oneshot::Receiver<()>,
        sender: Option<oneshot::Sender<()>>,
    }

    impl<Tx, Runner> Provider for TxMqImplByDeveloper<Tx, Runner>
    where
        AsyncTxCallbacks<Tx, MqTask, Runner>: Provider,
    {
        fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
            let (tx, rx) = oneshot::channel();
            Ok(Self {
                callback: AsyncTxCallbacks::build(ctx)?,
                result: rx,
                sender: Some(tx),
            })
        }
    }

    impl<Tx, Runner> TxMq for TxMqImplByDeveloper<Tx, Runner> {
        async fn send(&mut self, message: String) -> anyhow::Result<()> {
            // 1. save to db
            println!("[Mq Adapter] save to db. wait for commit");
            // 2. create a task which will be executed after commit
            self.callback.push_task(MqTask {
                message,
                result_sender: self.sender.take(),
            });

            Ok(())
        }
    }

    pub struct MqTask {
        message: String,
        result_sender: Option<oneshot::Sender<()>>,
    }

    impl LocalAsyncTask for MqTask {
        async fn run(&mut self) {
            println!("[Mq Adapter] tx committed. run task");
            self.run().await.unwrap();
            self.result_sender.take().unwrap().send(()).unwrap();
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
        uc.uc.tx_mq.result.await?;

        Ok(())
    }
}
