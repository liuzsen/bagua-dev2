use std::sync::Mutex as SyncMutex;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use anyhow::Context;
use diesel::backend::Backend;
use diesel::expression::exists::Exists;
use diesel::query_builder::AsQuery;
use diesel::Expression;
use diesel_async::methods::{ExecuteDsl, LoadQuery};
use diesel_async::pooled_connection::PoolTransactionManager;
use diesel_async::RunQueryDsl;
use diesel_async::{AsyncConnection, TransactionManager};
use tokio::sync::Mutex;

use crate::db::ConnectionPool;
use crate::provider::{Provider, SingletonProvider};

use super::{DbAdapter, TxCallback, TxnManager, TxnResult, TxnState};

pub mod new_type;
pub mod pg_pool;

pub struct DbAdapterDiesel<P>
where
    P: ConnectionPool,
{
    conn: Arc<Mutex<Option<P::Connection>>>,
    db_pool: P,
}

impl<P> Clone for DbAdapterDiesel<P>
where
    P: ConnectionPool + Clone,
{
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            db_pool: self.db_pool.clone(),
        }
    }
}

impl<P> Provider for DbAdapterDiesel<P>
where
    P: ConnectionPool + Provider + Clone,
{
    fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
        // DbAdapterDiesel is always singleton
        if let Some(this) = ctx.get::<Self>() {
            return Ok(this.clone());
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(None)),
            db_pool: P::build(ctx)?,
        })
    }
}

impl<P> SingletonProvider for DbAdapterDiesel<P> where P: ConnectionPool + Clone + Provider {}

macro_rules! fetch_or_reuse_conn {
    ($this:ident, $lock:ident) => {{
        match &mut *$lock {
            Some(c) => c,
            None => {
                *$lock = Some($this.db_pool.get_conn().await?);
                $lock.as_mut().unwrap()
            }
        }
    }};
}

impl<P> DbAdapter for DbAdapterDiesel<P>
where
    P: ConnectionPool,
    <P as ConnectionPool>::Connection: DerefMut + Send,
    <<P as ConnectionPool>::Connection as Deref>::Target: AsyncConnection,
{
    async fn begin_txn(&mut self) -> anyhow::Result<()> {
        let mut lock = self.conn.lock().await;
        let conn = fetch_or_reuse_conn!(self, lock);

        PoolTransactionManager::begin_transaction(conn)
            .await
            .context("failed to begin transaction via diesel connection")?;

        Ok(())
    }

    async fn commit_txn(&mut self) -> anyhow::Result<()> {
        let mut lock = self.conn.lock().await;
        let conn = fetch_or_reuse_conn!(self, lock);

        PoolTransactionManager::commit_transaction(conn)
            .await
            .context("failed to commit transaction via diesel connection")?;

        Ok(())
    }

    async fn rollback_txn(&mut self) -> anyhow::Result<()> {
        let mut lock = self.conn.lock().await;
        let conn = fetch_or_reuse_conn!(self, lock);

        PoolTransactionManager::rollback_transaction(conn)
            .await
            .context("failed to rollback transaction via diesel connection")?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct TxnManagerDiesel<A> {
    adapter: A,
    state: Arc<SyncMutex<TxnState>>,
    callbacks: Arc<SyncMutex<Vec<Box<dyn TxCallback>>>>,
}

impl<A> Provider for TxnManagerDiesel<A>
where
    A: Provider + Clone + SingletonProvider,
{
    fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
        if let Some(this) = ctx.get::<Self>() {
            return Ok(this.clone());
        }

        let this = Self::new(A::build_single(ctx)?);
        ctx.insert(this.clone());

        Ok(this)
    }
}

impl<A> SingletonProvider for TxnManagerDiesel<A> where A: Provider + Clone + SingletonProvider {}

impl<A> TxnManagerDiesel<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            state: Arc::new(SyncMutex::new(TxnState::NotInTransaction)),
            callbacks: Arc::new(SyncMutex::new(Vec::new())),
        }
    }

    fn state(&self) -> TxnState {
        self.state.lock().unwrap().clone()
    }

    fn invoke_callbacks(&mut self) {
        let mut callbacks_lock = self.callbacks.lock().unwrap();

        let callbacks = std::mem::take(&mut *callbacks_lock);
        drop(callbacks_lock);

        if callbacks.is_empty() {
            return;
        }

        let state = match self.state() {
            TxnState::NotInTransaction => {
                tracing::warn!("Transaction is not in transaction. Ignore callbacks. Maybe you forget to use transaction?");
                return;
            }
            TxnState::Committed => TxnResult::Committed,
            TxnState::RolledBack => TxnResult::RolledBack,
            TxnState::Begun => TxnResult::RolledBack,
        };

        for cb in callbacks {
            cb.call(state);
        }
    }

    fn set_state(&self, state: TxnState) {
        let mut lock = self.state.lock().unwrap();
        *lock = state;
    }
}

impl<A> TxnManager for TxnManagerDiesel<A>
where
    A: DbAdapter,
{
    async fn do_transaction<F, T, E>(&mut self, tx: F) -> crate::result::BizResult<T, E>
    where
        F: std::future::Future<Output = crate::result::BizResult<T, E>>,
    {
        self.adapter.begin_txn().await?;
        self.set_state(TxnState::Begun);

        let res = match tx.await {
            Ok(Ok(value)) => {
                // Only commit when there's no system error nor biz error
                self.adapter.commit_txn().await?;
                self.set_state(TxnState::Committed);

                Ok(Ok(value))
            }
            Ok(Err(user_error)) => {
                self.adapter.rollback_txn().await?;
                self.set_state(TxnState::RolledBack);

                Ok(Err(user_error))
            }
            Err(sys_err) => {
                tracing::info!("A system error occurred during transaction, rollback");

                self.adapter.rollback_txn().await?;
                self.set_state(TxnState::RolledBack);

                Err(sys_err)
            }
        };

        self.invoke_callbacks();

        res
    }

    fn register_callback<H>(&self, callback: H)
    where
        H: TxCallback,
    {
        let mut callbacks_lock = self.callbacks.lock().unwrap();
        callbacks_lock.push(Box::new(callback));
    }

    fn state(&self) -> TxnState {
        self.state()
    }
}

impl<A> Drop for TxnManagerDiesel<A> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.callbacks) == 1 {
            self.invoke_callbacks();
        }
    }
}

#[derive(derive_more::From)]
pub enum SqlErrorDiesel {
    Anyhow(anyhow::Error),
    Diesel(diesel::result::Error),
}

impl From<SqlErrorDiesel> for anyhow::Error {
    fn from(value: SqlErrorDiesel) -> Self {
        match value {
            SqlErrorDiesel::Anyhow(error) => error,
            SqlErrorDiesel::Diesel(error) => From::from(error),
        }
    }
}

impl SqlErrorDiesel {
    pub fn is_conflict(&self) -> bool {
        match self {
            SqlErrorDiesel::Anyhow(_error) => false,
            SqlErrorDiesel::Diesel(error) => match error {
                diesel::result::Error::DatabaseError(
                    database_error_kind,
                    _database_error_information,
                ) => match database_error_kind {
                    diesel::result::DatabaseErrorKind::UniqueViolation => true,
                    _ => false,
                },
                _ => false,
            },
        }
    }
}

pub trait DieselSqlRunner<DB: Backend> {
    type Connection: AsyncConnection<Backend = DB> + Send;

    async fn sql_execute<Sql>(&mut self, sql: Sql) -> Result<usize, SqlErrorDiesel>
    where
        Sql: ExecuteDsl<Self::Connection>;

    async fn sql_result<'query, U, Sql>(&mut self, sql: Sql) -> Result<Option<U>, SqlErrorDiesel>
    where
        U: Send,
        Sql: LoadQuery<'query, Self::Connection, U> + 'query;

    async fn sql_results<'query, U, Sql>(&mut self, sql: Sql) -> Result<Vec<U>, SqlErrorDiesel>
    where
        U: Send,
        Sql: LoadQuery<'query, Self::Connection, U> + 'query;

    async fn sql_exists<'query, Sql>(&mut self, sql: Sql) -> anyhow::Result<bool>
    where
        Exists<Sql>: Expression,
        diesel::dsl::select<diesel::dsl::exists<Sql>>:
            LoadQuery<'query, Self::Connection, bool> + 'query + Send,
        diesel::dsl::select<Exists<Sql>>: AsQuery;
}

impl<P, DB> DieselSqlRunner<DB> for DbAdapterDiesel<P>
where
    P: ConnectionPool,
    P::Connection: diesel_async::AsyncConnection<Backend = DB> + Send,
    DB: Backend,
{
    type Connection = P::Connection;

    async fn sql_execute<Sql>(&mut self, sql: Sql) -> Result<usize, SqlErrorDiesel>
    where
        Sql: diesel_async::methods::ExecuteDsl<Self::Connection>,
    {
        let mut lock = self.conn.lock().await;
        let conn = fetch_or_reuse_conn!(self, lock);

        Ok(sql.execute(conn).await?)
    }

    async fn sql_result<'query, U, Sql>(&mut self, sql: Sql) -> Result<Option<U>, SqlErrorDiesel>
    where
        U: Send,
        Sql: diesel_async::methods::LoadQuery<'query, Self::Connection, U> + 'query,
    {
        use diesel::result::OptionalExtension;
        let mut lock = self.conn.lock().await;
        let conn = fetch_or_reuse_conn!(self, lock);

        Ok(sql.get_result(conn).await.optional()?)
    }

    async fn sql_results<'query, U, Sql>(&mut self, sql: Sql) -> Result<Vec<U>, SqlErrorDiesel>
    where
        U: Send,
        Sql: diesel_async::methods::LoadQuery<'query, Self::Connection, U> + 'query,
    {
        let mut lock = self.conn.lock().await;
        let conn = fetch_or_reuse_conn!(self, lock);

        Ok(sql.get_results(conn).await?)
    }

    async fn sql_exists<'query, Sql>(&mut self, sql: Sql) -> anyhow::Result<bool>
    where
        diesel::expression::exists::Exists<Sql>: diesel::Expression,
        diesel::dsl::select<diesel::dsl::exists<Sql>>:
            diesel_async::methods::LoadQuery<'query, Self::Connection, bool> + 'query + Send,
        diesel::dsl::select<diesel::expression::exists::Exists<Sql>>:
            diesel::query_builder::AsQuery,
    {
        let mut lock = self.conn.lock().await;
        let conn = fetch_or_reuse_conn!(self, lock);

        let exists = diesel::select(diesel::dsl::exists(sql))
            .get_result(conn)
            .await?;
        Ok(exists)
    }
}
