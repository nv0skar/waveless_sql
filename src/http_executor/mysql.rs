// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

use super::*;

/// TODO: add documentation.
#[derive(
    Clone,
    PartialEq,
    Constructor,
    Serialize,
    Deserialize,
    BoxedAny,
    Getters,
    MutGetters,
    Display,
    Debug,
)]
#[display("SQL queries: {:?}", queries)]
#[getset(get = "pub", get_mut = "pub")]
#[serde(from = "SQLQueryWrapper")]
pub struct MySQLExecutor {
    #[serde(default, skip_serializing_if = "should_skip_option")]
    database: Option<DatabaseId>,

    /// If no query is marked to be included in the response the response's body will be empty.
    /// NOTE: queries are executed sequentially.
    queries: CheapVec<SQLQuery>, // maybe explore better options to avoid cloning and achieve transparent deserialization.
}

impl AnyExt for MySQLExecutor {
    fn name(&self) -> &str {
        "mysql"
    }
}

#[typetag::serde(name = "MySQL")]
#[async_trait]
impl AnyHttpExecutor for MySQLExecutor {
    /// Beware that the params are expected to be `ExecuteParams::StringMap`
    /// and the output will be a `serde_json::Value` that will be
    /// further serialized into JSON.
    async fn execute(&self, cx: PipelineCx, db_conns: DbConns) -> PipelineResult {
        any_sql_execute(&self.queries, cx, db_conns, self.database.to_owned()).await
    }
}

impl From<SQLQueryWrapper> for MySQLExecutor {
    fn from(value: SQLQueryWrapper) -> Self {
        match value {
            SQLQueryWrapper::Many { database, queries } => Self::new(
                database,
                queries
                    .iter()
                    .map(|query| query.to_owned().into())
                    .collect::<CheapVec<SQLQuery>>(),
            ),
            SQLQueryWrapper::Single {
                database,
                query: sql_query,
            } => {
                let queries = sql_query
                    .query()
                    .split(';')
                    .map(|query| query.into())
                    .filter(|query: &CompactString| !query.is_empty())
                    .map(|query| {
                        SQLQuery::new(
                            query,
                            *sql_query.include(),
                            sql_query.behaviour().to_owned(),
                        )
                        .into()
                    })
                    .collect::<CheapVec<SQLQuery>>();

                Self::new(database, queries)
            }
        }
    }
}
