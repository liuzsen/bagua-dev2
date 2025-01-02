use std::future::Future;

use crate::result::BizResult;

#[cfg(feature = "diesel")]
pub mod diesel;
pub mod primitives;

pub trait ConnectionPool: Clone + 'static {
    type Connection;

    async fn get_conn(&self) -> anyhow::Result<Self::Connection>;
}

pub trait DbAdapter: Clone + 'static {
    async fn begin_txn(&mut self) -> anyhow::Result<()>;

    async fn commit_txn(&mut self) -> anyhow::Result<()>;

    async fn rollback_txn(&mut self) -> anyhow::Result<()>;
}

pub trait TxnManager: Clone + 'static {
    async fn do_transaction<F, T, E>(&mut self, tx: F) -> BizResult<T, E>
    where
        F: Future<Output = BizResult<T, E>>;

    fn register_callback<H>(&self, callback: H)
    where
        H: TxCallback;

    fn state(&self) -> TxnState;
}

pub trait TxCallback: 'static {
    fn call(self: Box<Self>, tx_result: TxnResult);
}

#[derive(Debug, Clone, Copy)]
pub enum TxnResult {
    Committed,
    RolledBack,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TxnState {
    NotInTransaction,
    Begun,
    Committed,
    RolledBack,
}
