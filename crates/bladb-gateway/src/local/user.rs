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
        self.provider
            .login(app, email, password)
            .map(|session| session.to_public_json())
    }

    pub fn register(
        &self,
        app: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<Value, AppError> {
        self.provider
            .register(app, email, password, display_name)
            .map(|session| session.to_public_json())
    }

    pub fn me(&self, bearer_token: &str) -> Result<Value, AppError> {
        self.provider
            .me(bearer_token)
            .map(|session| session.to_public_json())
    }

    pub fn logout(&self, bearer_token: &str) -> Result<Value, AppError> {
        self.provider.logout(bearer_token)?;
        Ok(json!({ "revoked": true }))
    }

    pub(crate) fn session_from_bearer(&self, bearer_token: &str) -> Result<AuthSession, AppError> {
        self.provider.session_from_bearer(bearer_token)
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
    fn me(&self, bearer_token: &str) -> Result<AuthSession, AppError>;
    fn logout(&self, bearer_token: &str) -> Result<(), AppError>;
    fn session_from_bearer(&self, bearer_token: &str) -> Result<AuthSession, AppError>;
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

    fn me(&self, bearer_token: &str) -> Result<AuthSession, AppError> {
        self.auth.session_from_bearer(bearer_token)
    }

    fn logout(&self, bearer_token: &str) -> Result<(), AppError> {
        self.auth.logout(bearer_token).map(|_| ())
    }

    fn session_from_bearer(&self, bearer_token: &str) -> Result<AuthSession, AppError> {
        self.auth.session_from_bearer(bearer_token)
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
}
