use bladb_core::cluster::{
    parse_topology_manifest, DeploymentConfig, DiscoveryConfig, ModuleCategory,
    TopologyManifestError, TransportConfig, TransportProtocol,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NatsConnectionConfig {
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportLoopConfig {
    #[serde(default = "default_max_batch")]
    pub max_batch: usize,
    #[serde(default = "default_idle_sleep_ms")]
    pub idle_sleep_ms: u64,
}

impl Default for TransportLoopConfig {
    fn default() -> Self {
        Self {
            max_batch: default_max_batch(),
            idle_sleep_ms: default_idle_sleep_ms(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServeConfig {
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdapterBindingConfig {
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleRuntimeConfigFile {
    pub cluster: String,
    pub topology: String,
    #[serde(default)]
    pub nats: NatsConnectionConfig,
    #[serde(default)]
    pub transport_loop: TransportLoopConfig,
    #[serde(default)]
    pub serve: ServeConfig,
    #[serde(default)]
    pub adapter: AdapterBindingConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRuntimeConfig {
    pub cluster: String,
    pub topology_path: PathBuf,
    pub nats: NatsConnectionConfig,
    pub transport_loop: TransportLoopConfig,
    pub serve: ServeConfig,
    pub adapter: AdapterBindingConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleRuntimePlan {
    pub cluster: String,
    pub category: ModuleCategory,
    pub runtime: String,
    pub discovery: DiscoveryConfig,
    pub transport: TransportConfig,
    pub deployment: DeploymentConfig,
    pub nats: NatsConnectionConfig,
    pub transport_loop: TransportLoopConfig,
    pub serve: ServeConfig,
    pub adapter: AdapterBindingConfig,
}

impl ModuleRuntimeConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ModuleRuntimePlanError> {
        let path = path.as_ref();
        let contents =
            fs::read_to_string(path).map_err(|error| ModuleRuntimePlanError::ConfigRead {
                path: path.display().to_string(),
                reason: error.to_string(),
            })?;
        let parsed = match path.extension().and_then(|extension| extension.to_str()) {
            Some("json") => serde_json::from_str::<ModuleRuntimeConfigFile>(&contents)
                .map_err(|error| ModuleRuntimePlanError::ConfigParse(error.to_string()))?,
            _ => serde_yaml::from_str::<ModuleRuntimeConfigFile>(&contents)
                .map_err(|error| ModuleRuntimePlanError::ConfigParse(error.to_string()))?,
        };
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        Ok(Self::from_file_config(parsed, base_dir))
    }

    pub fn from_file_config(file: ModuleRuntimeConfigFile, base_dir: &Path) -> Self {
        let topology_path = resolve_relative(base_dir, &file.topology);
        Self {
            cluster: file.cluster,
            topology_path,
            nats: file.nats,
            transport_loop: file.transport_loop,
            serve: file.serve,
            adapter: file.adapter,
        }
    }

    pub fn from_env() -> Result<Self, ModuleRuntimePlanError> {
        Self::from_env_map(env::vars())
    }

    pub fn from_env_map<I, K, V>(vars: I) -> Result<Self, ModuleRuntimePlanError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let vars = vars
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<BTreeMap<String, String>>();
        let cluster = required_env(&vars, "BLADB_CLUSTER_NAME")?;
        let topology = required_env(&vars, "BLADB_TOPOLOGY_PATH")?;
        Ok(Self {
            cluster,
            topology_path: PathBuf::from(topology),
            nats: NatsConnectionConfig {
                url: vars.get("BLADB_NATS_URL").cloned(),
            },
            transport_loop: TransportLoopConfig {
                max_batch: vars
                    .get("BLADB_LOOP_MAX_BATCH")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or_else(default_max_batch),
                idle_sleep_ms: vars
                    .get("BLADB_LOOP_IDLE_SLEEP_MS")
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or_else(default_idle_sleep_ms),
            },
            serve: ServeConfig {
                bind_addr: vars
                    .get("BLADB_BIND_ADDR")
                    .cloned()
                    .unwrap_or_else(default_bind_addr),
            },
            adapter: AdapterBindingConfig {
                endpoint: vars.get("BLADB_ADAPTER_ENDPOINT").cloned(),
                database: vars.get("BLADB_ADAPTER_DATABASE").cloned(),
                runtime: vars.get("BLADB_ADAPTER_RUNTIME").cloned(),
                options: vars
                    .iter()
                    .filter_map(|(key, value)| {
                        key.strip_prefix("BLADB_ADAPTER_OPTION_")
                            .map(|stripped| (stripped.to_ascii_lowercase(), value.clone()))
                    })
                    .collect(),
            },
        })
    }

    pub fn load_topology_yaml(&self) -> Result<String, ModuleRuntimePlanError> {
        fs::read_to_string(&self.topology_path).map_err(|error| {
            ModuleRuntimePlanError::ConfigRead {
                path: self.topology_path.display().to_string(),
                reason: error.to_string(),
            }
        })
    }

    pub fn build_plan(&self) -> Result<ModuleRuntimePlan, ModuleRuntimePlanError> {
        let topology_yaml = self.load_topology_yaml()?;
        let topology = parse_topology_manifest(&topology_yaml)?;
        let cluster = topology
            .module_clusters
            .into_iter()
            .find(|cluster| cluster.name == self.cluster)
            .ok_or_else(|| ModuleRuntimePlanError::UnknownCluster(self.cluster.clone()))?;

        if cluster.category == ModuleCategory::Worker {
            return Err(ModuleRuntimePlanError::UnsupportedCategory {
                cluster: cluster.name,
                category: ModuleCategory::Worker,
            });
        }

        if let Some(runtime) = &self.adapter.runtime {
            if runtime != &cluster.runtime {
                return Err(ModuleRuntimePlanError::AdapterRuntimeMismatch {
                    expected: cluster.runtime.clone(),
                    actual: runtime.clone(),
                });
            }
        }

        if matches!(
            cluster.transport.protocol,
            TransportProtocol::NatsService | TransportProtocol::JetStream
        ) && self.nats.url.is_none()
        {
            return Err(ModuleRuntimePlanError::MissingTransportConfig {
                field: "nats.url".into(),
                cluster: cluster.name,
            });
        }

        Ok(ModuleRuntimePlan {
            cluster: cluster.name,
            category: cluster.category,
            runtime: cluster.runtime,
            discovery: cluster.discovery,
            transport: cluster.transport,
            deployment: cluster.deployment,
            nats: self.nats.clone(),
            transport_loop: self.transport_loop.clone(),
            serve: self.serve.clone(),
            adapter: self.adapter.clone(),
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModuleRuntimePlanError {
    #[error("failed to read config file `{path}`: {reason}")]
    ConfigRead { path: String, reason: String },
    #[error("failed to parse runtime config: {0}")]
    ConfigParse(String),
    #[error("missing required environment variable `{0}`")]
    MissingEnv(&'static str),
    #[error(transparent)]
    Topology(#[from] TopologyManifestError),
    #[error("cluster `{0}` was not found in topology manifest")]
    UnknownCluster(String),
    #[error("cluster `{cluster}` uses unsupported category `{category:?}` for module runtime")]
    UnsupportedCategory {
        cluster: String,
        category: ModuleCategory,
    },
    #[error("adapter runtime mismatch: expected `{expected}` but got `{actual}`")]
    AdapterRuntimeMismatch { expected: String, actual: String },
    #[error("missing transport config `{field}` for cluster `{cluster}`")]
    MissingTransportConfig { field: String, cluster: String },
}

fn default_bind_addr() -> String {
    "0.0.0.0:9000".into()
}

fn default_max_batch() -> usize {
    64
}

fn default_idle_sleep_ms() -> u64 {
    25
}

fn resolve_relative(base_dir: &Path, target: &str) -> PathBuf {
    let path = Path::new(target);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn required_env(
    vars: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<String, ModuleRuntimePlanError> {
    vars.get(key)
        .cloned()
        .ok_or(ModuleRuntimePlanError::MissingEnv(key))
}

#[cfg(test)]
mod tests {
    use super::{
        AdapterBindingConfig, ModuleRuntimeConfig, ModuleRuntimeConfigFile, ModuleRuntimePlanError,
        TransportLoopConfig,
    };
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builds_module_runtime_plan_from_real_flash_sale_topology() {
        let config = ModuleRuntimeConfig {
            cluster: "flashsale.orders-sql".into(),
            topology_path: workspace_root()
                .join("apps/examples/flash-sale/topology/flash-sale.topology.yaml"),
            nats: super::NatsConnectionConfig {
                url: Some("nats://nats.bladb.svc.cluster.local:4222".into()),
            },
            transport_loop: TransportLoopConfig {
                max_batch: 32,
                idle_sleep_ms: 10,
            },
            serve: super::ServeConfig {
                bind_addr: "0.0.0.0:9000".into(),
            },
            adapter: AdapterBindingConfig {
                endpoint: Some("postgres://orders@db/orders".into()),
                database: Some("orders".into()),
                runtime: Some("sql".into()),
                options: Default::default(),
            },
        };

        let plan = config.build_plan().expect("build module runtime plan");
        assert_eq!(plan.cluster, "flashsale.orders-sql");
        assert_eq!(plan.runtime, "sql");
        assert_eq!(plan.discovery.service, "bladb-module-orders");
        assert_eq!(
            plan.transport.subject.as_deref(),
            Some("rpc.flashsale.orders")
        );
        assert_eq!(
            plan.deployment
                .autoscale
                .as_ref()
                .map(|value| value.max_replicas),
            Some(8)
        );
        assert_eq!(plan.transport_loop.max_batch, 32);
        assert_eq!(plan.transport_loop.idle_sleep_ms, 10);
    }

    #[test]
    fn rejects_worker_cluster_for_module_runtime() {
        let config = ModuleRuntimeConfig {
            cluster: "flashsale.workflow-workers".into(),
            topology_path: workspace_root()
                .join("apps/examples/flash-sale/topology/flash-sale.topology.yaml"),
            nats: super::NatsConnectionConfig::default(),
            transport_loop: TransportLoopConfig::default(),
            serve: super::ServeConfig::default(),
            adapter: AdapterBindingConfig::default(),
        };

        let error = config
            .build_plan()
            .expect_err("expected unsupported category");
        assert_eq!(
            error,
            ModuleRuntimePlanError::UnsupportedCategory {
                cluster: "flashsale.workflow-workers".into(),
                category: bladb_core::cluster::ModuleCategory::Worker,
            }
        );
    }

    #[test]
    fn loads_runtime_config_from_env_map() {
        let config = ModuleRuntimeConfig::from_env_map([
            ("BLADB_CLUSTER_NAME", "iot.commands-mqtt"),
            (
                "BLADB_TOPOLOGY_PATH",
                "D:/study/bladb/apps/examples/iot-realtime/topology/iot-realtime.topology.yaml",
            ),
            ("BLADB_NATS_URL", "nats://nats.bladb.svc.cluster.local:4222"),
            ("BLADB_LOOP_MAX_BATCH", "128"),
            ("BLADB_LOOP_IDLE_SLEEP_MS", "50"),
            ("BLADB_BIND_ADDR", "0.0.0.0:9100"),
            ("BLADB_ADAPTER_RUNTIME", "mqtt"),
            ("BLADB_ADAPTER_ENDPOINT", "mqtt://broker:1883"),
            ("BLADB_ADAPTER_OPTION_CLIENT_ID", "iot-command-module"),
        ])
        .expect("config from env map");

        assert_eq!(config.cluster, "iot.commands-mqtt");
        assert_eq!(
            config.topology_path,
            PathBuf::from(
                "D:/study/bladb/apps/examples/iot-realtime/topology/iot-realtime.topology.yaml"
            )
        );
        assert_eq!(config.serve.bind_addr, "0.0.0.0:9100");
        assert_eq!(config.transport_loop.max_batch, 128);
        assert_eq!(config.transport_loop.idle_sleep_ms, 50);
        assert_eq!(config.adapter.runtime.as_deref(), Some("mqtt"));
        assert_eq!(
            config.adapter.options.get("client_id").map(String::as_str),
            Some("iot-command-module")
        );
    }

    #[test]
    fn resolves_relative_topology_path_from_file_config() {
        let file = ModuleRuntimeConfigFile {
            cluster: "flashsale.stock-redis".into(),
            topology: "../../apps/examples/flash-sale/topology/flash-sale.topology.yaml".into(),
            nats: super::NatsConnectionConfig::default(),
            transport_loop: TransportLoopConfig::default(),
            serve: super::ServeConfig::default(),
            adapter: AdapterBindingConfig::default(),
        };
        let base_dir = Path::new("D:/study/bladb/crates/bladb-module-runtime/examples");

        let config = ModuleRuntimeConfig::from_file_config(file, base_dir);
        assert_eq!(
            config.topology_path,
            PathBuf::from(
                "D:/study/bladb/crates/bladb-module-runtime/examples/../../apps/examples/flash-sale/topology/flash-sale.topology.yaml"
            )
        );
    }

    #[test]
    fn loads_real_example_runtime_config_file() {
        let path =
            workspace_root().join("apps/examples/flash-sale/runtime/flashsale.orders.runtime.yaml");
        let config = ModuleRuntimeConfig::from_path(&path).expect("load module runtime config");

        assert_eq!(config.cluster, "flashsale.orders-sql");
        assert_eq!(config.adapter.runtime.as_deref(), Some("sql"));
        assert_eq!(config.adapter.database.as_deref(), Some("orders"));
        assert_eq!(config.transport_loop.max_batch, 64);
        assert_eq!(config.transport_loop.idle_sleep_ms, 25);
        assert_eq!(
            config.topology_path,
            workspace_root().join("apps/examples/flash-sale/topology/flash-sale.topology.yaml")
        );
    }

    #[test]
    fn nats_service_cluster_requires_nats_url() {
        let config = ModuleRuntimeConfig {
            cluster: "flashsale.orders-sql".into(),
            topology_path: workspace_root()
                .join("apps/examples/flash-sale/topology/flash-sale.topology.yaml"),
            nats: super::NatsConnectionConfig::default(),
            transport_loop: TransportLoopConfig::default(),
            serve: super::ServeConfig::default(),
            adapter: AdapterBindingConfig {
                endpoint: Some("postgres://orders@db/orders".into()),
                database: Some("orders".into()),
                runtime: Some("sql".into()),
                options: Default::default(),
            },
        };

        let error = config.build_plan().expect_err("expected missing nats url");
        assert_eq!(
            error,
            ModuleRuntimePlanError::MissingTransportConfig {
                field: "nats.url".into(),
                cluster: "flashsale.orders-sql".into(),
            }
        );
    }
}
