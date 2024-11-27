use diesel::backend::Backend;
use diesel::{expression::exists::Exists, query_builder::AsQuery, Expression};
use diesel_async::RunQueryDsl;
use diesel_async::{
    methods::{ExecuteDsl, LoadQuery},
    AsyncConnection,
};

use std::{cell::UnsafeCell, rc::Rc};

use crate::provider::Provider;
use crate::provider::SingletonProvider;

use super::DbPool;

pub trait SqlRunner<DB: Backend> {
    type Connection: AsyncConnection<Backend = DB> + Send;

    async fn sql_execute<Sql>(&mut self, sql: Sql) -> Result<usize, SqlError>
    where
        Sql: ExecuteDsl<Self::Connection>;

    async fn sql_result<'query, U, Sql>(&mut self, sql: Sql) -> Result<Option<U>, SqlError>
    where
        U: Send,
        Sql: LoadQuery<'query, Self::Connection, U> + 'query;

    async fn sql_results<'query, U, Sql>(&mut self, sql: Sql) -> Result<Vec<U>, SqlError>
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

pub enum SqlError {
    Anyhow(anyhow::Error),
    Diesel(diesel::result::Error),
}

impl From<anyhow::Error> for SqlError {
    fn from(e: anyhow::Error) -> Self {
        SqlError::Anyhow(e)
    }
}

impl From<diesel::result::Error> for SqlError {
    fn from(e: diesel::result::Error) -> Self {
        SqlError::Diesel(e)
    }
}

impl From<SqlError> for anyhow::Error {
    fn from(e: SqlError) -> Self {
        match e {
            SqlError::Anyhow(e) => e,
            SqlError::Diesel(e) => e.into(),
        }
    }
}

impl SqlError {
    pub fn is_foreign_key_err(&self) -> bool {
        match self {
            SqlError::Diesel(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                _,
            )) => true,
            _ => false,
        }
    }
}

pub trait IsDbError {
    fn is_foreign_key_err(&self) -> bool;
}

impl<T> IsDbError for Result<T, SqlError> {
    fn is_foreign_key_err(&self) -> bool {
        match self {
            Ok(_) => false,
            Err(e) => e.is_foreign_key_err(),
        }
    }
}

pub struct DieselDriver<P>
where
    P: DbPool,
{
    conn: Rc<UnsafeCell<Option<P::Connection>>>,
    db_pool: P,
}

impl<P> Clone for DieselDriver<P>
where
    P: Clone + DbPool,
{
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            db_pool: self.db_pool.clone(),
        }
    }
}

impl<P> DieselDriver<P>
where
    P: DbPool,
{
    pub fn new(db_pool: P) -> Self {
        Self {
            conn: Rc::new(UnsafeCell::new(None)),
            db_pool,
        }
    }

    pub async fn fetch_or_reuse_conn(&mut self) -> anyhow::Result<&mut P::Connection>
    where
        P: DbPool,
    {
        unsafe {
            let conn = self.conn.get();
            let conn = &mut *conn;
            match conn {
                Some(conn) => Ok(conn),
                None => {
                    let pg_conn = self.db_pool.get_conn().await?;
                    if conn.is_none() {
                        *conn = Some(pg_conn);
                    }
                    Ok(conn.as_mut().unwrap())
                }
            }
        }
    }
}

impl<P> Provider for DieselDriver<P>
where
    P: Provider + 'static + Clone,
    P: DbPool,
{
    /// DieselDriver is always singleton
    fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
        if let Some(this) = ctx.get::<Self>() {
            return Ok(this.clone());
        }
        let this = Self::new(P::build(ctx)?);
        ctx.insert(this.clone());
        Ok(this)
    }
}

impl<P> SingletonProvider for DieselDriver<P>
where
    P: Provider + 'static + Clone,
    P: DbPool,
{
}

impl<P, DB> SqlRunner<DB> for DieselDriver<P>
where
    P: DbPool,
    P::Connection: diesel_async::AsyncConnection<Backend = DB> + Send,
    DB: Backend,
{
    type Connection = P::Connection;

    async fn sql_execute<Sql>(&mut self, sql: Sql) -> Result<usize, SqlError>
    where
        Sql: diesel_async::methods::ExecuteDsl<Self::Connection>,
    {
        let conn = self.fetch_or_reuse_conn().await?;
        Ok(sql.execute(conn).await?)
    }

    async fn sql_result<'query, U, Sql>(&mut self, sql: Sql) -> Result<Option<U>, SqlError>
    where
        U: Send,
        Sql: diesel_async::methods::LoadQuery<'query, Self::Connection, U> + 'query,
    {
        use diesel::result::OptionalExtension;
        let conn = self.fetch_or_reuse_conn().await?;
        Ok(sql.get_result(conn).await.optional()?)
    }

    async fn sql_results<'query, U, Sql>(&mut self, sql: Sql) -> Result<Vec<U>, SqlError>
    where
        U: Send,
        Sql: diesel_async::methods::LoadQuery<'query, Self::Connection, U> + 'query,
    {
        let conn = self.fetch_or_reuse_conn().await?;
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
        let conn = self.fetch_or_reuse_conn().await?;
        let exists = diesel::select(diesel::dsl::exists(sql))
            .get_result(conn)
            .await?;
        Ok(exists)
    }
}

pub mod transaction {
    use crate::{
        db::{
            transaction::{TransactionMaker, TxCallback},
            DbDriver,
        },
        provider::{Provider, SingletonProvider},
        result::BizResult,
    };

    use std::{
        cell::RefCell,
        future::Future,
        ops::{Deref, DerefMut},
        rc::Rc,
    };

    use anyhow::Context;
    use diesel_async::{AsyncConnection, TransactionManager};
    use tracing::error;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum TxStateInner {
        NotInTransaction,
        Committed,
        RolledBack,
    }

    #[derive(Clone)]
    pub struct DieselTxMaker<D> {
        driver: D,
        state: Rc<RefCell<TxStateInner>>,
        callbacks: Rc<RefCell<Vec<Box<dyn TxCallback>>>>,
    }

    impl<D> DieselTxMaker<D> {
        pub fn new(driver: D) -> Self {
            Self {
                driver,
                state: Rc::new(RefCell::new(TxStateInner::NotInTransaction)),
                callbacks: Rc::new(RefCell::new(Vec::new())),
            }
        }

        pub fn state(&self) -> TxStateInner {
            *self.state.borrow()
        }

        fn invoke_callbacks(&mut self) {
            if self.callbacks.borrow().is_empty() {
                return;
            }
            let state = match *self.state.borrow() {
                TxStateInner::NotInTransaction => {
                    tracing::warn!("Transaction state is not in transaction. Ignore callbacks. Maybe you forget to use transaction?");
                    return;
                }
                TxStateInner::Committed => crate::db::transaction::TxResult::Committed,
                TxStateInner::RolledBack => crate::db::transaction::TxResult::RolledBack,
            };

            let mut cbs = self.callbacks.borrow_mut();
            let cbs = std::mem::take(&mut *cbs);
            for cb in cbs {
                cb.call(state);
            }
        }
    }

    impl<D> TransactionMaker for DieselTxMaker<D>
    where
        D: DbDriver,
        <D as DbDriver>::Connection: DerefMut + Send,
        <<D as DbDriver>::Connection as Deref>::Target: AsyncConnection,
    {
        async fn do_transaction<F, T, E>(&mut self, tx: F) -> BizResult<T, E>
        where
            F: Future<Output = BizResult<T, E>>,
        {
            use diesel_async::pooled_connection::PoolTransactionManager;
            let conn = self
                .driver
                .get_connection()
                .await
                .context("get connection for transaction")?;
            PoolTransactionManager::begin_transaction(conn)
                .await
                .context("begin transaction")?;

            let res = match tx.await {
                Ok(Ok(value)) => {
                    // Only commit when there're no system error nor biz error
                    PoolTransactionManager::commit_transaction(conn).await?;
                    *self.state.borrow_mut() = TxStateInner::Committed;
                    Ok(Ok(value))
                }
                Ok(Err(user_error)) => {
                    *self.state.borrow_mut() = TxStateInner::RolledBack;
                    match PoolTransactionManager::rollback_transaction(conn).await {
                        Ok(()) => Ok(Err(user_error)),
                        Err(diesel::result::Error::BrokenTransactionManager) => {
                            Err(anyhow::anyhow!("broken transaction manager"))
                        }
                        Err(rollback_error) => Err(rollback_error.into()),
                    }
                }
                Err(sys_err) => {
                    tracing::info!("transaction has unknown error, rollback");
                    *self.state.borrow_mut() = TxStateInner::RolledBack;

                    match PoolTransactionManager::rollback_transaction(conn).await {
                        Ok(()) => Err(sys_err),
                        Err(diesel::result::Error::BrokenTransactionManager) => {
                            error!("broken transaction manager");
                            // In this case we are probably more interested by the
                            // original error, which likely caused this
                            Err(sys_err)
                        }
                        Err(rollback_error) => {
                            error!("failed to rollback transaction: {rollback_error}");
                            Err(rollback_error.into())
                        }
                    }
                }
            };

            self.invoke_callbacks();

            res
        }

        fn register_callback<H>(&mut self, callback: H)
        where
            H: crate::db::transaction::TxCallback,
        {
            self.callbacks.borrow_mut().push(Box::new(callback));
        }
    }

    impl<D> Drop for DieselTxMaker<D> {
        fn drop(&mut self) {
            if Rc::strong_count(&self.state) == 1 {
                self.invoke_callbacks();
            }
        }
    }

    impl<D> Provider for DieselTxMaker<D>
    where
        D: SingletonProvider + Clone + 'static,
    {
        /// DieselTxMaker is always singleton
        fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
            if let Some(this) = ctx.get::<Self>() {
                return Ok(this.clone());
            }

            let this = Self {
                driver: D::build_single(ctx)?,
                state: Rc::new(RefCell::new(TxStateInner::NotInTransaction)),
                callbacks: Rc::new(RefCell::new(Vec::new())),
            };
            ctx.insert(this.clone());

            Ok(this)
        }
    }

    impl<D> SingletonProvider for DieselTxMaker<D> where Self: Provider + Clone {}
}

pub mod pg_pool {
    //! Postgres connection pool

    use std::sync::OnceLock;
    use std::time::Duration;

    use anyhow::Result;
    use diesel_async::pooled_connection::deadpool::{Object, Pool};
    use diesel_async::pooled_connection::AsyncDieselConnectionManager;
    use diesel_async::AsyncPgConnection;
    use serde::{Deserialize, Serialize};

    /// pg pool connection
    pub type PgConn = Object<AsyncPgConnection>;
    pub type PgPool = Pool<AsyncPgConnection>;

    static POOL: OnceLock<PgPool> = OnceLock::new();

    /// Postgres connection pool config
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct PgPoolConfig {
        pub min_conn: u32,
        pub max_conn: u32,
        pub url: String,
    }

    impl Default for PgPoolConfig {
        fn default() -> Self {
            Self {
                min_conn: 1,
                max_conn: 10,
                url: "postgresql://postgres:postgres@127.0.0.1:5432/postgres".to_string(),
            }
        }
    }

    /// Initialize a global postgres connection pool
    pub async fn init(config: &PgPoolConfig) -> Result<()> {
        init_pool(&POOL, config).await
    }

    /// Get a postgres connection
    ///
    /// # Panics
    ///
    /// Panics if the global postgres connection pool has not been initialized
    pub async fn pg_conn() -> Result<PgConn> {
        let conn = POOL.get().unwrap().get().await?;
        Ok(conn)
    }

    /// Initialize a custom postgres connection pool
    pub async fn init_pool(
        pool_slot: &'static OnceLock<PgPool>,
        config: &PgPoolConfig,
    ) -> Result<()> {
        if pool_slot.get().is_some() {
            return Ok(());
        }

        let url_cfg =
            AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(&config.url);
        let new_pool = Pool::builder(url_cfg)
            .max_size(config.max_conn as usize)
            .build()?;
        let _conn = new_pool.get().await?;

        pool_slot.get_or_init(|| {
            tokio::spawn(async move {
                // Check every 30 seconds. Delete connections older than 1 minute
                let interval = Duration::from_secs(30);
                let max_age = Duration::from_secs(60);
                loop {
                    tokio::time::sleep(interval).await;

                    unsafe {
                        // SAFETY: the pool is initialized
                        pool_slot
                            .get()
                            .unwrap_unchecked()
                            .retain(|_, metrics| metrics.last_used() < max_age);
                    }
                }
            });

            new_pool
        });

        Ok(())
    }

    /// Get the global postgres connection pool
    pub fn get_pool() -> &'static PgPool {
        POOL.get().unwrap()
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use diesel::{prelude::*, Selectable};

    use crate::db::DbPool;

    use super::{DieselDriver, SqlRunner};

    diesel::table! {
        users (id) {
            id -> Int8,
            email -> Text,
            name -> Text,
        }
    }

    #[derive(Queryable, Selectable)]
    #[diesel(table_name = users)]
    pub struct User {
        pub id: i64,
        pub email: String,
        pub name: String,
    }

    struct MockDbPool;

    pub type PgConn =
        diesel_async::pooled_connection::deadpool::Object<diesel_async::AsyncPgConnection>;

    impl DbPool for MockDbPool {
        type Connection = PgConn;

        async fn get_conn(&self) -> anyhow::Result<Self::Connection> {
            unreachable!()
        }
    }

    async fn main() -> anyhow::Result<()> {
        let mut driver = DieselDriver::new(MockDbPool);
        let sql = users::table
            .select(User::as_select())
            .filter(users::name.eq("foo"));
        let _res = driver.sql_result(sql).await?;

        Ok(())
    }
}
