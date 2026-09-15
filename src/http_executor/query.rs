// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

#[derive(
    Clone, PartialEq, Constructor, Serialize, Deserialize, Getters, MutGetters, Display, Debug,
)]
#[display("{} (include: {})", query, include)]
#[getset(get = "pub", get_mut = "pub")]
pub struct SQLQuery {
    query: CompactString,

    /// Whether to include the query result in the response.
    #[serde(default = "default_true", skip_serializing_if = "should_skip")]
    include: bool,

    /// Query behaviour on response.
    #[serde(default, skip_serializing_if = "behaviour_skip")]
    behaviour: SQLBehaviour,
}

fn behaviour_skip(value: &SQLBehaviour) -> bool {
    *value == SQLBehaviour::Permissive
}

fn default_true() -> bool {
    true
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SQLBehaviour {
    Permissive,

    /// Whether the response from the database is expected not to be empty.
    FailOnEmpty,

    /// Whether the response from the database is expected to be exactly one row (unique) or empty.
    Unique,
}

impl Default for SQLBehaviour {
    fn default() -> Self {
        Self::Permissive
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum SQLQueryWrapper {
    Single {
        #[serde(default, skip_serializing_if = "should_skip_option")]
        database: Option<DatabaseId>,

        #[serde(flatten)]
        query: SQLQuery,
    },
    Many {
        #[serde(default, skip_serializing_if = "should_skip_option")]
        database: Option<DatabaseId>,

        queries: CheapVec<SQLQuery>,
    },
}

impl SQLQueryWrapper {
    pub fn new(query: CompactString) -> Self {
        Self::Single {
            database: None,
            query: SQLQuery {
                query,
                include: true,
                behaviour: SQLBehaviour::Permissive,
            },
        }
    }

    pub fn with_database(self, database: DatabaseId) -> Self {
        match self {
            SQLQueryWrapper::Single { query, .. } => Self::Single {
                database: Some(database),
                query,
            },
            SQLQueryWrapper::Many { queries, .. } => Self::Many {
                database: Some(database),
                queries,
            },
        }
    }

    pub fn with_include(self, include: bool) -> Self {
        match self {
            SQLQueryWrapper::Single {
                database: db_id,
                mut query,
            } => {
                query.include = include;

                Self::Single {
                    database: db_id,
                    query,
                }
            }
            SQLQueryWrapper::Many {
                database: db_id,
                mut queries,
            } => {
                queries.iter_mut().for_each(|query| {
                    query.include = include;
                });

                Self::Many {
                    database: db_id,
                    queries,
                }
            }
        }
    }

    pub fn with_behaviour(self, behaviour: SQLBehaviour) -> Self {
        match self {
            SQLQueryWrapper::Single {
                database: db_id,
                mut query,
            } => {
                query.behaviour = behaviour;

                Self::Single {
                    database: db_id,
                    query,
                }
            }
            SQLQueryWrapper::Many {
                database: db_id,
                mut queries,
            } => {
                queries.iter_mut().for_each(|query| {
                    query.behaviour = behaviour;
                });

                Self::Many {
                    database: db_id,
                    queries,
                }
            }
        }
    }
}
