pub mod diesel;

pub trait DbDriver {
    type Connection;

    async fn connect(&mut self) -> anyhow::Result<&mut Self::Connection>;
}

pub trait DbPool {
    type Connection;

    async fn get_conn(&self) -> anyhow::Result<Self::Connection>;
}
