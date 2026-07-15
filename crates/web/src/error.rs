use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub enum ApiError {
    NotFound,
    BadRequest(String),
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::NotFound => {
                (StatusCode::NOT_FOUND, "device not found".to_string()).into_response()
            }
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            ApiError::Internal(e) => {
                tracing::error!("internal error: {e:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        }
    }
}

pub fn parse_device_id(raw: &str) -> Result<flock_core::DeviceId, ApiError> {
    raw.parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid device id: {raw}")))
}
