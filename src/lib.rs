// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

pub mod auth;
pub mod databases;
pub mod generator;
pub mod http_executor;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use boxed_any::*;
use boxed_any_derive::*;
use rustyrosetta::*;

use waveless_commons::{databases::*, endpoint::*, project::*, *};

use async_trait::*;
use chrono::{NaiveDateTime, Utc};
use color_eyre::Section;
use compact_str::*;
use derive_more::{Constructor, Display};
use eyre::{Context, ContextCompat, Result, bail, eyre};
use getset::*;
use http::StatusCode;
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::*;

/// Forces the linker to retain the types by executing its static `typetag` initializers.
///
/// Call this function once at the start of `main()` in the host binary to prevent dead-code
/// elimination (DCE) from stripping this crate's registered types.
#[inline(always)]
pub fn register() {}

#[inline]
pub(crate) fn assert_db_backends_length(
    db_conns: DbConns,
    origin: CompactString,
) -> Result<(), RequestError> {
    match db_conns.len() != 1 {
        true => Err(RequestError::Other(
            eyre!("`waveless_sql` does not support multiple (without id) or a missing database backends.")
                .note(format!("Loaded database backends: {}", db_conns.len()))
                .suggestion(format!(
                    "Add exactly one database backend or specify it's id (executor: `{}`).",
                    origin,
                )),
        )),
        false => Ok(()),
    }
}
