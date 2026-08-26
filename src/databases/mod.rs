// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

pub mod any_sql;

pub mod mysql;
pub mod postgres;

pub use any_sql::*;

use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseConnectionType, Statement};
use sqlx::pool::*;
