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
pub struct MySQLExecute {
    /// If no query is marked to be included in the response the response's body will be empty.
    /// NOTE: queries are executed sequentially.
    queries: CheapVec<SQLQuery>, // maybe explore better options to avoid cloning and achieve transparent deserialization.
}

impl AnyExt for MySQLExecute {
    fn name(&self) -> &str {
        "mysql"
    }
}

#[typetag::serde(name = "MySQL")]
#[async_trait]
impl AnyHttpExecute for MySQLExecute {
    /// Beware that the params are expected to be `ExecuteParams::StringMap`
    /// and the output will be a `serde_json::Value` that will be
    /// further serialized into JSON.
    async fn execute(
        &self,
        cx: RequestCx,
        db_conns: DbConns,
    ) -> Result<HttpResponse, RequestError> {
        any_sql_execute(&self.queries, cx, db_conns).await
    }
}

impl From<SQLQueryWrapper> for MySQLExecute {
    fn from(value: SQLQueryWrapper) -> Self {
        match value {
            SQLQueryWrapper::Many { queries } => Self::new(
                queries
                    .iter()
                    .map(|query| query.to_owned().into())
                    .collect::<CheapVec<SQLQuery>>(),
            ),
            SQLQueryWrapper::Single { query: sql_query } => {
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

                Self::new(queries)
            }
        }
    }
}
