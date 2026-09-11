// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

use super::*;

use sea_orm::SqlxMySqlPoolConnection;
use sqlx::mysql::*;

#[derive(Clone, BoxedAny, Getters, Debug)]
#[getset(get = "pub")]
pub struct MySQLConnection(DatabaseConnection);

impl AnyExt for MySQLConnection {
    fn name(&self) -> &str {
        "mysql"
    }
}

#[async_trait]
impl AnyDatabaseConnection for MySQLConnection {
    async fn execute(&self, input: DatabaseInput) -> Result<DatabaseOutput> {
        let sql_connection = AnySQLConnection::new(&self.0);

        sql_connection.execute(input).await
    }
}

/// MySQL database
// TODO - Support more authentication methods
#[derive(
    Clone, PartialEq, Constructor, Serialize, Deserialize, BoxedAny, Getters, Display, Debug,
)]
#[display("MySQL: {}@{} on {}", username, host, db)]
#[getset(get = "pub")]
pub struct MySQLDbConnectionConfig {
    host: SocketAddr,
    username: CompactString,
    password: CompactString,
    db: CompactString,
}

#[typetag::serde(name = "MySQL")]
#[async_trait]
impl AnyDatabaseConnectionConfig for MySQLDbConnectionConfig {
    async fn new_conn(
        &self,
        id: CompactString,
        pool_min_size: Option<usize>,
        pool_max_size: Option<usize>,
    ) -> Result<(Arc<dyn AnyDatabaseConnection>, Box<dyn Any>)> {
        info!(
            "Creating new MySQL database connection ({}) on {}.",
            self.host, self.db
        );

        let num_cpus = std::thread::available_parallelism()?.get();

        let conn_options = MySqlConnectOptions::new()
            .host(&self.host.ip().to_string())
            .port(self.host.port())
            .username(&self.username)
            .password(&self.password)
            .database(&self.db);

        let pool = PoolOptions::<MySql>::new()
            .min_connections(pool_min_size.unwrap_or(num_cpus) as u32)
            .max_connections(pool_max_size.unwrap_or(num_cpus * 2) as u32)
            .connect_with(conn_options)
            .await
            .wrap_err(format!("Failed creating {}'s MySQL pool.", id))?;

        let pool_wrapper =
            DatabaseConnection::from(DatabaseConnectionType::SqlxMySqlPoolConnection(
                SqlxMySqlPoolConnection::from(pool.to_owned()),
            ));

        Ok((Arc::new(MySQLConnection(pool_wrapper)), Box::new(pool)))
    }
}
