// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

use super::*;

use sea_orm::Value; // Switched from sqlx, as sqlx doesn't support conversion into JSON for arbitrary schemas.

#[derive(Clone, Constructor, Debug)]
pub struct AnySQLConnection<'a>(&'a DatabaseConnection);

impl<'a> AnySQLConnection<'a> {
    pub async fn execute(&self, input: DatabaseInput) -> Result<DatabaseOutput> {
        match input {
            DatabaseInput::Query(query) => {
                let stmt = Statement::from_string(self.0.get_database_backend(), query.to_string());

                let res = self.0.query_all_raw(stmt).await?;

                Ok(DatabaseOutput::Any(Box::new(res)))
            }
            DatabaseInput::QueryValues(query, params) => {
                let stmts = Statement::from_sql_and_values(
                    self.0.get_database_backend(),
                    query.to_string(),
                    params.iter().map(|param| Value::from(param.to_string())),
                );

                let res = self.0.query_all_raw(stmts).await?;

                Ok(DatabaseOutput::Any(Box::new(res)))
            }
            _ => Err(eyre!("Unsupported input for SQL query.")),
        }
    }
}
