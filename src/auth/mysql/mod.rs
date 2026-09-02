// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

mod role;
mod session;

pub use role::*;
pub use session::*;

use crate::*;

use super::*;

use databases::mysql::*;

use sea_orm::QueryResult;

/// TODO: add docs here.
#[derive(
    Clone, PartialEq, Constructor, Serialize, Deserialize, BoxedAny, Getters, Display, Debug,
)]
#[display("Name & password authentication on SQL using table {}", table_name)]
#[getset(get = "pub")]
pub struct MySQLSimpleAuthentication {
    /// Will use the primary database by default.
    #[serde(default, skip_serializing_if = "should_skip_option")]
    database: Option<DatabaseId>,

    table_name: CompactString,

    user_id_field: CompactString,

    /// This field references to the user's name, emails, IDs... and must not be primary key.
    name_field: CompactString,

    password_field: CompactString,

    /// Specifies all other the fields the user table contains, useful for signing up new users.
    #[serde(default, skip_serializing_if = "CheapVec::is_empty")]
    extra_fields: CheapVec<CompactString>,

    totp_field: Option<CompactString>,
}

impl AnyExt for MySQLSimpleAuthentication {
    fn name(&self) -> &'static str {
        "mysqlsimple"
    }
}

impl DatabaseConsumer for MySQLSimpleAuthentication {
    fn databases(&self) -> CheapVec<DatabaseId> {
        if let Some(database) = &self.database {
            CheapVec::from_iter([database.to_owned()])
        } else {
            CheapVec::new_const()
        }
    }
}

#[typetag::serde(name = "MySQLSimple")]
#[async_trait]
impl AnyAuthenticationMethod for MySQLSimpleAuthentication {
    async fn check(
        &self,
        db_conns: DbConns,
        entries: HashMap<CompactString, CompactString>,
    ) -> Result<Option<UserId>> {
        assert_db_backends_length(db_conns.to_owned(), self.name().into())?;

        let db_conn = db_conns.values().next().unwrap();

        let Ok(db_conn) = db_conn
            .to_owned()
            .into_arc_any()
            .downcast::<MySQLConnection>()
        else {
            bail!(
                "Database connection for `MySQLSimple` authentication should be of type {:?} but it's of type {:?}.",
                TypeId::of::<MySQLDbConnsectionConfig>(),
                db_conn.inner_type_id()
            )
        };

        let name_field = entries
            .get(&self.name_field)
            .ok_or(eyre!("'{}' field not found.", self.name_field))?;
        let password_field = entries
            .get(&self.password_field)
            .ok_or(eyre!("'{}' field not found.", self.password_field))?;

        let res = db_conn
            .execute(DatabaseInput::QueryValues(
                format!(
                    "SELECT {} FROM {} WHERE {} = ? AND {} = ?",
                    self.user_id_field, self.table_name, self.name_field, self.password_field
                )
                .into(),
                CheapVec::from_vec(vec![
                    sea_orm::Value::from(name_field.to_string()),
                    sea_orm::Value::from(password_field.to_string()),
                ]),
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

        Ok(Some(user_id as usize))
    }

    async fn new(
        &self,
        db_conns: DbConns,
        entries: HashMap<CompactString, CompactString>,
    ) -> Result<UserId> {
        assert_db_backends_length(db_conns.to_owned(), self.name().into())?;

        let db_conn = db_conns.values().next().unwrap();

        let Ok(db_conn) = db_conn
            .to_owned()
            .into_arc_any()
            .downcast::<MySQLConnection>()
        else {
            bail!(
                "Database connection for `MySQLSimple` authentication should be of type {:?} but it's of type {:?}.",
                TypeId::of::<MySQLDbConnsectionConfig>(),
                db_conn.inner_type_id()
            )
        };

        let name_field = entries
            .get(&self.name_field)
            .ok_or(eyre!("'{}' field not found.", self.name_field))?;
        let password_field = entries
            .get(&self.password_field)
            .ok_or(eyre!("'{}' field not found.", self.password_field))?;

        let mut query_input = CheapVec::<_, 8>::from_vec(vec![
            sea_orm::Value::from(name_field.to_string()),
            sea_orm::Value::from(password_field.to_string()),
        ]);

        for extra_field in &self.extra_fields {
            query_input.push(sea_orm::Value::from(
                entries
                    .get(extra_field)
                    .cloned()
                    .ok_or(eyre!("'{}' field not found.", extra_field))
                    .map(|val| val.to_string())?,
            ));
        }

        // Adding the keyword `RETURNING` doesn't allow deserializing the response.
        match db_conn
            .execute(DatabaseInput::QueryValues(
                format!(
                    "INSERT INTO {} ({}) VALUES ({})",
                    self.table_name,
                    [
                        vec![self.name_field.to_owned(), self.password_field.to_owned()],
                        self.extra_fields.to_vec()
                    ]
                    .concat()
                    .join(","),
                    CheapVec::<&str>::from_elem("?", self.extra_fields.len() + 2).join(", "), // +2 as we have count the name and password field.
                )
                .into(),
                query_input,
            ))
            .await
            .map_err(|err| eyre!("Query execution error: {}", err))
        {
            Ok(val) => val,
            Err(err) => {
                if err.to_compact_string().to_lowercase().contains("duplicate") {
                    return Err(eyre!(
                        "Signup failed, an account with the same unique fields already exists."
                    ));
                } else {
                    return Err(err);
                }
            }
        };

        let res = db_conn
            .execute(DatabaseInput::QueryValues(
                format!(
                    "SELECT {} FROM {} WHERE {} = ?",
                    self.user_id_field, self.table_name, self.name_field
                )
                .into(),
                CheapVec::from_vec(vec![sea_orm::Value::from(name_field.to_string())]),
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
            bail!("Unexpected database's executor's output.");
        };

        let Ok(user_id) = entry.try_get::<u32>("", &self.user_id_field) else {
            bail!(
                "Field '{}' expected but not returned in '{}' table. Maybe it exists but the associated data type is not `INT UNSIGNED`.",
                self.user_id_field,
                self.table_name
            )
        };

        Ok(user_id as usize)
    }

    async fn delete(&self, _db_conns: DbConns, _user_id: UserId) -> Result<()> {
        todo!()
    }
}

impl Default for MySQLSimpleAuthentication {
    fn default() -> Self {
        Self {
            database: None,
            table_name: "users_auth".into(),
            user_id_field: "user_id".into(),
            name_field: "email".into(),
            password_field: "password".into(),
            extra_fields: CheapVec::new_const(),
            totp_field: None,
        }
    }
}
