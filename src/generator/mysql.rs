// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

use super::*;

use databases::mysql::*;
use http_executor::{mysql::*, *};

/// The MySQL discovery strategy will analyze a MySQL database in order to generate a representation of the data model that will be analyzed by the endpoint generator backend.
#[derive(
    Clone, PartialEq, Constructor, Serialize, Deserialize, BoxedAny, Getters, Display, Debug,
)]
#[display("MySQL schema discovery (skipping: {:?})", skip_tables)]
#[getset(get = "pub")]
pub struct MySQLSchemaDiscovery {
    database: DatabaseId,

    #[serde(default, skip_serializing_if = "should_skip_cheapvec")]
    skip_tables: CheapVec<CompactString, 0>, // Do not forget that auth, session and role tables are also skipped
}

impl AnyExt for MySQLSchemaDiscovery {
    fn name(&self) -> &str {
        "mysql_schema"
    }
}

#[typetag::serde(name = "MySQL")]
#[async_trait]
impl AnyEndpointGenerator for MySQLSchemaDiscovery {
    async fn generate(&self, db_conns: DbConns) -> Result<(Endpoints, Option<Bytes>)> {
        let (_, db_conn) = db_conns
            .iter()
            .find(|(db_id, _)| db_id == self.database())
            .wrap_err(format!(
                "Could't find the associated database id `{}`",
                self.database()
            ))?;

        let Ok(mysql_conn) = db_conn
            .to_owned()
            .into_arc_any()
            .downcast::<MySQLConnection>()
        else {
            bail!(
                "Database connection config should be of type {:?} but it's of type {:?}.",
                TypeId::of::<MySQLConnection>(),
                db_conn.inner_type_id()
            )
        };

        let conn_pool = mysql_conn.get().get_mysql_connection_pool();

        let schema = sea_schema::mysql::discovery::SchemaDiscovery::new(
            (*conn_pool).to_owned(),
            &self.database,
        )
        .discover()
        .await?;

        let mut endpoints = Endpoints::new_unchecked(CheapVec::new_const());

        let checksum = CheapVec::<u8, 0>::from_slice(
            &crc32fast::hash(format!("{:?}", schema).as_str().as_bytes()).to_ne_bytes(),
        );

        // For each table generate a GET one, GET many, POST, UPDATE and DELETE endpoints.
        for table in schema.tables {
            if self
                .skip_tables()
                .contains(&table.info.name.to_compact_string())
            {
                continue;
            }

            // Check whether the table is a view, only the GET many endpoint will be generated.
            let is_view = table.info.comment.to_lowercase().eq("view");

            // Get the table primary key. If it is not present only the GET one and POST endpoints will generated.
            let pk_id = table
                .columns
                .iter()
                .find(|column| column.key == sea_schema::mysql::def::ColumnKey::Primary)
                .map(|table| table.name.to_owned());

            if pk_id.is_none() {
                debug!(
                    "Table {} doesn't have a primary key. Only GET many and POST endpoints will be generated.",
                    table.info.name.to_owned()
                )
            }

            let columns_names = table
                .columns
                .iter()
                .filter(|column| column.key != sea_schema::mysql::def::ColumnKey::Primary)
                .map(|column| column.name.to_compact_string())
                .collect::<CheapVec<CompactString>>();

            let route_one: CompactString =
                format!("{}/{}", table.info.name.to_lowercase(), "{id}").into();
            let route_many: CompactString = table.info.name.to_lowercase().into();

            for method in &[
                HttpMethod::Get,
                HttpMethod::Post,
                HttpMethod::Put,
                HttpMethod::Delete,
            ] {
                match (method, &pk_id) {
                    (HttpMethod::Get, _) => {
                        match &pk_id {
                            Some(pk_id) if !is_view => {
                                let mut endpoint_one = EndpointBuilder::default();

                                endpoint_one
                                    .id(format!("{}_GetOne", table.info.name.to_owned()).into())
                                    .description(
                                        format!(
                                            "Get row from {} by it's primary key.",
                                            table.info.name
                                        )
                                        .into(),
                                    )
                                    .databases(CheapVec::from_iter([self.database().to_owned()]))
                                    .execution_target(ExecutionTarget::Http(
                                        HttpTargetBuilder::default()
                                            .method(*method)
                                            .version("v1".into())
                                            .route(route_one.to_owned())
                                            .execution_pipeline(
                                                Arc::<MySQLExecutor>::new(
                                                    SQLQueryWrapper::new(
                                                        format!(
                                                            "SELECT * FROM {} WHERE {} = {}",
                                                            table.info.name, pk_id, "{id}"
                                                        )
                                                        .into(),
                                                    )
                                                    .with_behaviour(SQLBehaviour::Unique)
                                                    .into(),
                                                )
                                                .into(),
                                            )
                                            .query_params(CheapVec::new_const())
                                            .body_params(CheapVec::new_const())
                                            .capture_all_params(false)
                                            .auto_generated(true)
                                            .build()?,
                                    ))
                                    .tags(CheapVec::from_vec(vec![
                                        table.info.name.to_compact_string(),
                                        "get_one".into(),
                                    ]))
                                    .deprecated(false);

                                endpoints.add(endpoint_one.build()?)?;
                            }
                            _ => (),
                        }

                        let mut endpoint_many = EndpointBuilder::default();

                        endpoint_many
                            .id(format!("{}_GetMany", table.info.name.to_owned()).into())
                            .databases(CheapVec::from_iter([self.database().to_owned()]))
                            .execution_target(ExecutionTarget::Http(
                                HttpTargetBuilder::default()
                                    .method(*method)
                                    .version("v1".into())
                                    .route(route_many.to_owned())
                                    .execution_pipeline(
                                        Arc::<MySQLExecutor>::new(
                                            SQLQueryWrapper::new(
                                                format!("SELECT * FROM {}", table.info.name,)
                                                    .into(),
                                            )
                                            .into(),
                                        )
                                        .into(),
                                    )
                                    .query_params(CheapVec::new_const())
                                    .body_params(CheapVec::new_const())
                                    .capture_all_params(false)
                                    .auto_generated(true)
                                    .build()?,
                            ))
                            .description(format!("Get all rows from {}.", table.info.name).into())
                            .tags(CheapVec::from_vec(vec![
                                table.info.name.to_compact_string(),
                                "get_all".into(),
                            ]))
                            .deprecated(false);

                        endpoints.add(endpoint_many.build()?)?;
                    }
                    (HttpMethod::Post, _) if !is_view => {
                        let mut endpoint = EndpointBuilder::default();

                        endpoint
                            .id(format!("{}_Post", table.info.name).into())
                            .databases(CheapVec::from_iter([self.database().to_owned()]))
                            .execution_target(ExecutionTarget::Http(
                                HttpTargetBuilder::default()
                                    .method(*method)
                                    .version("v1".into())
                                    .route(route_many.to_owned())
                                    .execution_pipeline(
                                        Arc::<MySQLExecutor>::new(
                                            SQLQueryWrapper::new(
                                                format!(
                                                    "INSERT INTO {} ({}) VALUES ({})",
                                                    table.info.name,
                                                    columns_names
                                                        .iter()
                                                        .fold(String::new(), |last, next| format!(
                                                            "{}, {}",
                                                            last, next
                                                        ))
                                                        .trim_matches(
                                                            |c: char| c.is_whitespace() || c == ','
                                                        ),
                                                    columns_names
                                                        .iter()
                                                        .fold(String::new(), |last, next| format!(
                                                            "{}, {{ {} }}",
                                                            last, next
                                                        ))
                                                        .trim_matches(
                                                            |c: char| c.is_whitespace() || c == ','
                                                        ),
                                                )
                                                .into(),
                                            )
                                            .with_include(false)
                                            .into(),
                                        )
                                        .into(),
                                    )
                                    .query_params(CheapVec::new_const())
                                    .body_params(columns_names.to_owned())
                                    .capture_all_params(false)
                                    .auto_generated(true)
                                    .build()?,
                            ))
                            .description(format!("Insert data into {}.", table.info.name).into())
                            .tags(CheapVec::from_vec(vec![
                                table.info.name.to_compact_string(),
                                "post".into(),
                            ]))
                            .deprecated(false);

                        endpoints.add(endpoint.build()?)?;
                    }
                    (HttpMethod::Put, Some(pk_id)) if !is_view => {
                        let mut endpoint = EndpointBuilder::default();

                        endpoint
                            .id(format!("{}_Put", table.info.name).into())
                            .databases(CheapVec::from_iter([self.database().to_owned()]))
                            .execution_target(ExecutionTarget::Http(
                                HttpTargetBuilder::default()
                                    .method(*method)
                                    .version("v1".into())
                                    .route(route_one.to_owned())
                                    .execution_pipeline(
                                        Arc::<MySQLExecutor>::new(
                                            SQLQueryWrapper::new(
                                                format!(
                                                    "UPDATE {} SET {} WHERE {} = {} ",
                                                    table.info.name,
                                                    columns_names
                                                        .iter()
                                                        .map(|name| format!(
                                                            "{} = {{ {} }}",
                                                            name, name
                                                        ))
                                                        .fold(String::new(), |last, next| format!(
                                                            "{}, {}",
                                                            last, next
                                                        ))
                                                        .trim_matches(
                                                            |c: char| c.is_whitespace() || c == ','
                                                        ),
                                                    pk_id,
                                                    "{id}"
                                                )
                                                .into(),
                                            )
                                            .with_include(false)
                                            .into(),
                                        )
                                        .into(),
                                    )
                                    .query_params(CheapVec::new_const())
                                    .body_params(columns_names.to_owned())
                                    .capture_all_params(false)
                                    .auto_generated(true)
                                    .build()?,
                            ))
                            .description(
                                format!(
                                    "Updates {} on row with the given primary key.",
                                    table.info.name
                                )
                                .into(),
                            )
                            .tags(CheapVec::from_vec(vec![
                                table.info.name.to_compact_string(),
                                "put".into(),
                            ]))
                            .deprecated(false);

                        endpoints.add(endpoint.build()?)?;
                    }
                    (HttpMethod::Delete, Some(pk_id)) if !is_view => {
                        let mut endpoint = EndpointBuilder::default();

                        endpoint
                            .id(format!("{}_Delete", table.info.name).into())
                            .databases(CheapVec::from_iter([self.database().to_owned()]))
                            .execution_target(ExecutionTarget::Http(
                                HttpTargetBuilder::default()
                                    .method(*method)
                                    .version("v1".into())
                                    .route(route_one.to_owned())
                                    .execution_pipeline(
                                        Arc::<MySQLExecutor>::new(
                                            SQLQueryWrapper::new(
                                                format!(
                                                    "DELETE FROM {} WHERE {} = {} ",
                                                    table.info.name, pk_id, "{id}"
                                                )
                                                .into(),
                                            )
                                            .with_include(false)
                                            .into(),
                                        )
                                        .into(),
                                    )
                                    .query_params(CheapVec::new_const())
                                    .body_params(CheapVec::new_const())
                                    .capture_all_params(false)
                                    .auto_generated(true)
                                    .build()?,
                            ))
                            .description(
                                format!(
                                    "Deletes data from {} with the given primary key.",
                                    table.info.name
                                )
                                .into(),
                            )
                            .tags(CheapVec::from_vec(vec![
                                table.info.name.to_compact_string(),
                                "delete".into(),
                            ]))
                            .deprecated(false);

                        endpoints.add(endpoint.build()?)?;
                    }
                    _ => {}
                }
            }
        }

        Ok((endpoints, Some(checksum)))
    }

    fn id(&self) -> Result<Bytes> {
        Ok([self.name(), ":", self.database()]
            .concat()
            .into_bytes()
            .into())
    }
}
