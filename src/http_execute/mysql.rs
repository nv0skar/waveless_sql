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
#[serde(from = "MySQLQueryWrapper")]
pub struct MySQLExecute {
    /// If no query is marked to be included in the response the response's body will be empty.
    /// NOTE: queries are executed sequentially.
    queries: CheapVec<MySQLQuery>, // maybe explore better options to avoid cloning and achieve transparent deserialization.
}

boxed_any!(MySQLExecute);

#[derive(Clone, PartialEq, Constructor, Serialize, Deserialize, Display, Debug)]
#[repr(transparent)]
#[serde(transparent)]
pub struct MySQLQuery(SQLQuery);

impl From<SQLQuery> for MySQLQuery {
    fn from(value: SQLQuery) -> Self {
        Self(value)
    }
}

impl AsRef<SQLQuery> for MySQLQuery {
    fn as_ref(&self) -> &SQLQuery {
        &self.0
    }
}

#[derive(Clone, PartialEq, Constructor, Serialize, Deserialize, Debug)]
#[repr(transparent)]
#[serde(transparent)]
pub struct MySQLQueryWrapper(SQLQueryWrapper);

impl From<SQLQueryWrapper> for MySQLQueryWrapper {
    fn from(value: SQLQueryWrapper) -> Self {
        Self(value)
    }
}

impl AsRef<SQLQueryWrapper> for MySQLQueryWrapper {
    fn as_ref(&self) -> &SQLQueryWrapper {
        &self.0
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
        db_conn: Arc<dyn AnyDatabaseConnection>,
    ) -> Result<HttpResponse, RequestError> {
        any_sql_execute(&self.queries, cx, db_conn).await
    }
}

impl From<MySQLQueryWrapper> for MySQLExecute {
    fn from(value: MySQLQueryWrapper) -> Self {
        match value.as_ref() {
            SQLQueryWrapper::Many { queries } => Self::new(
                queries
                    .iter()
                    .map(|query| query.to_owned().into())
                    .collect::<CheapVec<MySQLQuery>>(),
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
                    .collect::<CheapVec<MySQLQuery>>();

                Self::new(queries)
            }
        }
    }
}
