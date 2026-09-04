// waveless_sql
// Copyright (C) 2026 Oscar Alvarez Gonzalez

use crate::*;

use super::*;

use sea_orm::{FromQueryResult, QueryResult};

/// Beware that the params are expected to be `ExecuteParams::StringMap`
/// and the output will be a `serde_json::Value` that will be
/// further serialized into JSON.
pub async fn any_sql_execute(
    queries: &CheapVec<SQLQuery>,
    cx: RequestCx,
    db_conns: DbConns,
) -> Result<HttpResponse, RequestError> {
    let RequestCx {
        method,
        request_params: params,
        endpoint,
        ..
    } = cx;

    assert_db_backends_length(db_conns.to_owned(), endpoint.id().to_owned())?;

    let db_conn = db_conns.values().next().unwrap();

    let mut queries = queries.iter();

    let mut res_buffer = CheapVec::<serde_json::Value>::new();

    while let Some(sql_query) = queries.next() {
        // Replaces Waveless' client's query's parameters placeholders with SQL's ones.
        let params_order = sql_query
            .query()
            .trim_start_matches(|c| c != '{')
            .split('{')
            .map(|sub| sub.split_once('}').unwrap_or_default().0.trim())
            .filter(|sub| !sub.is_empty())
            .collect::<CheapVec<&str>>();

        let mut query = sql_query
            .query()
            .split('{')
            .map(|sub| {
                if sub.contains('}') {
                    sub.trim_start_matches(|c| c != '}').replace('}', "?")
                } else {
                    sub.to_string()
                }
            })
            .collect::<CompactString>();

        // Replaces Waveless' runtime injected query's parameters placeholders with the value.
        // NOTE: the value will be replaced directly in the SQL query,
        // be aware that a malformed runtime parameter might cause a SQL
        // injection attack (the attack vector could be in malicious
        // authentications, sessions, roles methods implementations).
        query = query
            .split('|')
            .enumerate()
            .map(|(i, sub)| {
                if i % 2 != 0 {
                    if let Some(ParamValue::Internal(value)) = params.get(sub.trim()) {
                        Ok(value.to_compact_string())
                    } else {
                        Err(RequestError::Expected(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!(
                                "Expected the runtime parameter `{}`, but it was not injected.",
                                sub
                            )
                            .into(),
                        ))
                    }
                } else {
                    Ok(sub.into())
                }
            })
            .collect::<Result<CompactString, RequestError>>()?;

        // Gets parameter values in the order they appear.
        let mut ordered_values = CheapVec::<_, 8>::new();

        for param_id in params_order.iter() {
            match params
                .get(&param_id.to_compact_string())
                .map(|opt| {
                    if let ParamValue::Client(param) = opt {
                        param.to_owned()
                    } else {
                        None
                    }
                })
                .flatten()
            {
                Some(value) => ordered_values.push(value),
                None => {
                    if method == HttpMethod::Put {
                        // Modifies the query and strip `?`'s at the positions.
                        // As it is a PUT query we have to strip the column's name, '?' at the current position

                        let re = regex::Regex::new(
                            format!(r#",\s*{}\s*=\s*\?|{}\s*=\s*\?\s*,?"#, param_id, param_id)
                                .as_str(),
                        )
                        .map_err(|err| {
                            RequestError::Expected(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                format!(
                                    "Cannot create the regex to extract '{}' from the query: {}",
                                    param_id, err
                                )
                                .into(),
                            )
                        })?;

                        query = re.replace_all(&query, "").into();
                    } else {
                        return Err(RequestError::Expected(
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                                format!(
                                                    "The endpoint requires '{}', but it wasn't provided in the request.",
                                                    param_id
                                                )
                                                .into(),
                                            ));
                    }
                }
            }
        }

        let res = db_conn
            .execute(DatabaseInput::QueryValues(query, ordered_values))
            .await
            .map_err(|err| {
                RequestError::Expected(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Query execution error: {}", err).into(),
                )
            })?;

        let DatabaseOutput::Any(res) = res else {
            return Err(RequestError::Other(eyre!(
                "Unexpected database's executor's output."
            )));
        };

        let res = res.downcast::<Vec<QueryResult>>().map_err(|err| {
            RequestError::Other(eyre!("Cannot downcast to SQL query result. {:?}", err))
        })?;

        let mut rows = CheapVec::<_, 0>::new();

        for row in *res {
            rows.push(
                sea_orm::JsonValue::from_query_result(&row, "").map_err(|err| {
                    RequestError::Expected(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Internal error: cannot serialize row into JSON. {}", err).into(),
                    )
                })?,
            );
        }

        match sql_query.behaviour() {
                    SQLBehaviour::FailOnEmpty if rows.is_empty() => {
                        Err(RequestError::Expected(
                            StatusCode::BAD_REQUEST,
                            format!("Query result cannot be empty. HINT: error triggered because `FailOnEmpty` is enabled for this `SQL` execution context, maybe you want to set it to `Permissive`?",).into(),
                        ))?
                    }
                    SQLBehaviour::Unique if rows.len() != 1 => Err(RequestError::Expected(
                        StatusCode::BAD_REQUEST,
                        format!("Resource does not exist. HINT: error triggered because `Unique` is enabled for this `SQL` execution context, maybe you want to set it to `Permissive`?",).into(),
                    ))?,
                    _ => (),
                }

        if *sql_query.include() {
            res_buffer.push(match sql_query.behaviour() {
                SQLBehaviour::Unique => json!(rows.first().unwrap()),
                _ => json!(&rows),
            });
        }
    }

    match res_buffer.len() {
        0 => Ok(HttpResponse::new(None, None)),
        1 => Ok(HttpResponse::new(
            None,
            Some(BodyValue::Json(res_buffer.last().unwrap().to_owned())),
        )),
        _ => Ok(HttpResponse::new(
            None,
            Some(BodyValue::Json(json!(res_buffer))),
        )),
    }
}
