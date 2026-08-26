// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

pub mod auth;
pub mod databases;
pub mod http_execute;
pub mod schema;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rustyrosetta::*;

use waveless_commons::{databases::*, endpoint::*, project::*, *};

use anyhow::{Result, anyhow, bail};
use async_trait::*;
use chrono::{NaiveDateTime, Utc};
use compact_str::*;
use derive_more::{Constructor, Display};
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
