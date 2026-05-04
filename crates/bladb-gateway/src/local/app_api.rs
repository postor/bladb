use super::{auth::AuthSession, AppError};
use serde_json::Value;

#[derive(Clone)]
pub(crate) struct AppApiRequest {
    pub method: String,
    pub path: String,
    pub body: Option<Value>,
    pub session: Option<AuthSession>,
}

pub(crate) trait AppApiHandler: Send + Sync {
    fn can_handle(&self, method: &str, path: &str) -> bool;
    fn handle(&self, request: AppApiRequest) -> Result<Value, AppError>;
}
