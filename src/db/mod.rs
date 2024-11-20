pub mod diesel;
pub mod transaction;

pub trait DbDriver: 'static {
    type Connection;

    async fn connect(&mut self) -> anyhow::Result<&mut Self::Connection>;
}

pub trait DbPool: 'static {
    type Connection;

    async fn get_conn(&self) -> anyhow::Result<Self::Connection>;
}
