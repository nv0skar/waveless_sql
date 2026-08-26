// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

use super::*;

/// TODO: add documentation.
#[derive(
    Clone, PartialEq, Constructor, Serialize, Deserialize, Getters, MutGetters, Display, Debug,
)]
#[display("SQL queries: {:?}", queries)]
#[getset(get = "pub", get_mut = "pub")]
#[serde(from = "PostgresQueryWrapper")]
pub struct PostgresExecute {
    /// If no query is marked to be included in the response the response's body will be empty.
    /// NOTE: queries are executed sequentially.
    queries: CheapVec<PostgresQuery>, // maybe explore better options to avoid cloning and achieve transparent deserialization.
}

boxed_any!(PostgresExecute);

#[derive(Clone, PartialEq, Constructor, Serialize, Deserialize, Display, Debug)]
#[repr(transparent)]
#[serde(transparent)]
pub struct PostgresQuery(SQLQuery);

impl From<SQLQuery> for PostgresQuery {
    fn from(value: SQLQuery) -> Self {
        Self(value)
    }
}

impl AsRef<SQLQuery> for PostgresQuery {
    fn as_ref(&self) -> &SQLQuery {
        &self.0
    }
}

#[derive(Clone, PartialEq, Constructor, Serialize, Deserialize, Debug)]
#[repr(transparent)]
#[serde(transparent)]
pub struct PostgresQueryWrapper(SQLQueryWrapper);

impl From<SQLQueryWrapper> for PostgresQueryWrapper {
    fn from(value: SQLQueryWrapper) -> Self {
        Self(value)
    }
}

impl AsRef<SQLQueryWrapper> for PostgresQueryWrapper {
    fn as_ref(&self) -> &SQLQueryWrapper {
        &self.0
    }
}

#[typetag::serde(name = "Postgres")]
#[async_trait]
impl AnyHttpExecute for PostgresExecute {
    /// Beware that the params are expected to be `ExecuteParams::StringMap`
    /// and the output will be a `serde_json::Value` that will be
    /// further serialized into JSON.
    async fn execute(
        &self,
        cx: RequestCx,
        db_conn: Arc<dyn AnyDatabaseConnection>,
    ) -> Result<HttpResponse, RequestError> {
        any_sql_execute(&self.queries, cx, db_conn).await
    }
}

impl From<PostgresQueryWrapper> for PostgresExecute {
    fn from(value: PostgresQueryWrapper) -> Self {
        match value.as_ref() {
            SQLQueryWrapper::Many { queries } => Self::new(
                queries
                    .iter()
                    .map(|query| query.to_owned().into())
                    .collect::<CheapVec<PostgresQuery>>(),
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
                    .collect::<CheapVec<PostgresQuery>>();

                Self::new(queries)
            }
        }
    }
}
