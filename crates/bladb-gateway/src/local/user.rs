use super::{
    auth::{AuthSession, InMemoryAuthService, InMemoryUserConfig},
    config::{OfficialUsersFeaturesConfig, OfficialUsersModuleConfig},
    AppError,
};
use crate::default_http_agent;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct OfficialUserModule {
    provider: Arc<dyn UserModuleProvider>,
    enabled: bool,
    assembly: OfficialUserAssembly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCookie {
    app: String,
    token: String,
    max_age_seconds: u64,
    clearing: bool,
}

impl SessionCookie {
    pub(crate) fn from_session(session: &AuthSession) -> Self {
        Self {
            app: session.user.app.clone(),
            token: session.token.clone(),
            max_age_seconds: session.cookie_max_age_seconds(),
            clearing: false,
        }
    }

    pub(crate) fn clearing(app: &str) -> Self {
        Self {
            app: app.trim().to_ascii_lowercase(),
            token: "expired".into(),
            max_age_seconds: 0,
            clearing: true,
        }
    }

    pub fn cookie_name_for_app(app: &str) -> String {
        format!(
            "bladb_sid_{}",
            app.trim()
                .to_ascii_lowercase()
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() {
                        character
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        )
    }

    pub fn header_value(&self) -> String {
        let mut parts = vec![
            format!("{}={}", Self::cookie_name_for_app(&self.app), self.token),
            "Path=/".into(),
            format!("Max-Age={}", self.max_age_seconds),
            "HttpOnly".into(),
            "SameSite=Lax".into(),
        ];
        if self.clearing {
            parts.push("Expires=Thu, 01 Jan 1970 00:00:00 GMT".into());
        }
        parts.join("; ")
    }
}

impl OfficialUserModule {
    pub fn from_config(
        config: Option<OfficialUsersModuleConfig>,
        seed_users: Vec<InMemoryUserConfig>,
    ) -> Result<Self, String> {
        let enabled = config
            .as_ref()
            .map(|configured| configured.enabled)
            .unwrap_or(false);
        let features = config
            .as_ref()
            .map(|configured| configured.features.clone())
            .unwrap_or(OfficialUsersFeaturesConfig {
                login: true,
                register: true,
                verify_email: false,
                reset_password: false,
            });

        let assembly = OfficialUserAssembly::from_config(config.as_ref(), seed_users.len());
        let provider: Arc<dyn UserModuleProvider> = match config
            .as_ref()
            .and_then(|configured| configured.session.transport.as_deref())
        {
            Some("launcher-http") => Arc::new(HttpUserModuleProvider::from_config(
                config
                    .as_ref()
                    .and_then(|configured| configured.session.launcher_url.clone())
                    .ok_or_else(|| {
                        "modules.official.users.session.launcherUrl is required when session.transport=`launcher-http`"
                            .to_string()
                    })?,
                features.clone(),
            )),
            _ => Arc::new(InMemoryUserModuleProvider::new(features, seed_users)),
        };
        Ok(Self {
            provider,
            enabled,
            assembly,
        })
    }

    pub fn login(&self, app: &str, email: &str, password: &str) -> Result<Value, AppError> {
        self.login_session(app, email, password)
            .map(|session| session.to_public_json())
    }

    pub fn register(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<Value, AppError> {
        self.register_session(app, email, password, display_name)
            .map(|session| session.to_public_json())
    }

    pub fn me(&self, bearer_token: &str) -> Result<Value, AppError> {
        self.provider
            .session_from_bearer(bearer_token)
            .map(|session| session.to_public_json())
    }

    pub fn logout(&self, bearer_token: &str) -> Result<Value, AppError> {
        self.provider.logout_session(bearer_token)?;
        Ok(json!({ "revoked": true }))
    }

    pub(crate) fn login_session(
        &self,
        app: &str,
        email: &str,
        password: &str,
    ) -> Result<AuthSession, AppError> {
        self.provider.login(app, email, password)
    }

    pub(crate) fn register_session(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<AuthSession, AppError> {
        self.provider.register(app, email, password, display_name)
    }

    pub(crate) fn session_from_bearer(&self, bearer_token: &str) -> Result<AuthSession, AppError> {
        self.provider.session_from_bearer(bearer_token)
    }

    pub(crate) fn session_from_cookie(
        &self,
        app: &str,
        cookie_token: &str,
    ) -> Result<AuthSession, AppError> {
        self.provider.session_from_cookie(app, cookie_token)
    }

    pub(crate) fn ensure_anonymous_session(&self, app: &str) -> Result<AuthSession, AppError> {
        self.provider.ensure_anonymous_session(app)
    }

    pub(crate) fn me_for_request(
        &self,
        app: Option<&str>,
        bearer_token: Option<&str>,
        cookie_token: Option<&str>,
    ) -> Result<AuthSession, AppError> {
        if let Some(token) = bearer_token {
            return self.provider.session_from_bearer(token);
        }

        let app = app.ok_or_else(|| AppError::unauthorized("missing app for session cookie"))?;
        let cookie = cookie_token
            .ok_or_else(|| AppError::unauthorized("missing bearer token or session cookie"))?;

        self.provider.session_from_cookie(app, cookie)
    }

    pub(crate) fn logout_for_request(
        &self,
        app: Option<&str>,
        bearer_token: Option<&str>,
        cookie_token: Option<&str>,
    ) -> Result<Option<SessionCookie>, AppError> {
        if let Some(token) = bearer_token {
            let session = self.provider.session_from_bearer(token)?;
            self.provider.logout_session(token)?;
            return Ok(Some(SessionCookie::clearing(&session.user.app)));
        }

        let app = app.ok_or_else(|| AppError::unauthorized("missing app for session cookie"))?;
        let cookie = cookie_token
            .ok_or_else(|| AppError::unauthorized("missing bearer token or session cookie"))?;
        let session = self.provider.session_from_cookie(app, cookie)?;
        self.provider.logout_session(&session.token)?;
        Ok(Some(SessionCookie::clearing(app)))
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn assembly(&self) -> &OfficialUserAssembly {
        &self.assembly
    }
}

pub trait UserModuleProvider: Send + Sync {
    fn login(&self, app: &str, email: &str, password: &str) -> Result<AuthSession, AppError>;
    fn register(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<AuthSession, AppError>;
    fn session_from_bearer(&self, bearer_token: &str) -> Result<AuthSession, AppError>;
    fn session_from_cookie(&self, app: &str, cookie_token: &str) -> Result<AuthSession, AppError>;
    fn ensure_anonymous_session(&self, app: &str) -> Result<AuthSession, AppError>;
    fn logout_session(&self, session_token: &str) -> Result<(), AppError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialUserAssembly {
    pub session_transport: Option<String>,
    pub jwt_algorithm: Option<String>,
    pub password_algorithm: Option<String>,
    pub storage_engine: Option<String>,
    pub seed_user_count: usize,
}

impl OfficialUserAssembly {
    fn from_config(config: Option<&OfficialUsersModuleConfig>, seed_user_count: usize) -> Self {
        Self {
            session_transport: config
                .and_then(|configured| normalized_option(configured.session.transport.as_deref())),
            jwt_algorithm: config
                .and_then(|configured| normalized_option(configured.jwt.algorithm.as_deref())),
            password_algorithm: config
                .and_then(|configured| normalized_option(configured.password.algorithm.as_deref())),
            storage_engine: config
                .and_then(|configured| normalized_option(configured.storage.engine.as_deref())),
            seed_user_count,
        }
    }
}

struct InMemoryUserModuleProvider {
    features: OfficialUsersFeaturesConfig,
    auth: InMemoryAuthService,
}

struct HttpUserModuleProvider {
    base_url: String,
    features: OfficialUsersFeaturesConfig,
    token_apps: Mutex<HashMap<String, String>>,
}

impl HttpUserModuleProvider {
    fn from_config(base_url: String, features: OfficialUsersFeaturesConfig) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            features,
            token_apps: Mutex::new(HashMap::new()),
        }
    }

    fn ensure_login_enabled(&self) -> Result<(), AppError> {
        if self.features.login {
            return Ok(());
        }

        Err(AppError::not_found(
            "modules.official.users.features.login is disabled",
        ))
    }

    fn ensure_register_enabled(&self) -> Result<(), AppError> {
        if self.features.register {
            return Ok(());
        }

        Err(AppError::not_found(
            "modules.official.users.features.register is disabled",
        ))
    }

    fn invoke(&self, app: &str, method: &str, payload: Value) -> Result<Value, AppError> {
        let url = format!(
            "{}/invoke/{}",
            self.base_url,
            subject_for_user_method(app, method)
        );
        let agent = default_http_agent().map_err(AppError::internal)?;
        let response = agent
            .post(&url)
            .set("content-type", "application/json")
            .send_json(payload)
            .map_err(|error| {
                AppError::internal(format!("http user launcher request failed: {error}"))
            })?;

        let status = response.status();
        let parsed: Value = response.into_json().map_err(|error| {
            AppError::internal(format!("failed to parse launcher response: {error}"))
        })?;

        if status >= 400 {
            return Err(AppError::internal(format!(
                "http user launcher returned status {status}"
            )));
        }

        match parsed.get("ok").and_then(Value::as_bool) {
            Some(true) => parsed
                .get("data")
                .cloned()
                .ok_or_else(|| AppError::internal("launcher response missing data field")),
            Some(false) => Err(AppError::unauthorized(
                parsed
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("launcher user call failed"),
            )),
            None => Ok(parsed),
        }
    }

    fn remember_session_app(&self, session: &AuthSession) -> Result<(), AppError> {
        self.token_apps
            .lock()
            .map_err(|_| AppError::internal("launcher-http token app map lock poisoned"))?
            .insert(session.token.clone(), session.user.app.clone());
        Ok(())
    }

    fn app_for_token(&self, token: &str) -> Result<String, AppError> {
        self.token_apps
            .lock()
            .map_err(|_| AppError::internal("launcher-http token app map lock poisoned"))?
            .get(token)
            .cloned()
            .ok_or_else(|| {
                AppError::unauthorized(
                    "session app is unknown for launcher-http token; use an app-scoped me/logout flow first",
                )
            })
    }

    fn app_for_cookie(&self, app: &str) -> String {
        app.trim().to_ascii_lowercase()
    }
}

impl InMemoryUserModuleProvider {
    fn new(features: OfficialUsersFeaturesConfig, seed_users: Vec<InMemoryUserConfig>) -> Self {
        Self {
            features,
            auth: InMemoryAuthService::from_user_configs(seed_users),
        }
    }

    fn ensure_login_enabled(&self) -> Result<(), AppError> {
        if self.features.login {
            return Ok(());
        }

        Err(AppError::not_found(
            "modules.official.users.features.login is disabled",
        ))
    }

    fn ensure_register_enabled(&self) -> Result<(), AppError> {
        if self.features.register {
            return Ok(());
        }

        Err(AppError::not_found(
            "modules.official.users.features.register is disabled",
        ))
    }
}

impl UserModuleProvider for InMemoryUserModuleProvider {
    fn login(&self, app: &str, email: &str, password: &str) -> Result<AuthSession, AppError> {
        self.ensure_login_enabled()?;
        self.auth.login(app, email, password)
    }

    fn register(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<AuthSession, AppError> {
        self.ensure_register_enabled()?;
        self.auth.register(app, email, password, display_name)
    }

    fn session_from_bearer(&self, bearer_token: &str) -> Result<AuthSession, AppError> {
        self.auth.session_from_bearer(bearer_token)
    }

    fn session_from_cookie(&self, app: &str, cookie_token: &str) -> Result<AuthSession, AppError> {
        self.auth.session_from_cookie(app, cookie_token)
    }

    fn ensure_anonymous_session(&self, app: &str) -> Result<AuthSession, AppError> {
        self.auth.ensure_anonymous_session(app)
    }

    fn logout_session(&self, session_token: &str) -> Result<(), AppError> {
        self.auth.logout(session_token).map(|_| ())
    }
}

impl UserModuleProvider for HttpUserModuleProvider {
    fn login(&self, app: &str, email: &str, password: &str) -> Result<AuthSession, AppError> {
        self.ensure_login_enabled()?;
        let value = self.invoke(
            app,
            "login",
            json!({
                "app": app,
                "module": "user",
                "method": "login",
                "input": {
                    "app": app,
                    "email": email,
                    "password": password
                },
                "db": {}
            }),
        )?;
        let session = AuthSession::from_public_json(&value)?;
        self.remember_session_app(&session)?;
        Ok(session)
    }

    fn register(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<AuthSession, AppError> {
        self.ensure_register_enabled()?;
        let value = self.invoke(
            app,
            "register",
            json!({
                "app": app,
                "module": "user",
                "method": "register",
                "input": {
                    "app": app,
                    "email": email,
                    "password": password,
                    "displayName": display_name
                },
                "db": {}
            }),
        )?;
        let session = AuthSession::from_public_json(&value)?;
        self.remember_session_app(&session)?;
        Ok(session)
    }

    fn session_from_bearer(&self, bearer_token: &str) -> Result<AuthSession, AppError> {
        let token = sanitize_http_session_token(bearer_token)?;
        let app = self.app_for_token(&token)?;
        let value = self.invoke(
            &app,
            "me",
            json!({
                "app": app,
                "module": "user",
                "method": "me",
                "input": {
                    "app": app
                },
                "db": {
                    "user": {
                        "me": {
                            "token": bearer_token
                        }
                    }
                }
            }),
        )?;
        let session = AuthSession::from_public_json(&value)?;
        self.remember_session_app(&session)?;
        Ok(session)
    }

    fn session_from_cookie(&self, app: &str, cookie_token: &str) -> Result<AuthSession, AppError> {
        let normalized_app = self.app_for_cookie(app);
        let value = self.invoke(
            &normalized_app,
            "me",
            json!({
                "app": normalized_app,
                "module": "user",
                "method": "me",
                "input": {
                    "app": normalized_app
                },
                "db": {
                    "user": {
                        "cookie": {
                            "token": cookie_token
                        }
                    }
                }
            }),
        )?;
        let session = AuthSession::from_public_json(&value)?;
        self.remember_session_app(&session)?;
        Ok(session)
    }

    fn ensure_anonymous_session(&self, app: &str) -> Result<AuthSession, AppError> {
        let normalized_app = self.app_for_cookie(app);
        let value = self.invoke(
            &normalized_app,
            "ensureAnonymousSession",
            json!({
                "app": normalized_app,
                "module": "user",
                "method": "ensureAnonymousSession",
                "input": {
                    "app": normalized_app
                },
                "db": {}
            }),
        )?;
        let session = AuthSession::from_public_json(&value)?;
        self.remember_session_app(&session)?;
        Ok(session)
    }

    fn logout_session(&self, session_token: &str) -> Result<(), AppError> {
        let token = sanitize_http_session_token(session_token)?;
        let app = self.app_for_token(&token)?;
        self.invoke(
            &app,
            "logout",
            json!({
                "app": app,
                "module": "user",
                "method": "logout",
                "input": {
                    "app": app
                },
                "db": {
                    "user": {
                        "logout": {
                            "token": session_token
                        }
                    }
                }
            }),
        )?;
        self.token_apps
            .lock()
            .map_err(|_| AppError::internal("launcher-http token app map lock poisoned"))?
            .remove(&token);
        Ok(())
    }
}

fn normalized_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn sanitize_http_session_token(value: &str) -> Result<String, AppError> {
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

fn subject_for_user_method(app: &str, method: &str) -> String {
    format!(
        "bladb.app.{}.module.user.{method}",
        app.trim().to_ascii_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::config::{
        OfficialUsersJwtConfig, OfficialUsersMailerConfig, OfficialUsersModuleConfig,
        OfficialUsersPasswordConfig, OfficialUsersSessionConfig, OfficialUsersStorageConfig,
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        thread,
    };

    fn seed_user() -> InMemoryUserConfig {
        InMemoryUserConfig {
            app: "flash-sale".into(),
            uid: "u_2001".into(),
            tenant_id: "tenant_flashsale".into(),
            email: "buyer@flash-sale.demo".into(),
            password: "demo123".into(),
            display_name: "Flash Buyer".into(),
            roles: vec!["buyer".into()],
        }
    }

    fn official_config() -> OfficialUsersModuleConfig {
        OfficialUsersModuleConfig {
            enabled: true,
            session: OfficialUsersSessionConfig {
                transport: Some("gateway-auth".into()),
                launcher_url: None,
            },
            jwt: OfficialUsersJwtConfig {
                algorithm: Some("HS256".into()),
                secret: Some("secret".into()),
                public_key_file: None,
                private_key_file: None,
            },
            password: OfficialUsersPasswordConfig {
                algorithm: Some("argon2id".into()),
            },
            storage: OfficialUsersStorageConfig {
                engine: Some("mysql".into()),
                mysql: None,
                mongodb: None,
            },
            mailer: OfficialUsersMailerConfig::default(),
            features: OfficialUsersFeaturesConfig {
                login: true,
                register: true,
                verify_email: false,
                reset_password: false,
            },
        }
    }

    #[test]
    fn official_user_module_exposes_provider_assembly_details() {
        let module = OfficialUserModule::from_config(Some(official_config()), vec![seed_user()])
            .expect("build official user module");

        assert!(module.is_enabled());
        assert_eq!(
            module.assembly(),
            &OfficialUserAssembly {
                session_transport: Some("gateway-auth".into()),
                jwt_algorithm: Some("HS256".into()),
                password_algorithm: Some("argon2id".into()),
                storage_engine: Some("mysql".into()),
                seed_user_count: 1,
            }
        );
    }

    #[test]
    fn disabled_config_still_builds_seeded_local_auth_module() {
        let module = OfficialUserModule::from_config(None, vec![seed_user()])
            .expect("build seeded local auth module");

        assert!(!module.is_enabled());
        assert_eq!(module.assembly().seed_user_count, 1);
        assert_eq!(module.assembly().session_transport, None);
    }

    #[test]
    fn cookie_names_are_scoped_per_app() {
        assert_eq!(
            SessionCookie::cookie_name_for_app("flash-sale"),
            "bladb_sid_flash_sale"
        );
        assert_eq!(
            SessionCookie::cookie_name_for_app("user-module-demo"),
            "bladb_sid_user_module_demo"
        );
    }

    #[test]
    fn launcher_http_subjects_are_app_scoped() {
        assert_eq!(
            subject_for_user_method("blog", "login"),
            "bladb.app.blog.module.user.login"
        );
        assert_eq!(
            subject_for_user_method("user-module-demo", "me"),
            "bladb.app.user-module-demo.module.user.me"
        );
    }

    #[test]
    fn launcher_http_provider_supports_blog_login_cookie_me_and_anonymous_session() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake launcher");
        let address = listener.local_addr().expect("fake launcher addr");
        let (request_tx, request_rx) = mpsc::channel();

        let server = thread::spawn(move || {
            for _ in 0..5 {
                let (mut stream, _) = listener.accept().expect("accept launcher request");
                let request = read_http_request(&mut stream);
                let first_line = request.lines().next().unwrap_or_default().to_string();
                let body = request
                    .split("\r\n\r\n")
                    .nth(1)
                    .or_else(|| request.split("\n\n").nth(1))
                    .unwrap_or("{}")
                    .to_string();
                request_tx
                    .send((first_line.clone(), body.clone()))
                    .expect("capture request");

                let payload: Value = serde_json::from_str(&body).expect("parse launcher body");
                let response_body =
                    if first_line.contains("/invoke/bladb.app.blog.module.user.login") {
                        json!({
                            "ok": true,
                            "data": {
                                "token": "session-blog-1",
                                "sessionKind": "authenticated",
                                "anonymous": false,
                                "user": {
                                    "app": "blog",
                                    "uid": "u_5001",
                                    "tenantId": "tenant_blog",
                                    "email": "editor@blog.demo",
                                    "displayName": "Blog Editor",
                                    "roles": ["editor"],
                                    "anonymous": false
                                }
                            }
                        })
                    } else if first_line.contains("/invoke/bladb.app.blog.module.user.me") {
                        if payload["db"]["user"]["cookie"]["token"] == "anon-blog-1" {
                            json!({
                                "ok": true,
                                "data": {
                                    "token": "anon-blog-1",
                                    "sessionKind": "anonymous",
                                    "anonymous": true,
                                    "user": {
                                        "app": "blog",
                                        "uid": "anon_blog_6001",
                                        "tenantId": "tenant_blog",
                                        "email": "anon+anon_blog_6001@blog.local",
                                        "displayName": "Anonymous blog 6001",
                                        "roles": ["editor"],
                                        "anonymous": true
                                    }
                                }
                            })
                        } else {
                            json!({
                                "ok": true,
                                "data": {
                                    "token": "session-blog-1",
                                    "sessionKind": "authenticated",
                                    "anonymous": false,
                                    "user": {
                                        "app": "blog",
                                        "uid": "u_5001",
                                        "tenantId": "tenant_blog",
                                        "email": "editor@blog.demo",
                                        "displayName": "Blog Editor",
                                        "roles": ["editor"],
                                        "anonymous": false
                                    }
                                }
                            })
                        }
                    } else if first_line
                        .contains("/invoke/bladb.app.blog.module.user.ensureAnonymousSession")
                    {
                        json!({
                            "ok": true,
                            "data": {
                                "token": "anon-blog-1",
                                "sessionKind": "anonymous",
                                "anonymous": true,
                                "user": {
                                    "app": "blog",
                                    "uid": "anon_blog_6001",
                                    "tenantId": "tenant_blog",
                                    "email": "anon+anon_blog_6001@blog.local",
                                    "displayName": "Anonymous blog 6001",
                                    "roles": ["editor"],
                                    "anonymous": true
                                }
                            }
                        })
                    } else if first_line.contains("/invoke/bladb.app.blog.module.user.logout") {
                        json!({ "ok": true, "data": { "revoked": true } })
                    } else {
                        json!({
                            "ok": false,
                            "error": {
                                "message": "unexpected subject"
                            }
                        })
                    };

                let body = serde_json::to_string(&response_body).expect("serialize response");
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write launcher response");
            }
        });

        let provider = HttpUserModuleProvider::from_config(
            format!("http://{}", address),
            OfficialUsersFeaturesConfig {
                login: true,
                register: true,
                verify_email: false,
                reset_password: false,
            },
        );

        let login = provider
            .login("blog", "editor@blog.demo", "demo123")
            .expect("blog login through launcher");
        assert_eq!(login.user.app, "blog");
        assert_eq!(login.token, "session-blog-1");

        let me = provider
            .session_from_bearer("Bearer session-blog-1")
            .expect("bearer me through launcher");
        assert_eq!(me.user.email, "editor@blog.demo");

        let anonymous = provider
            .ensure_anonymous_session("blog")
            .expect("anonymous session through launcher");
        assert_eq!(anonymous.user.app, "blog");
        assert!(anonymous.user.anonymous);

        let cookie_me = provider
            .session_from_cookie("blog", "anon-blog-1")
            .expect("cookie me through launcher");
        assert_eq!(cookie_me.user.uid, "anon_blog_6001");
        assert!(cookie_me.user.anonymous);

        provider
            .logout_session("Bearer session-blog-1")
            .expect("launcher logout");

        let requests = request_rx.try_iter().collect::<Vec<_>>();
        assert!(requests.iter().any(|(line, body)| {
            line.contains("/invoke/bladb.app.blog.module.user.login")
                && body.contains("\"email\":\"editor@blog.demo\"")
        }));
        assert!(requests.iter().any(|(line, body)| {
            line.contains("/invoke/bladb.app.blog.module.user.me")
                && body.contains("\"token\":\"Bearer session-blog-1\"")
        }));
        assert!(requests.iter().any(|(line, body)| {
            line.contains("/invoke/bladb.app.blog.module.user.ensureAnonymousSession")
                && body.contains("\"app\":\"blog\"")
        }));
        assert!(requests.iter().any(|(line, body)| {
            line.contains("/invoke/bladb.app.blog.module.user.me")
                && body.contains("\"cookie\":{\"token\":\"anon-blog-1\"}")
        }));
        assert!(requests.iter().any(|(line, body)| {
            line.contains("/invoke/bladb.app.blog.module.user.logout")
                && body.contains("\"token\":\"Bearer session-blog-1\"")
        }));

        server.join().expect("join fake launcher");
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> String {
        let mut request_bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        let header_end;
        let content_length;

        loop {
            let bytes_read = stream.read(&mut buffer).expect("read launcher request");
            if bytes_read == 0 {
                panic!("launcher request closed before headers were fully read");
            }
            request_bytes.extend_from_slice(&buffer[..bytes_read]);
            if let Some(index) = request_bytes
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
            {
                header_end = index;
                let headers = String::from_utf8_lossy(&request_bytes[..index]);
                content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        if name.eq_ignore_ascii_case("content-length") {
                            value.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                break;
            }
        }

        let body_start = header_end + 4;
        while request_bytes.len() < body_start + content_length {
            let bytes_read = stream
                .read(&mut buffer)
                .expect("read launcher request body");
            if bytes_read == 0 {
                panic!("launcher request closed before body was fully read");
            }
            request_bytes.extend_from_slice(&buffer[..bytes_read]);
        }

        String::from_utf8_lossy(&request_bytes).to_string()
    }
}
