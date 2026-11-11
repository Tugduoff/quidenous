use axum::{
    Json,
    http::StatusCode,
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
            Self::Image(_) => (StatusCode::BAD_REQUEST, "bad_image"),
            Self::Db(_) | Self::Io(_) => {
                tracing::error!(error = %self, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        (status, Json(json!({ "error": code }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
