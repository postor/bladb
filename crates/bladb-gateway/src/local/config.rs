use super::{auth::InMemoryUserConfig, flash_sale::FlashSaleModuleConfig, iot::IotModuleConfig};
use crate::AuthContext;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGatewayModulesConfig {
    pub flash_sale: Option<FlashSaleModuleConfig>,
    pub iot: Option<IotModuleConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalGatewayFileConfig {
    pub runtimes: Vec<GatewayRuntimeFileConfig>,
    #[serde(default)]
    pub auth: GatewayAuthFileConfig,
    #[serde(default)]
    pub modules: LocalGatewayModulesConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GatewayAuthFileConfig {
    #[serde(default)]
    pub users: Vec<InMemoryUserConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GatewayRuntimeFileConfig {
    pub name: String,
    pub policy: String,
    pub topology: String,
    #[serde(default)]
    pub default_auth: AuthContext,
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
        })
    }
}

fn read_relative_file(base_dir: &Path, target: &str, label: &str) -> Result<String, String> {
    let path = Path::new(target);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    };

    fs::read_to_string(&resolved).map_err(|error| {
        format!(
            "failed to read {label} file {}: {error}",
            resolved.display()
        )
    })
}
