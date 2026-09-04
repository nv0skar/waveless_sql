// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

use super::*;

use sea_orm::SqlxPostgresPoolConnection;
use sqlx::postgres::*;

#[derive(Clone, BoxedAny, Debug)]
pub struct PostgresConnection(DatabaseConnection);

impl AnyExt for PostgresConnection {
    fn name(&self) -> &str {
        "postgres"
    }
}

#[async_trait]
impl AnyDatabaseConnection for PostgresConnection {
    async fn execute(&self, input: DatabaseInput) -> Result<DatabaseOutput> {
        let sql_connection = AnySQLConnection::new(&self.0);

        sql_connection.execute(input).await
    }
}

/// Postgres database
// TODO - Support more authentication methods
#[derive(
    Clone, PartialEq, Constructor, Serialize, Deserialize, BoxedAny, Getters, Display, Debug,
)]
#[display("Postgres: {}@{} on {}", username, host, db)]
#[getset(get = "pub")]
pub struct PostgresDbConnsectionConfig {
    host: SocketAddr,
    username: CompactString,
    password: CompactString,
    db: CompactString,
}

#[typetag::serde(name = "Postgres")]
#[async_trait]
impl AnyDatabaseConnectionConfig for PostgresDbConnsectionConfig {
    async fn new_conn(
        &self,
        id: CompactString,
        pool_min_size: Option<usize>,
        pool_max_size: Option<usize>,
    ) -> Result<(Arc<dyn AnyDatabaseConnection>, Box<dyn Any>)> {
        info!(
            "Creating new Postgre database connection ({}) on {}.",
            self.host, self.db
        );

        let num_cpus = std::thread::available_parallelism()?.get();

        let conn_options = PgConnectOptions::new()
            .host(&self.host.ip().to_string())
            .port(self.host.port())
            .username(&self.username)
            .password(&self.password)
            .database(&self.db);

        let pool = PoolOptions::<Postgres>::new()
            .min_connections(pool_min_size.unwrap_or(num_cpus) as u32)
            .max_connections(pool_max_size.unwrap_or(num_cpus * 2) as u32)
            .connect_with(conn_options)
            .await
            .wrap_err(format!("Failed creating {}'s Postgres pool.", id))?;

        let pool_wrapper =
            DatabaseConnection::from(DatabaseConnectionType::SqlxPostgresPoolConnection(
                SqlxPostgresPoolConnection::from(pool.to_owned()),
            ));

        Ok((Arc::new(PostgresConnection(pool_wrapper)), Box::new(pool)))
    }
}
