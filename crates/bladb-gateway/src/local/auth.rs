use super::AppError;
use crate::AuthContext;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct InMemoryAuthService {
    state: Arc<Mutex<AuthState>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InMemoryUserConfig {
    pub app: String,
    pub uid: String,
    pub tenant_id: String,
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub roles: Vec<String>,
}

#[derive(Clone)]
struct AuthState {
    users: HashMap<String, AuthUser>,
    sessions: HashMap<String, AuthSession>,
    next_user_id: u64,
    next_session_id: u64,
}

#[derive(Clone)]
pub(crate) struct AuthUser {
    pub(crate) app: String,
    pub(crate) uid: String,
    pub(crate) tenant_id: String,
    pub(crate) email: String,
    password: String,
    pub(crate) display_name: String,
    pub(crate) roles: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct AuthSession {
    pub(crate) token: String,
    pub(crate) user: AuthUser,
}

impl InMemoryAuthService {
    pub fn from_user_configs(user_configs: Vec<InMemoryUserConfig>) -> Self {
        let mut users = HashMap::new();
        let mut max_user_id = 2999_u64;

        for user in user_configs {
            if let Some(parsed) = parse_uid_counter(&user.uid) {
                max_user_id = max_user_id.max(parsed);
            }

            let auth_user = AuthUser {
                app: user.app.trim().to_ascii_lowercase(),
                uid: user.uid,
                tenant_id: user.tenant_id,
                email: user.email.trim().to_ascii_lowercase(),
                password: user.password,
                display_name: user.display_name,
                roles: user.roles,
            };
            users.insert(user_key(&auth_user.app, &auth_user.email), auth_user);
        }

        Self {
            state: Arc::new(Mutex::new(AuthState {
                users,
                sessions: HashMap::new(),
                next_user_id: max_user_id + 1,
                next_session_id: 1,
            })),
        }
    }

    pub(crate) fn register(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<AuthSession, AppError> {
        let app = normalize_app(app)?;
        if email.trim().is_empty() || password.trim().len() < 6 || display_name.trim().is_empty() {
            return Err(AppError::invalid_request(
                "register requires non-empty email, displayName, and a password with at least 6 characters",
            ));
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("auth state lock poisoned"))?;
        let key = user_key(&app, email);
        if state.users.contains_key(&key) {
            return Err(AppError::invalid_request("user already exists"));
        }

        let (tenant_id, roles) = default_identity_for_app(&state, &app);
        let user_id = state.next_user_id;
        state.next_user_id += 1;
        let session_id = state.next_session_id;
        state.next_session_id += 1;

        let user = AuthUser {
            uid: format!("u_{user_id}"),
            tenant_id,
            app: app.clone(),
            email: email.trim().to_ascii_lowercase(),
            password: password.into(),
            display_name: display_name.trim().into(),
            roles,
        };
        let session = AuthSession {
            token: format!("session-{app}-{session_id}"),
            user: user.clone(),
        };

        state.users.insert(key, user);
        state
            .sessions
            .insert(session.token.clone(), session.clone());

        Ok(session)
    }

    pub(crate) fn login(
        &self,
        app: &str,
        email: &str,
        password: &str,
    ) -> Result<AuthSession, AppError> {
        let app = normalize_app(app)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("auth state lock poisoned"))?;
        let key = user_key(&app, email);
        let user = state
            .users
            .get(&key)
            .cloned()
            .ok_or_else(|| AppError::unauthorized("invalid email or password"))?;

        if user.password != password {
            return Err(AppError::unauthorized("invalid email or password"));
        }

        let session_id = state.next_session_id;
        state.next_session_id += 1;
        let session = AuthSession {
            token: format!("session-{app}-{session_id}"),
            user,
        };
        state
            .sessions
            .insert(session.token.clone(), session.clone());

        Ok(session)
    }

    pub(crate) fn session_from_bearer(&self, bearer_token: &str) -> Result<AuthSession, AppError> {
        let token = bearer_token
            .trim()
            .strip_prefix("Bearer ")
            .or_else(|| bearer_token.trim().strip_prefix("bearer "))
            .unwrap_or_else(|| bearer_token.trim());

        if token.is_empty() {
            return Err(AppError::unauthorized("missing bearer token"));
        }

        let state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("auth state lock poisoned"))?;
        state
            .sessions
            .get(token)
            .cloned()
            .ok_or_else(|| AppError::unauthorized("session expired or token is invalid"))
    }

    pub(crate) fn logout(&self, bearer_token: &str) -> Result<Value, AppError> {
        let token = bearer_token
            .trim()
            .strip_prefix("Bearer ")
            .or_else(|| bearer_token.trim().strip_prefix("bearer "))
            .unwrap_or_else(|| bearer_token.trim());

        if token.is_empty() {
            return Err(AppError::unauthorized("missing bearer token"));
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("auth state lock poisoned"))?;
        let revoked = state.sessions.remove(token).is_some();

        if !revoked {
            return Err(AppError::unauthorized(
                "session expired or token is invalid",
            ));
        }

        Ok(json!({ "revoked": true }))
    }
}

impl AuthUser {
    pub(crate) fn auth_context(&self) -> AuthContext {
        AuthContext {
            uid: Some(self.uid.clone()),
            tenant_id: Some(self.tenant_id.clone()),
            roles: self.roles.clone(),
            permission_version: Some("v1".into()),
        }
    }
}

impl AuthSession {
    pub(crate) fn to_public_json(&self) -> Value {
        json!({
            "token": self.token,
            "user": {
                "app": self.user.app,
                "uid": self.user.uid,
                "tenantId": self.user.tenant_id,
                "email": self.user.email,
                "displayName": self.user.display_name,
                "roles": self.user.roles
            }
        })
    }
}

fn user_key(app: &str, email: &str) -> String {
    format!(
        "{}:{}",
        app.trim().to_ascii_lowercase(),
        email.trim().to_ascii_lowercase()
    )
}

fn normalize_app(app: &str) -> Result<String, AppError> {
    let normalized = app.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::invalid_request("app is required"));
    }

    Ok(normalized)
}

fn default_identity_for_app(state: &AuthState, app: &str) -> (String, Vec<String>) {
    state
        .users
        .values()
        .find(|user| user.app == app)
        .map(|user| (user.tenant_id.clone(), user.roles.clone()))
        .unwrap_or_else(|| ("tenant_local".into(), vec!["member".into()]))
}

fn parse_uid_counter(uid: &str) -> Option<u64> {
    uid.strip_prefix("u_")
        .and_then(|suffix| suffix.parse::<u64>().ok())
}
