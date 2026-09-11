// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

use super::*;

/// TODO: add docs here.
#[derive(Clone, Constructor, Serialize, Deserialize, BoxedAny, Getters, Display, Debug)]
#[display("SQL backed users' roles check on {}", table_name)]
#[getset(get = "pub")]
pub struct MySQLRole {
    /// Will use the primary database by default.
    #[serde(default, skip_serializing_if = "should_skip_option")]
    database: Option<DatabaseId>,

    table_name: CompactString,

    user_id_field: CompactString,

    /// Must not be primary key.
    role_field: CompactString,
}

impl AnyExt for MySQLRole {
    fn name(&self) -> &str {
        "mysqlrole"
    }
}

impl DatabaseConsumer for MySQLRole {
    fn databases(&self) -> CheapVec<DatabaseId> {
        if let Some(database) = &self.database {
            CheapVec::from_iter([database.to_owned()])
        } else {
            CheapVec::new_const()
        }
    }
}

#[typetag::serde(name = "MySQLRole")]
#[async_trait]
impl AnyRoleMethod for MySQLRole {
    async fn get(&self, db_conns: DbConns, user_id: UserId) -> Result<Option<CompactString>> {
        assert_db_backends_length(db_conns.to_owned(), self.name().into())?;

        let db_conn = db_conns.values().next().unwrap();

        let Ok(db_conn) = db_conn
            .to_owned()
            .into_arc_any()
            .downcast::<MySQLConnection>()
        else {
            bail!(
                "Database connection for `MySQLRole` role method should be of type {:?} but it's of type {:?}.",
                TypeId::of::<MySQLDbConnectionConfig>(),
                db_conn.inner_type_id()
            )
        };

        let res = db_conn
            .execute(DatabaseInput::QueryValues(
                format!(
                    "SELECT {} FROM {} WHERE {} = ?",
                    self.role_field, self.table_name, self.user_id_field
                )
                .into(),
                CheapVec::from_vec(vec![user_id.to_string().into()]),
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

        let Ok(role) = entry.try_get::<String>("", &self.role_field) else {
            bail!(
                "Field '{}' expected but not returned in '{}' table. Maybe it exists but the associated data type is not `VARCHAR`.",
                self.user_id_field,
                self.table_name
            )
        };

        Ok(Some(role.into()))
    }

    async fn set(&self, _db_conns: DbConns, _user_id: UserId, _role: CompactString) -> Result<()> {
        todo!()
    }

    async fn remove(&self, _db_conns: DbConns, _user_id: UserId) -> Result<()> {
        todo!()
    }
}

impl Default for MySQLRole {
    fn default() -> Self {
        Self {
            database: None,
            table_name: "roles_auth".into(),
            user_id_field: "user_id".into(),
            role_field: "role".into(),
        }
    }
}
