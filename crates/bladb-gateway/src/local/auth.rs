use super::AppError;
use crate::AuthContext;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const DEFAULT_SESSION_LEASE_SECONDS: u64 = 60 * 60 * 24 * 30;

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
    pub(crate) anonymous: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthSessionKind {
    Authenticated,
    Anonymous,
}

#[derive(Clone)]
pub(crate) struct AuthSession {
    pub(crate) token: String,
    pub(crate) user: AuthUser,
    pub(crate) kind: AuthSessionKind,
    issued_at: u64,
    last_seen_at: u64,
    expires_at: u64,
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
                anonymous: false,
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
            anonymous: false,
        };
        let session = new_session(
            &app,
            session_id,
            user.clone(),
            AuthSessionKind::Authenticated,
        );

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
        let session = new_session(&app, session_id, user, AuthSessionKind::Authenticated);
        state
            .sessions
            .insert(session.token.clone(), session.clone());

        Ok(session)
    }

    pub(crate) fn ensure_anonymous_session(&self, app: &str) -> Result<AuthSession, AppError> {
        let app = normalize_app(app)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("auth state lock poisoned"))?;
        let (tenant_id, roles) = default_identity_for_app(&state, &app);
        let user_id = state.next_user_id;
        state.next_user_id += 1;
        let session_id = state.next_session_id;
        state.next_session_id += 1;
        let uid = format!("anon_{}_{}", app.replace('-', "_"), user_id);

        let user = AuthUser {
            app: app.clone(),
            uid: uid.clone(),
            tenant_id,
            email: anonymous_email(&app, &uid),
            password: String::new(),
            display_name: anonymous_display_name(&app, user_id),
            roles,
            anonymous: true,
        };
        let session = new_session(&app, session_id, user, AuthSessionKind::Anonymous);
        state
            .sessions
            .insert(session.token.clone(), session.clone());

        Ok(session)
    }

    pub(crate) fn session_from_bearer(&self, bearer_token: &str) -> Result<AuthSession, AppError> {
        let token = sanitize_session_token(bearer_token)?;
        self.resolve_session(&token, None)
    }

    pub(crate) fn session_from_cookie(
        &self,
        app: &str,
        cookie_token: &str,
    ) -> Result<AuthSession, AppError> {
        let app = normalize_app(app)?;
        let token = sanitize_cookie_token(cookie_token)?;
        self.resolve_session(&token, Some(app.as_str()))
    }

    pub(crate) fn logout(&self, session_token: &str) -> Result<Value, AppError> {
        let token = sanitize_session_token(session_token)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("auth state lock poisoned"))?;
        let revoked = state.sessions.remove(&token).is_some();

        if !revoked {
            return Err(AppError::unauthorized(
                "session expired or token is invalid",
            ));
        }

        Ok(json!({ "revoked": true }))
    }

    fn resolve_session(
        &self,
        token: &str,
        expected_app: Option<&str>,
    ) -> Result<AuthSession, AppError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::internal("auth state lock poisoned"))?;
        let now = now_epoch_seconds();
        let Some(existing) = state.sessions.get(token).cloned() else {
            return Err(AppError::unauthorized(
                "session expired or token is invalid",
            ));
        };

        if existing.expires_at <= now {
            state.sessions.remove(token);
            return Err(AppError::unauthorized(
                "session expired or token is invalid",
            ));
        }

        if let Some(app) = expected_app {
            if existing.user.app != app {
                return Err(AppError::unauthorized(
                    "session does not belong to the requested app",
                ));
            }
        }

        let mut renewed = existing;
        renewed.last_seen_at = now;
        renewed.expires_at = now + DEFAULT_SESSION_LEASE_SECONDS;
        state.sessions.insert(token.to_string(), renewed.clone());

        Ok(renewed)
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
            "sessionKind": self.kind.as_str(),
            "anonymous": self.kind == AuthSessionKind::Anonymous,
            "issuedAt": self.issued_at,
            "lastSeenAt": self.last_seen_at,
            "expiresAt": self.expires_at,
            "user": {
                "app": self.user.app,
                "uid": self.user.uid,
                "tenantId": self.user.tenant_id,
                "email": self.user.email,
                "displayName": self.user.display_name,
                "roles": self.user.roles,
                "anonymous": self.user.anonymous
            }
        })
    }

    pub(crate) fn cookie_max_age_seconds(&self) -> u64 {
        self.expires_at.saturating_sub(now_epoch_seconds())
    }
}

impl AuthSessionKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Authenticated => "authenticated",
            Self::Anonymous => "anonymous",
        }
    }
}

fn new_session(app: &str, session_id: u64, user: AuthUser, kind: AuthSessionKind) -> AuthSession {
    let now = now_epoch_seconds();
    let suffix = match kind {
        AuthSessionKind::Authenticated => "session",
        AuthSessionKind::Anonymous => "anon",
    };

    AuthSession {
        token: format!("{suffix}-{app}-{session_id}"),
        user,
        kind,
        issued_at: now,
        last_seen_at: now,
        expires_at: now + DEFAULT_SESSION_LEASE_SECONDS,
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

fn sanitize_session_token(value: &str) -> Result<String, AppError> {
    let token = value
        .trim()
        .strip_prefix("Bearer ")
        .or_else(|| value.trim().strip_prefix("bearer "))
        .unwrap_or_else(|| value.trim())
        .trim();

    if token.is_empty() {
        return Err(AppError::unauthorized("missing bearer token"));
    }

    Ok(token.to_string())
}

fn sanitize_cookie_token(value: &str) -> Result<String, AppError> {
    let token = value.trim();
    if token.is_empty() {
        return Err(AppError::unauthorized("missing session cookie"));
    }

    Ok(token.to_string())
}

fn anonymous_display_name(app: &str, user_id: u64) -> String {
    format!("Anonymous {} {}", app.replace('-', " "), user_id)
}

fn anonymous_email(app: &str, uid: &str) -> String {
    format!("anon+{uid}@{app}.local")
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
