use amqprs::connection::Connection;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub rmq: Connection,
}

impl AppState {
    pub fn new(db: PgPool, rmq: Connection) -> Self {
        Self { db, rmq }
    }
}
