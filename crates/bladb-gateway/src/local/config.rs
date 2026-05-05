use super::{
    auth::InMemoryUserConfig, flash_sale::FlashSaleModuleConfig, iot::IotModuleConfig,
    ros2::Ros2ModuleConfig,
};
use crate::AuthContext;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct GatewayRuntimeConfig {
    pub name: String,
    pub policy_yaml: String,
    pub topology_yaml: String,
    pub default_auth: AuthContext,
}

#[derive(Debug, Clone)]
pub struct LocalGatewayConfig {
    pub runtimes: Vec<GatewayRuntimeConfig>,
    pub auth_users: Vec<InMemoryUserConfig>,
    pub modules: LocalGatewayModulesConfig,
    pub official_users: Option<OfficialUsersModuleConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGatewayModulesConfig {
    pub flash_sale: Option<FlashSaleModuleConfig>,
    pub iot: Option<IotModuleConfig>,
    pub ros2: Option<Ros2ModuleConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialModulesConfig {
    pub users: Option<OfficialUsersModuleConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersModuleConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub session: OfficialUsersSessionConfig,
    #[serde(default)]
    pub jwt: OfficialUsersJwtConfig,
    #[serde(default)]
    pub password: OfficialUsersPasswordConfig,
    #[serde(default)]
    pub storage: OfficialUsersStorageConfig,
    #[serde(default)]
    pub mailer: OfficialUsersMailerConfig,
    #[serde(default)]
    pub features: OfficialUsersFeaturesConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersSessionConfig {
    pub transport: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersJwtConfig {
    pub algorithm: Option<String>,
    pub secret: Option<String>,
    pub public_key_file: Option<String>,
    pub private_key_file: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersPasswordConfig {
    pub algorithm: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersStorageConfig {
    pub engine: Option<String>,
    pub mysql: Option<OfficialUsersMysqlConfig>,
    pub mongodb: Option<OfficialUsersMongodbConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersMysqlConfig {
    pub dsn: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersMongodbConfig {
    pub uri: Option<String>,
    pub database: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersMailerConfig {
    pub provider: Option<String>,
    pub from: Option<String>,
    pub smtp: Option<OfficialUsersSmtpConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersSmtpConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsersFeaturesConfig {
    #[serde(default)]
    pub register: bool,
    #[serde(default)]
    pub login: bool,
    #[serde(default)]
    pub verify_email: bool,
    #[serde(default)]
    pub reset_password: bool,
}

impl OfficialUsersModuleConfig {
    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        self.session.validate()?;
        self.jwt.validate()?;
        self.password.validate()?;
        self.storage.validate()?;
        self.mailer.validate(&self.features)?;

        Ok(())
    }

    fn resolve_runtime_paths(mut self, base_dir: &Path) -> Self {
        self.jwt = self.jwt.resolve_runtime_paths(base_dir);
        self
    }
}

impl OfficialUsersSessionConfig {
    fn validate(&self) -> Result<(), String> {
        required_non_empty(
            self.transport.as_deref(),
            "modules.official.users.session.transport",
        )?;
        Ok(())
    }
}

impl OfficialUsersJwtConfig {
    fn validate(&self) -> Result<(), String> {
        let algorithm = required_non_empty(
            self.algorithm.as_deref(),
            "modules.official.users.jwt.algorithm",
        )?;

        match algorithm {
            "HS256" => {
                required_non_empty(self.secret.as_deref(), "modules.official.users.jwt.secret")?;
                forbid_configured(
                    self.public_key_file.as_deref(),
                    "modules.official.users.jwt.publicKeyFile",
                    "HS256 uses a shared secret instead of key files",
                )?;
                forbid_configured(
                    self.private_key_file.as_deref(),
                    "modules.official.users.jwt.privateKeyFile",
                    "HS256 uses a shared secret instead of key files",
                )?;
            }
            "RS256" | "EdDSA" => {
                forbid_configured(
                    self.secret.as_deref(),
                    "modules.official.users.jwt.secret",
                    "asymmetric signing uses key files instead of a shared secret",
                )?;
                required_non_empty(
                    self.public_key_file.as_deref(),
                    "modules.official.users.jwt.publicKeyFile",
                )?;
                required_non_empty(
                    self.private_key_file.as_deref(),
                    "modules.official.users.jwt.privateKeyFile",
                )?;
            }
            other => {
                return Err(format!(
                    "modules.official.users.jwt.algorithm must be one of `HS256`, `RS256`, or `EdDSA`, got `{other}`"
                ));
            }
        }

        Ok(())
    }

    fn resolve_runtime_paths(mut self, base_dir: &Path) -> Self {
        self.public_key_file = self
            .public_key_file
            .as_deref()
            .map(|path| resolve_relative_path(base_dir, path).display().to_string());
        self.private_key_file = self
            .private_key_file
            .as_deref()
            .map(|path| resolve_relative_path(base_dir, path).display().to_string());
        self
    }
}

impl OfficialUsersPasswordConfig {
    fn validate(&self) -> Result<(), String> {
        let algorithm = required_non_empty(
            self.algorithm.as_deref(),
            "modules.official.users.password.algorithm",
        )?;

        match algorithm {
            "argon2id" | "bcrypt" => Ok(()),
            other => Err(format!(
                "modules.official.users.password.algorithm must be `argon2id` or `bcrypt`, got `{other}`"
            )),
        }
    }
}

impl OfficialUsersStorageConfig {
    fn validate(&self) -> Result<(), String> {
        let engine = required_non_empty(
            self.engine.as_deref(),
            "modules.official.users.storage.engine",
        )?;

        match engine {
            "mysql" => {
                let mysql = self.mysql.as_ref().ok_or_else(|| {
                    "modules.official.users.storage.mysql is required when storage.engine=`mysql`"
                        .to_string()
                })?;
                required_non_empty(
                    mysql.dsn.as_deref(),
                    "modules.official.users.storage.mysql.dsn",
                )?;
                Ok(())
            }
            "mongodb" => {
                let mongodb = self.mongodb.as_ref().ok_or_else(|| {
                    "modules.official.users.storage.mongodb is required when storage.engine=`mongodb`"
                        .to_string()
                })?;
                required_non_empty(
                    mongodb.uri.as_deref(),
                    "modules.official.users.storage.mongodb.uri",
                )?;
                required_non_empty(
                    mongodb.database.as_deref(),
                    "modules.official.users.storage.mongodb.database",
                )?;
                Ok(())
            }
            other => Err(format!(
                "modules.official.users.storage.engine must be `mysql` or `mongodb`, got `{other}`"
            )),
        }
    }
}

impl OfficialUsersMailerConfig {
    fn validate(&self, features: &OfficialUsersFeaturesConfig) -> Result<(), String> {
        let provider = self
            .provider
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let mail_features_enabled = features.verify_email || features.reset_password;

        match provider {
            Some("smtp") => {
                required_non_empty(self.from.as_deref(), "modules.official.users.mailer.from")?;
                let smtp = self.smtp.as_ref().ok_or_else(|| {
                    "modules.official.users.mailer.smtp is required when mailer.provider=`smtp`"
                        .to_string()
                })?;
                required_non_empty(
                    smtp.host.as_deref(),
                    "modules.official.users.mailer.smtp.host",
                )?;
                if smtp.port.is_none() {
                    return Err(
                        "modules.official.users.mailer.smtp.port is required when mailer.provider=`smtp`"
                            .into(),
                    );
                }
                Ok(())
            }
            Some(other) => Err(format!(
                "modules.official.users.mailer.provider must currently be `smtp` when configured, got `{other}`"
            )),
            None if mail_features_enabled => Err(
                "modules.official.users.mailer.provider is required when verifyEmail or resetPassword is enabled"
                    .into(),
            ),
            None => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalGatewayFileConfig {
    pub(crate) runtimes: Vec<GatewayRuntimeFileConfig>,
    #[serde(default)]
    pub(crate) auth: GatewayAuthFileConfig,
    #[serde(default)]
    pub(crate) modules: LocalGatewayModulesConfig,
    #[serde(default)]
    pub(crate) official: OfficialModulesConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GatewayRuntimeFileConfig {
    pub name: String,
    pub policy: String,
    pub topology: String,
    #[serde(default)]
    pub default_auth: AuthContext,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GatewayAuthFileConfig {
    #[serde(default)]
    pub users: Vec<InMemoryUserConfig>,
}

impl LocalGatewayConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|error| {
            format!("failed to read gateway config {}: {error}", path.display())
        })?;
        let parsed = match path.extension().and_then(|extension| extension.to_str()) {
            Some("json") => {
                serde_json::from_str::<LocalGatewayFileConfig>(&contents).map_err(|error| {
                    format!(
                        "failed to parse gateway json config {}: {error}",
                        path.display()
                    )
                })?
            }
            _ => serde_yaml::from_str::<LocalGatewayFileConfig>(&contents).map_err(|error| {
                format!(
                    "failed to parse gateway yaml config {}: {error}",
                    path.display()
                )
            })?,
        };
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        Self::from_file_config(parsed, base_dir)
    }

    pub(crate) fn from_file_config(
        parsed: LocalGatewayFileConfig,
        base_dir: &Path,
    ) -> Result<Self, String> {
        if let Some(official_users) = parsed.official.users.as_ref() {
            official_users.validate()?;
        }

        let official_users = parsed
            .official
            .users
            .map(|config| config.resolve_runtime_paths(base_dir));

        Ok(Self {
            runtimes: parsed
                .runtimes
                .iter()
                .map(|runtime| {
                    Ok(GatewayRuntimeConfig {
                        name: runtime.name.clone(),
                        policy_yaml: read_relative_file(base_dir, &runtime.policy, "policy")?,
                        topology_yaml: read_relative_file(base_dir, &runtime.topology, "topology")?,
                        default_auth: runtime.default_auth.clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            auth_users: parsed.auth.users,
            modules: parsed.modules,
            official_users,
        })
    }
}

fn required_non_empty<'a>(value: Option<&'a str>, field: &str) -> Result<&'a str, String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{field} is required when modules.official.users.enabled=true"))
}

fn forbid_configured(value: Option<&str>, field: &str, reason: &str) -> Result<(), String> {
    if value.is_some_and(|candidate| !candidate.trim().is_empty()) {
        return Err(format!("{field} must not be set: {reason}"));
    }

    Ok(())
}

fn read_relative_file(base_dir: &Path, target: &str, label: &str) -> Result<String, String> {
    let resolved = resolve_relative_path(base_dir, target);

    fs::read_to_string(&resolved).map_err(|error| {
        format!(
            "failed to read {label} file {}: {error}",
            resolved.display()
        )
    })
}

fn resolve_relative_path(base_dir: &Path, target: &str) -> PathBuf {
    let path = Path::new(target);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_users_config() -> OfficialUsersModuleConfig {
        OfficialUsersModuleConfig {
            enabled: true,
            session: OfficialUsersSessionConfig {
                transport: Some("gateway-auth".into()),
            },
            jwt: OfficialUsersJwtConfig {
                algorithm: Some("HS256".into()),
                secret: Some("${BLADB_JWT_SECRET}".into()),
                public_key_file: None,
                private_key_file: None,
            },
            password: OfficialUsersPasswordConfig {
                algorithm: Some("argon2id".into()),
            },
            storage: OfficialUsersStorageConfig {
                engine: Some("mysql".into()),
                mysql: Some(OfficialUsersMysqlConfig {
                    dsn: Some("${BLADB_USERS_MYSQL_DSN}".into()),
                }),
                mongodb: Some(OfficialUsersMongodbConfig {
                    uri: Some("${BLADB_USERS_MONGODB_URI}".into()),
                    database: Some("bladb_users".into()),
                }),
            },
            mailer: OfficialUsersMailerConfig {
                provider: Some("smtp".into()),
                from: Some("no-reply@example.com".into()),
                smtp: Some(OfficialUsersSmtpConfig {
                    host: Some("smtp.example.com".into()),
                    port: Some(587),
                    username: Some("${BLADB_SMTP_USER}".into()),
                    password: Some("${BLADB_SMTP_PASS}".into()),
                }),
            },
            features: OfficialUsersFeaturesConfig {
                register: true,
                login: true,
                verify_email: false,
                reset_password: false,
            },
        }
    }

    #[test]
    fn enabled_official_users_config_accepts_hs256_mysql_and_smtp() {
        valid_users_config().validate().expect("valid config");
    }

    #[test]
    fn enabled_official_users_config_accepts_rs256_key_files() {
        let mut config = valid_users_config();
        config.jwt.algorithm = Some("RS256".into());
        config.jwt.secret = None;
        config.jwt.public_key_file = Some("keys/users.public.pem".into());
        config.jwt.private_key_file = Some("keys/users.private.pem".into());

        config.validate().expect("valid rs256 config");
    }

    #[test]
    fn rejects_hs256_without_secret() {
        let mut config = valid_users_config();
        config.jwt.secret = None;

        let error = config.validate().expect_err("missing jwt secret");
        assert!(error.contains("modules.official.users.jwt.secret"));
    }

    #[test]
    fn rejects_mongodb_without_database() {
        let mut config = valid_users_config();
        config.storage.engine = Some("mongodb".into());
        config.storage.mysql = None;
        config.storage.mongodb = Some(OfficialUsersMongodbConfig {
            uri: Some("${BLADB_USERS_MONGODB_URI}".into()),
            database: None,
        });

        let error = config.validate().expect_err("missing mongodb database");
        assert!(error.contains("modules.official.users.storage.mongodb.database"));
    }

    #[test]
    fn rejects_missing_mailer_when_reset_password_is_enabled() {
        let mut config = valid_users_config();
        config.mailer = OfficialUsersMailerConfig::default();
        config.features.reset_password = true;

        let error = config.validate().expect_err("missing mailer provider");
        assert!(error.contains("modules.official.users.mailer.provider"));
    }

    #[test]
    fn from_file_config_rejects_invalid_official_users_contract() {
        let parsed = LocalGatewayFileConfig {
            runtimes: vec![],
            auth: GatewayAuthFileConfig::default(),
            modules: LocalGatewayModulesConfig::default(),
            official: OfficialModulesConfig {
                users: Some(OfficialUsersModuleConfig {
                    enabled: true,
                    ..OfficialUsersModuleConfig::default()
                }),
            },
        };

        let error = LocalGatewayConfig::from_file_config(parsed, Path::new("."))
            .expect_err("invalid official users config");
        assert!(error.contains("modules.official.users.session.transport"));
    }

    #[test]
    fn from_file_config_resolves_official_user_key_paths_relative_to_config_dir() {
        let mut config = valid_users_config();
        config.jwt.algorithm = Some("RS256".into());
        config.jwt.secret = None;
        config.jwt.public_key_file = Some("keys/users.public.pem".into());
        config.jwt.private_key_file = Some("keys/users.private.pem".into());

        let parsed = LocalGatewayFileConfig {
            runtimes: vec![],
            auth: GatewayAuthFileConfig::default(),
            modules: LocalGatewayModulesConfig::default(),
            official: OfficialModulesConfig {
                users: Some(config),
            },
        };

        let base_dir = Path::new("config-root");
        let config = LocalGatewayConfig::from_file_config(parsed, base_dir)
            .expect("resolve official user key paths");
        let jwt = &config
            .official_users
            .as_ref()
            .expect("official users config")
            .jwt;

        assert_eq!(
            jwt.public_key_file.as_deref(),
            Some(
                base_dir
                    .join("keys/users.public.pem")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert_eq!(
            jwt.private_key_file.as_deref(),
            Some(
                base_dir
                    .join("keys/users.private.pem")
                    .to_string_lossy()
                    .as_ref()
            )
        );
    }
}
