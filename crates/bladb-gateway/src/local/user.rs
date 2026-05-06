use super::{
    auth::{AuthSession, InMemoryAuthService, InMemoryUserConfig},
    config::{OfficialUsersFeaturesConfig, OfficialUsersModuleConfig},
    AppError,
};
use serde_json::{json, Value};
use std::sync::Arc;

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
        let provider = InMemoryUserModuleProvider::new(features, seed_users);
        Ok(Self {
            provider: Arc::new(provider),
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
        self.provider
            .register(app, email, password, display_name)
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

fn normalized_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::config::{
        OfficialUsersJwtConfig, OfficialUsersMailerConfig, OfficialUsersModuleConfig,
        OfficialUsersPasswordConfig, OfficialUsersSessionConfig, OfficialUsersStorageConfig,
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
}
