use crate::{lookup::SqlLookup, records::DatabaseRecord, store::store};
use concentus_types::{
    Album, ApiError, Artist, CONTENT_TYPE, Lookup, SCHEMA_HEADER, SCHEMA_REVISION, Tag, Track,
    decode, encode,
};
use serde::Serialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, Method, Path, RouteFn, RouteFuture, RouterBuilder, StatusCode,
        error::ContentTooLargeError, path_param, query_params, request::headers,
        response::Response, to_bytes,
    },
};

const MAX_LOOKUP_BYTES: usize = 16 * 1024;

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    [
        routes::<Artist>(),
        routes::<Album>(),
        routes::<Track>(),
        routes::<Tag>(),
    ]
    .into_iter()
    .flatten()
    .fold(builder, RouterBuilder::route)
}

fn routes<T: DatabaseRecord>() -> [RouteFn; 3] {
    let path = format!("/api/{}", T::NAME);
    [
        RouteFn::new(Method::GET, Path::new(&path).to_owned(), list::<T>),
        RouteFn::new(
            Method::GET,
            Path::new(&format!("{path}/{{id}}")).to_owned(),
            get::<T>,
        ),
        RouteFn::new(
            Method::POST,
            Path::new(&format!("{path}/lookup")).to_owned(),
            lookup::<T>,
        ),
    ]
}

fn list<T: DatabaseRecord>(cx: &Cx, _body: Body) -> RouteFuture<'_> {
    Box::pin(async move {
        if let Some(response) = check_schema(cx)? {
            return Ok(response);
        }
        let Ok(query) = query_params::<PageQuery>(cx) else {
            return error(StatusCode::BAD_REQUEST, "Invalid pagination parameters.");
        };
        match store(cx)
            .list::<T>(query.after.as_deref(), query.limit.unwrap_or(50))
            .await
        {
            Ok(rows) => postcard(StatusCode::OK, &rows),
            Err(cause) => store_error(&cause),
        }
    })
}

fn get<T: DatabaseRecord>(cx: &Cx, _body: Body) -> RouteFuture<'_> {
    Box::pin(async move {
        if let Some(response) = check_schema(cx)? {
            return Ok(response);
        }
        match store(cx).get::<T>(id(cx)).await {
            Ok(Some(record)) => postcard(StatusCode::OK, &record),
            Ok(None) => error(StatusCode::NOT_FOUND, "Record not found."),
            Err(cause) => store_error(&cause),
        }
    })
}

fn lookup<T: DatabaseRecord>(cx: &Cx, body: Body) -> RouteFuture<'_> {
    Box::pin(async move {
        if let Some(response) = check_schema(cx)? {
            return Ok(response);
        }
        let content_type = headers(cx)
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next());
        if !content_type.is_some_and(|value| value.trim().eq_ignore_ascii_case(CONTENT_TYPE)) {
            return error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Expected a Concentus Postcard request.",
            );
        }
        let bytes = match to_bytes(body, MAX_LOOKUP_BYTES).await {
            Ok(bytes) => bytes,
            Err(cause) if cause.is::<ContentTooLargeError>() => {
                return error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Lookup body exceeds its size limit.",
                );
            }
            Err(_) => return error(StatusCode::BAD_REQUEST, "Lookup body could not be read."),
        };
        let Ok(request) = decode::<Lookup<T>>(&bytes) else {
            return error(StatusCode::BAD_REQUEST, "Invalid Postcard lookup request.");
        };
        let criteria = match SqlLookup::from_request(request) {
            Ok(criteria) => criteria,
            Err(message) => return error(StatusCode::BAD_REQUEST, message),
        };
        match store(cx).lookup::<T>(&criteria).await {
            Ok(record) => postcard(StatusCode::OK, &record),
            Err(cause) => store_error(&cause),
        }
    })
}

fn check_schema(cx: &Cx) -> Result<Option<Response>> {
    if headers(cx)
        .get(SCHEMA_HEADER)
        .is_some_and(|value| value == SCHEMA_REVISION)
    {
        return Ok(None);
    }
    error(
        StatusCode::CONFLICT,
        "Incompatible Concentus schema revision.",
    )
    .map(Some)
}

fn postcard(status: StatusCode, message: &impl Serialize) -> Result<Response> {
    let bytes = encode(message).map_err(|cause| topcoat::Error::msg(cause.to_string()))?;
    Ok(Response::builder()
        .status(status)
        .header("content-type", CONTENT_TYPE)
        .header(SCHEMA_HEADER, SCHEMA_REVISION)
        .header("cache-control", "no-store")
        .body(Body::from(bytes))?)
}

fn error(status: StatusCode, message: &str) -> Result<Response> {
    postcard(
        status,
        &ApiError {
            message: message.into(),
        },
    )
}

fn store_error(cause: &topcoat::Error) -> Result<Response> {
    eprintln!("Catalog API storage error: {cause}");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "The catalog store failed.",
    )
}

path_param!(id);

#[query_params(error = bad_request)]
struct PageQuery {
    after: Option<String>,
    limit: Option<u32>,
}
