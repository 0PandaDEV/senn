use std::fmt::{self, Display};

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.status, self.message)
    }
}

impl std::error::Error for ApiError {}

pub fn plain(status: u16, message: &str) -> topcoat::Error {
    ApiError {
        status,
        message: message.to_string(),
    }
    .into()
}
