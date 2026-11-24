use crate::{
    catalog::Kind,
    enrichment::{Discovery, Enrichment},
    lookup::SqlLookup,
    records::DatabaseRecord,
    store::{LookupMatch, store},
};
use concentus_types::{
    Album, ApiError, Artist, CONTENT_TYPE, Lookup, PageQuery, Tag, Track, decode, encode,
};
use serde::Serialize;
use topcoat::{
    Result,
    context::{Cx, context},
    router::{
        Body, Method, Path, RouteFn, RouteFuture, RouterBuilder, StatusCode,
        content::Form,
        error::ContentTooLargeError,
        path_param,
        request::{FromRequest, headers},
        response::Response,
        to_bytes,
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

fn list<T: DatabaseRecord>(cx: &Cx, body: Body) -> RouteFuture<'_> {
    Box::pin(async move {
        let Ok(Form(query)) = Form::<PageQuery>::from_request(cx, body).await else {
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
        match store(cx).get::<T>(id(cx)).await {
            Ok(Some(record)) => accessed::<T>(cx, id(cx), &record),
            Ok(None) => error(StatusCode::NOT_FOUND, "Record not found."),
            Err(cause) => store_error(&cause),
        }
    })
}

fn lookup<T: DatabaseRecord>(cx: &Cx, body: Body) -> RouteFuture<'_> {
    Box::pin(async move {
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
        let kind = Kind::parse(T::NAME);
        let seed = kind.and_then(|kind| Discovery::from_lookup(kind, &request));
        let criteria = match SqlLookup::from_request(request) {
            Ok(criteria) => criteria,
            Err(message) => return error(StatusCode::BAD_REQUEST, message),
        };
        let db = store(cx);
        let mut result = db.lookup::<T>(&criteria).await;
        if matches!(result, Ok(LookupMatch::Missing))
            && let (Some(kind), Some(seed)) = (kind, seed)
        {
            tracing::info!(
                collection = T::NAME,
                "Lookup missed locally, trying enrichment"
            );
            let enrichment: &Enrichment = context(cx);
            enrichment.discover(db, kind, seed).await;
            result = db.lookup::<T>(&criteria).await;
        }
        let outcome = match &result {
            Ok(LookupMatch::Found(..)) => "found",
            Ok(LookupMatch::Missing) => "missing",
            Ok(LookupMatch::Ambiguous) => "ambiguous",
            Err(_) => "error",
        };
        tracing::info!(collection = T::NAME, outcome, "Catalog lookup completed");
        match result {
            Ok(LookupMatch::Found(id, record)) => accessed::<T>(cx, &id, &Some(record)),
            Ok(LookupMatch::Missing | LookupMatch::Ambiguous) => {
                postcard(StatusCode::OK, &Option::<T>::None)
            }
            Err(cause) => store_error(&cause),
        }
    })
}

fn accessed<T: DatabaseRecord>(cx: &Cx, id: &str, record: &impl Serialize) -> Result<Response> {
    let response = postcard(StatusCode::OK, record)?;
    if Kind::parse(T::NAME).is_some_and(|kind| Kind::MUSIC.contains(&kind)) {
        store(cx).record_access(id);
    }
    Ok(response)
}

fn postcard(status: StatusCode, message: &impl Serialize) -> Result<Response> {
    let bytes = encode(message).map_err(|cause| topcoat::Error::msg(cause.to_string()))?;
    Ok(Response::builder()
        .status(status)
        .header("content-type", CONTENT_TYPE)
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
    tracing::error!(error = %cause, "Catalog API storage error");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "The catalog store failed.",
    )
}

path_param!(id);
