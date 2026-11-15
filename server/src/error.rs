use axum::{
    Json,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    BadRequest(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    #[error("{0}")]
    Forbidden(&'static str),
    #[error("{0}")]
    Missing(&'static str),
    #[error("{0}")]
    Gone(&'static str),
    #[error("{0}")]
    Unavailable(&'static str),
    /// `429` with its own code (a quota, not a retry-after rate limit).
    #[error("{0}")]
    TooMany(&'static str),
    /// Seconds to wait.
    #[error("rate limited")]
    RateLimited(u64),
    #[error("{0}")]
    Internal(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::BadRequest(code) => (StatusCode::BAD_REQUEST, *code),
            Self::Conflict(code) => (StatusCode::CONFLICT, *code),
            Self::Forbidden(code) => (StatusCode::FORBIDDEN, *code),
            Self::Missing(code) => (StatusCode::NOT_FOUND, *code),
            Self::Gone(code) => (StatusCode::GONE, *code),
            Self::Unavailable(code) => (StatusCode::SERVICE_UNAVAILABLE, *code),
            Self::TooMany(code) => (StatusCode::TOO_MANY_REQUESTS, *code),
            Self::RateLimited(secs) => {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(axum::http::header::RETRY_AFTER, secs.to_string())],
                    Json(json!({ "error": "rate_limited", "retry_after": secs })),
                )
                    .into_response();
            }
            Self::Image(_) => (StatusCode::BAD_REQUEST, "bad_image"),
            Self::Db(_) | Self::Io(_) | Self::Internal(_) => {
                let mut res = (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "internal" })),
                )
                    .into_response();
                // Logged with the request by `log_server_errors`.
                res.extensions_mut().insert(InternalError(self.to_string()));
                return res;
            }
        };
        (status, Json(json!({ "error": code }))).into_response()
    }
}

/// The text of the error behind a 500, for the log line.
#[derive(Clone, Debug)]
pub struct InternalError(pub String);

/// Logs every 5xx with its method and path (never the query: tokens ride in some) and the
/// error text when the handler failed with an [`AppError`].
pub async fn log_server_errors(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let res = next.run(req).await;
    if res.status().is_server_error() {
        let error = res
            .extensions()
            .get::<InternalError>()
            .map_or("-", |e| e.0.as_str());
        tracing::error!(%method, %path, status = res.status().as_u16(), error, "server error");
    }
    res
}

pub type AppResult<T> = Result<T, AppError>;
