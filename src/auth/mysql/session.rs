// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

use super::*;

/// TODO: add docs here.
#[derive(Clone, Constructor, Serialize, Deserialize, BoxedAny, Getters, Display, Debug)]
#[display("SQL backed token on table {}", table_name)]
#[getset(get = "pub")]
pub struct MySQLToken {
    /// Will use the primary database by default.
    #[serde(default, skip_serializing_if = "should_skip_option")]
    database: Option<DatabaseId>,

    table_name: CompactString,

    token_field: CompactString,

    /// Must not be primary key.
    user_id_field: CompactString,

    created_field: CompactString,

    /// Max age of sessions.
    max_age: usize,
}

impl AnyExt for MySQLToken {
    fn name(&self) -> &'static str {
        "mysqltoken"
    }
}

impl DatabaseConsumer for MySQLToken {
    fn databases(&self) -> CheapVec<DatabaseId> {
        if let Some(database) = &self.database {
            CheapVec::from_iter([database.to_owned()])
        } else {
            CheapVec::new_const()
        }
    }
}

#[typetag::serde(name = "MySQLToken")]
#[async_trait]
impl AnySessionMethod for MySQLToken {
    fn max_age(&self) -> Option<usize> {
        Some(self.max_age)
    }

    async fn check(&self, db_conns: DbConns, token: CompactString) -> Result<Option<UserId>> {
        assert_db_backends_length(db_conns.to_owned(), self.name().into())?;

        let db_conn = db_conns.values().next().unwrap();

        let Ok(db_conn) = db_conn
            .to_owned()
            .into_arc_any()
            .downcast::<MySQLConnection>()
        else {
            bail!(
                "Database connection for `MySQLToken` session method should be of type {:?} but it's of type {:?}.",
                TypeId::of::<MySQLDbConnsectionConfig>(),
                db_conn.inner_type_id()
            )
        };

        let res = db_conn
            .execute(DatabaseInput::QueryValues(
                format!(
                    "SELECT {}, {} FROM {} WHERE {} = ?",
                    self.user_id_field, self.created_field, self.table_name, self.token_field
                )
                .into(),
                CheapVec::from_vec(vec![token.to_owned()]),
            ))
            .await
            .map_err(|err| eyre!("Query execution error: {}", err))?;

        let DatabaseOutput::Any(res) = res else {
            bail!("Unexpected database's executor's output.");
        };

        let res = res.downcast::<Vec<QueryResult>>().map_err(|err| {
            RequestError::Other(eyre!("Cannot downcast to MySQL query result. {:?}", err))
        })?;

        let Some(entry) = res.first() else {
            return Ok(None);
        };

        let Ok(user_id) = entry.try_get::<u32>("", &self.user_id_field) else {
            bail!(
                "Field '{}' expected but not returned in '{}' table. Maybe it exists but the associated data type is not `INT UNSIGNED`.",
                self.user_id_field,
                self.table_name
            )
        };

        let Ok(created_at) = entry.try_get::<NaiveDateTime>("", &self.created_field) else {
            bail!(
                "Cannot find field '{}' in '{}' table. Maybe it exists but the associated data type is not `DATETIME`.",
                self.created_field,
                self.table_name
            )
        };

        // Checks whether the token has expired.
        if created_at + Duration::from_secs(self.max_age as u64) <= Utc::now().naive_utc()
            || created_at > Utc::now().naive_utc()
        {
            // TODO: remove the expired or invalid token.
            return Ok(None);
        }

        Ok(Some(user_id as usize))
    }

    async fn new(&self, db_conns: DbConns, user_id: UserId) -> Result<CompactString> {
        assert_db_backends_length(db_conns.to_owned(), self.name().into())?;

        let db_conn = db_conns.values().next().unwrap();

        let Ok(db_conn) = db_conn
            .to_owned()
            .into_arc_any()
            .downcast::<MySQLConnection>()
        else {
            bail!(
                "Database connection for `MySQLToken` session method should be of type {:?} but it's of type {:?}.",
                TypeId::of::<MySQLDbConnsectionConfig>(),
                db_conn.inner_type_id()
            )
        };

        let token: CompactString = Alphanumeric.sample_string(&mut rand::rng(), 32).into();

        let _ = db_conn
            .execute(DatabaseInput::QueryValues(
                format!("INSERT INTO {} VALUES (?, ?, ?)", self.table_name).into(),
                CheapVec::from_vec(vec![
                    token.to_owned(),
                    user_id.to_string().into(),
                    Utc::now().naive_utc().to_string().into(),
                ]),
            ))
            .await
            .map_err(|err| eyre!("Query execution error: {}", err))?;

        Ok(token)
    }

    async fn invalidate(
        &self,
        db_conns: DbConns,
        user_id: UserId,
        token: Option<CompactString>,
    ) -> Result<()> {
        assert_db_backends_length(db_conns.to_owned(), self.name().into())?;

        let db_conn = db_conns.values().next().unwrap();

        let Ok(db_conn) = db_conn
            .to_owned()
            .into_arc_any()
            .downcast::<MySQLConnection>()
        else {
            bail!(
                "Database connection for `MySQLToken` session method should be of type {:?} but it's of type {:?}.",
                TypeId::of::<MySQLDbConnsectionConfig>(),
                db_conn.inner_type_id()
            )
        };

        match token {
            Some(token) => {
                // Invalidate a given token id.
                db_conn
                    .execute(DatabaseInput::QueryValues(
                        format!(
                            "DELETE FROM {} WHERE {} = ? AND {} = ?",
                            self.table_name, self.token_field, self.user_id_field
                        )
                        .into(),
                        CheapVec::from_vec(vec![
                            token.to_string().into(),
                            user_id.to_string().into(),
                        ]),
                    ))
                    .await
                    .map_err(|err| eyre!("Query execution error: {}", err))?;
            }
            None => {
                // Invalidate all tokens from a given user.
                db_conn
                    .execute(DatabaseInput::QueryValues(
                        format!(
                            "DELETE FROM {} WHERE {} = ?",
                            self.table_name, self.user_id_field
                        )
                        .into(),
                        CheapVec::from_vec(vec![user_id.to_string().into()]),
                    ))
                    .await
                    .map_err(|err| eyre!("Query execution error: {}", err))?;
            }
        };

        Ok(())
    }

    async fn remove_expired(&self, _db_conn: Arc<dyn AnyDatabaseConnection>) -> Result<()> {
        todo!()
    }
}

impl Default for MySQLToken {
    fn default() -> Self {
        Self {
            database: None,
            table_name: "sessions_auth".into(),
            token_field: "session_id".into(),
            user_id_field: "user_id".into(),
            created_field: "created_at".into(),
            max_age: 86400,
        }
    }
}
