use diesel::{expression::exists::Exists, query_builder::AsQuery, Expression};
use diesel_async::RunQueryDsl;
use diesel_async::{
    methods::{ExecuteDsl, LoadQuery},
    AsyncConnection,
};

use std::{cell::UnsafeCell, rc::Rc};

use crate::{db::DbDriver, provider::Provider};

use super::DbPool;

pub trait SqlRunner {
    type Connection: AsyncConnection + Send;

    async fn get_connection(&mut self) -> anyhow::Result<&mut Self::Connection>;

    async fn sql_execute<Sql>(&mut self, sql: Sql) -> anyhow::Result<usize>
    where
        Sql: ExecuteDsl<Self::Connection>;

    async fn sql_result<'query, U, Sql>(&mut self, sql: Sql) -> anyhow::Result<Option<U>>
    where
        U: Send,
        Sql: LoadQuery<'query, Self::Connection, U> + 'query;

    async fn sql_results<'query, U, Sql>(&mut self, sql: Sql) -> anyhow::Result<Vec<U>>
    where
        U: Send,
        Sql: LoadQuery<'query, Self::Connection, U> + 'query;

    async fn sql_first<'query, U, Sql>(&mut self, sql: Sql) -> anyhow::Result<Option<U>>
    where
        U: Send,
        Sql: diesel::query_dsl::methods::LimitDsl,
        diesel::dsl::Limit<Sql>: LoadQuery<'query, Self::Connection, U> + 'query + Send;

    async fn sql_exists<'query, Sql>(&mut self, sql: Sql) -> anyhow::Result<bool>
    where
        Exists<Sql>: Expression,
        diesel::dsl::select<diesel::dsl::exists<Sql>>:
            LoadQuery<'query, Self::Connection, bool> + 'query + Send,
        diesel::dsl::select<Exists<Sql>>: AsQuery;
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

impl<P> DbDriver for DieselDriver<P>
where
    P: DbPool,
{
    type Connection = P::Connection;

    async fn connect(&mut self) -> anyhow::Result<&mut Self::Connection> {
        self.fetch_or_reuse_conn().await
    }
}

impl<P> Provider for DieselDriver<P>
where
    P: Provider + 'static + Clone,
    P: DbPool,
{
    fn build(ctx: &mut crate::provider::ProviderContext) -> anyhow::Result<Self> {
        if let Some(this) = ctx.get::<Self>() {
            return Ok(this.clone());
        }
        let this = Self::new(P::build(ctx)?);
        ctx.insert(this.clone());
        Ok(this)
    }
}

impl<P> SqlRunner for DieselDriver<P>
where
    P: DbPool,
    P::Connection: diesel_async::AsyncConnection + Send,
{
    type Connection = P::Connection;

    async fn get_connection(&mut self) -> anyhow::Result<&mut Self::Connection> {
        self.fetch_or_reuse_conn().await
    }

    async fn sql_execute<Sql>(&mut self, sql: Sql) -> anyhow::Result<usize>
    where
        Sql: diesel_async::methods::ExecuteDsl<Self::Connection>,
    {
        let conn = self.get_connection().await?;
        Ok(sql.execute(conn).await?)
    }

    async fn sql_result<'query, U, Sql>(&mut self, sql: Sql) -> anyhow::Result<Option<U>>
    where
        U: Send,
        Sql: diesel_async::methods::LoadQuery<'query, Self::Connection, U> + 'query,
    {
        use diesel::result::OptionalExtension;
        let conn = self.get_connection().await?;
        Ok(sql.get_result(conn).await.optional()?)
    }

    async fn sql_results<'query, U, Sql>(&mut self, sql: Sql) -> anyhow::Result<Vec<U>>
    where
        U: Send,
        Sql: diesel_async::methods::LoadQuery<'query, Self::Connection, U> + 'query,
    {
        let conn = self.get_connection().await?;
        Ok(sql.get_results(conn).await?)
    }

    async fn sql_first<'query, U, Sql>(&mut self, sql: Sql) -> anyhow::Result<Option<U>>
    where
        U: Send,
        Sql: diesel::query_dsl::methods::LimitDsl,
        diesel::dsl::Limit<Sql>:
            diesel_async::methods::LoadQuery<'query, Self::Connection, U> + 'query + Send,
    {
        use diesel::result::OptionalExtension;
        let conn = self.get_connection().await?;
        Ok(sql.first(conn).await.optional()?)
    }

    async fn sql_exists<'query, Sql>(&mut self, sql: Sql) -> anyhow::Result<bool>
    where
        diesel::expression::exists::Exists<Sql>: diesel::Expression,
        diesel::dsl::select<diesel::dsl::exists<Sql>>:
            diesel_async::methods::LoadQuery<'query, Self::Connection, bool> + 'query + Send,
        diesel::dsl::select<diesel::expression::exists::Exists<Sql>>:
            diesel::query_builder::AsQuery,
    {
        let conn = self.get_connection().await?;
        let exists = diesel::select(diesel::dsl::exists(sql))
            .get_result(conn)
            .await?;
        Ok(exists)
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
            todo!()
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
