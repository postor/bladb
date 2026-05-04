use bladb_core::worker::{
    parse_worker_manifest, TriggerTransportConfig, TriggerTransportKind, WorkerDefinition,
    WorkerDeploymentConfig, WorkerManifestError, WorkerStep, WorkerTrigger,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerLoopConfig {
    #[serde(default = "default_max_batch")]
    pub max_batch: usize,
    #[serde(default = "default_idle_sleep_ms")]
    pub idle_sleep_ms: u64,
}

impl Default for WorkerLoopConfig {
    fn default() -> Self {
        Self {
            max_batch: default_max_batch(),
            idle_sleep_ms: default_idle_sleep_ms(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerServeConfig {
    #[serde(default = "default_metrics_bind_addr")]
    pub metrics_bind_addr: String,
}

impl Default for WorkerServeConfig {
    fn default() -> Self {
        Self {
            metrics_bind_addr: default_metrics_bind_addr(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerRuntimeConfigFile {
    pub worker: String,
    pub manifest: String,
    #[serde(default)]
    pub nats_url: Option<String>,
    #[serde(default)]
    pub worker_loop: WorkerLoopConfig,
    #[serde(default)]
    pub serve: WorkerServeConfig,
    #[serde(default)]
    pub overrides: WorkerRuntimeOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkerRuntimeOverrides {
    #[serde(default)]
    pub max_concurrency: Option<u16>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRuntimeConfig {
    pub worker: String,
    pub manifest_path: PathBuf,
    pub nats_url: Option<String>,
    pub worker_loop: WorkerLoopConfig,
    pub serve: WorkerServeConfig,
    pub overrides: WorkerRuntimeOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledWorkerPlan {
    pub worker: String,
    pub source: String,
    pub trigger: WorkerTrigger,
    pub identity_mode: String,
    pub identity_as: Option<String>,
    pub idempotency_key_from: String,
    pub retry_max_attempts: u32,
    pub retry_backoff: String,
    pub dead_letter_subject: Option<String>,
    pub timeout_ms: u64,
    pub deployment: WorkerDeploymentConfig,
    pub max_concurrency: u16,
    pub subscription: WorkerSubscriptionPlan,
    pub steps: Vec<WorkerStep>,
    pub nats_url: Option<String>,
    pub worker_loop: WorkerLoopConfig,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerSubscriptionPlan {
    pub transport: TriggerTransportConfig,
    pub subject: Option<String>,
    pub consumer: Option<String>,
    pub stream: Option<String>,
}

impl WorkerRuntimeConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, WorkerRuntimePlanError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|error| WorkerRuntimePlanError::ConfigRead {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        let parsed = match path.extension().and_then(|extension| extension.to_str()) {
            Some("json") => {
                serde_json::from_str::<WorkerRuntimeConfigFile>(&contents).map_err(|error| {
                    WorkerRuntimePlanError::ConfigParse(error.to_string())
                })?
            }
            _ => serde_yaml::from_str::<WorkerRuntimeConfigFile>(&contents).map_err(|error| {
                WorkerRuntimePlanError::ConfigParse(error.to_string())
            })?,
        };
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        Ok(Self::from_file_config(parsed, base_dir))
    }

    pub fn from_file_config(file: WorkerRuntimeConfigFile, base_dir: &Path) -> Self {
        Self {
            worker: file.worker,
            manifest_path: resolve_relative(base_dir, &file.manifest),
            nats_url: file.nats_url,
            worker_loop: file.worker_loop,
            serve: file.serve,
            overrides: file.overrides,
        }
    }

    pub fn from_env() -> Result<Self, WorkerRuntimePlanError> {
        Self::from_env_map(env::vars())
    }

    pub fn from_env_map<I, K, V>(vars: I) -> Result<Self, WorkerRuntimePlanError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let vars = vars
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<BTreeMap<String, String>>();
        Ok(Self {
            worker: required_env(&vars, "BLADB_WORKER_NAME")?,
            manifest_path: PathBuf::from(required_env(&vars, "BLADB_WORKER_MANIFEST")?),
            nats_url: vars.get("BLADB_NATS_URL").cloned(),
            worker_loop: WorkerLoopConfig {
                max_batch: vars
                    .get("BLADB_LOOP_MAX_BATCH")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or_else(default_max_batch),
                idle_sleep_ms: vars
                    .get("BLADB_LOOP_IDLE_SLEEP_MS")
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or_else(default_idle_sleep_ms),
            },
            serve: WorkerServeConfig {
                metrics_bind_addr: vars
                    .get("BLADB_METRICS_BIND_ADDR")
                    .cloned()
                    .unwrap_or_else(default_metrics_bind_addr),
            },
            overrides: WorkerRuntimeOverrides {
                max_concurrency: vars
                    .get("BLADB_WORKER_MAX_CONCURRENCY")
                    .and_then(|value| value.parse::<u16>().ok()),
                labels: vars
                    .iter()
                    .filter_map(|(key, value)| {
                        key.strip_prefix("BLADB_WORKER_LABEL_")
                            .map(|stripped| (stripped.to_ascii_lowercase(), value.clone()))
                    })
                    .collect(),
            },
        })
    }

    pub fn build_plan(&self) -> Result<CompiledWorkerPlan, WorkerRuntimePlanError> {
        let manifest_yaml =
            fs::read_to_string(&self.manifest_path).map_err(|error| WorkerRuntimePlanError::ConfigRead {
                path: self.manifest_path.display().to_string(),
                reason: error.to_string(),
            })?;
        let manifest = parse_worker_manifest(&manifest_yaml)?;
        let worker = manifest
            .workers
            .into_iter()
            .find(|worker| worker.name == self.worker)
            .ok_or_else(|| WorkerRuntimePlanError::UnknownWorker(self.worker.clone()))?;

        let requires_nats = match &worker.trigger {
            WorkerTrigger::Event { transport, .. } | WorkerTrigger::Queue { transport, .. } => {
                matches!(transport.kind, TriggerTransportKind::JetStream)
            }
        };
        if requires_nats && self.nats_url.is_none() {
            return Err(WorkerRuntimePlanError::MissingTransportConfig {
                field: "natsUrl".into(),
                worker: worker.name,
            });
        }

        Ok(compile_worker_plan(worker, self))
    }
}

fn compile_worker_plan(worker: WorkerDefinition, config: &WorkerRuntimeConfig) -> CompiledWorkerPlan {
    let max_concurrency = config
        .overrides
        .max_concurrency
        .or(worker.deployment.max_concurrency)
        .unwrap_or(1);
    let (subject, consumer, stream, transport) = match &worker.trigger {
        WorkerTrigger::Event { transport, .. } | WorkerTrigger::Queue { transport, .. } => (
            transport.subject.clone(),
            transport.consumer.clone(),
            transport.stream.clone(),
            transport.clone(),
        ),
    };

    CompiledWorkerPlan {
        worker: worker.name,
        source: worker.source,
        trigger: worker.trigger,
        identity_mode: worker.identity.mode,
        identity_as: worker.identity.r#as,
        idempotency_key_from: worker.idempotency.key_from,
        retry_max_attempts: worker.retry.max_attempts,
        retry_backoff: worker.retry.backoff,
        dead_letter_subject: worker
            .dead_letter
            .as_ref()
            .and_then(|dead_letter| dead_letter.subject.clone().or(dead_letter.topic.clone()).or(dead_letter.queue.clone())),
        timeout_ms: worker.timeout_ms,
        deployment: worker.deployment,
        max_concurrency,
        subscription: WorkerSubscriptionPlan {
            transport,
            subject,
            consumer,
            stream,
        },
        steps: worker.steps,
        nats_url: config.nats_url.clone(),
        worker_loop: config.worker_loop.clone(),
        labels: config.overrides.labels.clone(),
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkerRuntimePlanError {
    #[error("failed to read config file `{path}`: {reason}")]
    ConfigRead { path: String, reason: String },
    #[error("failed to parse worker runtime config: {0}")]
    ConfigParse(String),
    #[error("missing required environment variable `{0}`")]
    MissingEnv(&'static str),
    #[error(transparent)]
    Worker(#[from] WorkerManifestError),
    #[error("worker `{0}` was not found in worker manifest")]
    UnknownWorker(String),
    #[error("missing transport config `{field}` for worker `{worker}`")]
    MissingTransportConfig { field: String, worker: String },
}

fn required_env(
    vars: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<String, WorkerRuntimePlanError> {
    vars.get(key)
        .cloned()
        .ok_or(WorkerRuntimePlanError::MissingEnv(key))
}

fn resolve_relative(base_dir: &Path, target: &str) -> PathBuf {
    let path = Path::new(target);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn default_metrics_bind_addr() -> String {
    "0.0.0.0:9091".into()
}

fn default_max_batch() -> usize {
    64
}

fn default_idle_sleep_ms() -> u64 {
    25
}

#[cfg(test)]
mod tests {
    use super::{
        CompiledWorkerPlan, WorkerRuntimeConfig, WorkerRuntimeConfigFile, WorkerRuntimeOverrides,
        WorkerLoopConfig, WorkerRuntimePlanError,
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
    fn builds_worker_plan_from_real_flash_sale_fixture() {
        let config = WorkerRuntimeConfig {
            worker: "order.payment-timeout-handler".into(),
            manifest_path: workspace_root()
                .join("apps/examples/flash-sale/workers/flash-sale.workers.yaml"),
            nats_url: Some("nats://nats.bladb.svc.cluster.local:4222".into()),
            worker_loop: WorkerLoopConfig {
                max_batch: 16,
                idle_sleep_ms: 20,
            },
            serve: super::WorkerServeConfig::default(),
            overrides: WorkerRuntimeOverrides::default(),
        };

        let plan = config.build_plan().expect("build worker plan");
        assert_eq!(plan.worker, "order.payment-timeout-handler");
        assert_eq!(
            plan.subscription.subject.as_deref(),
            Some("queue.flashsale.order.payment.timeout")
        );
        assert_eq!(plan.dead_letter_subject.as_deref(), Some("dlq.flashsale.order.payment.timeout"));
        assert_eq!(plan.max_concurrency, 8);
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.worker_loop.max_batch, 16);
        assert_eq!(plan.worker_loop.idle_sleep_ms, 20);
    }

    #[test]
    fn worker_plan_override_can_raise_max_concurrency() {
        let config = WorkerRuntimeConfig {
            worker: "telemetry.counter-updater".into(),
            manifest_path: workspace_root()
                .join("apps/examples/iot-realtime/workers/iot-realtime.workers.yaml"),
            nats_url: Some("nats://nats.bladb.svc.cluster.local:4222".into()),
            worker_loop: WorkerLoopConfig::default(),
            serve: super::WorkerServeConfig::default(),
            overrides: WorkerRuntimeOverrides {
                max_concurrency: Some(96),
                labels: Default::default(),
            },
        };

        let plan = config.build_plan().expect("build worker plan");
        assert_eq!(plan.worker, "telemetry.counter-updater");
        assert_eq!(plan.max_concurrency, 96);
        assert_eq!(
            plan.subscription.stream.as_deref(),
            Some("BLADB_IOT_EVENTS")
        );
    }

    #[test]
    fn loads_worker_runtime_config_from_env_map() {
        let config = WorkerRuntimeConfig::from_env_map([
            ("BLADB_WORKER_NAME", "telemetry.alert-evaluator"),
            (
                "BLADB_WORKER_MANIFEST",
                "D:/study/bladb/apps/examples/iot-realtime/workers/iot-realtime.workers.yaml",
            ),
            ("BLADB_NATS_URL", "nats://nats.bladb.svc.cluster.local:4222"),
            ("BLADB_LOOP_MAX_BATCH", "128"),
            ("BLADB_LOOP_IDLE_SLEEP_MS", "60"),
            ("BLADB_METRICS_BIND_ADDR", "0.0.0.0:9191"),
            ("BLADB_WORKER_MAX_CONCURRENCY", "48"),
            ("BLADB_WORKER_LABEL_APP", "iot-alerts"),
        ])
        .expect("config from env map");

        assert_eq!(config.worker, "telemetry.alert-evaluator");
        assert_eq!(config.serve.metrics_bind_addr, "0.0.0.0:9191");
        assert_eq!(config.worker_loop.max_batch, 128);
        assert_eq!(config.worker_loop.idle_sleep_ms, 60);
        assert_eq!(config.overrides.max_concurrency, Some(48));
        assert_eq!(
            config.overrides.labels.get("app").map(String::as_str),
            Some("iot-alerts")
        );
    }

    #[test]
    fn rejects_unknown_worker_name() {
        let config = WorkerRuntimeConfig {
            worker: "missing-worker".into(),
            manifest_path: workspace_root()
                .join("apps/examples/flash-sale/workers/flash-sale.workers.yaml"),
            nats_url: None,
            worker_loop: WorkerLoopConfig::default(),
            serve: super::WorkerServeConfig::default(),
            overrides: WorkerRuntimeOverrides::default(),
        };

        let error = config.build_plan().expect_err("expected unknown worker");
        assert_eq!(error, WorkerRuntimePlanError::UnknownWorker("missing-worker".into()));
    }

    #[test]
    fn resolves_relative_worker_manifest_path_from_file_config() {
        let file = WorkerRuntimeConfigFile {
            worker: "order.analytics-sync".into(),
            manifest: "../../apps/examples/flash-sale/workers/flash-sale.workers.yaml".into(),
            nats_url: None,
            worker_loop: WorkerLoopConfig::default(),
            serve: super::WorkerServeConfig::default(),
            overrides: WorkerRuntimeOverrides::default(),
        };

        let config = WorkerRuntimeConfig::from_file_config(
            file,
            Path::new("D:/study/bladb/crates/bladb-worker-runtime/examples"),
        );
        assert_eq!(
            config.manifest_path,
            PathBuf::from(
                "D:/study/bladb/crates/bladb-worker-runtime/examples/../../apps/examples/flash-sale/workers/flash-sale.workers.yaml"
            )
        );
    }

    #[test]
    fn compiled_worker_plan_keeps_step_shapes_for_native_backends() {
        let plan = CompiledWorkerPlan {
            worker: "telemetry.counter-updater".into(),
            source: "mqtt.devices.telemetry".into(),
            trigger: bladb_core::worker::WorkerTrigger::Event {
                topic: "telemetry.received".into(),
                transport: bladb_core::worker::TriggerTransportConfig {
                    kind: bladb_core::worker::TriggerTransportKind::JetStream,
                    stream: Some("BLADB_IOT_EVENTS".into()),
                    consumer: Some("iot-telemetry-counter-updater".into()),
                    subject: Some("events.iot.telemetry.received".into()),
                },
            },
            identity_mode: "system".into(),
            identity_as: Some("worker.telemetry-counter-updater".into()),
            idempotency_key_from: "event.payload.deviceId".into(),
            retry_max_attempts: 5,
            retry_backoff: "exponential".into(),
            dead_letter_subject: None,
            timeout_ms: 5000,
            deployment: bladb_core::worker::WorkerDeploymentConfig {
                min_replicas: 2,
                max_replicas: Some(15),
                max_concurrency: Some(64),
                rolling_max_unavailable: Some("1".into()),
            },
            max_concurrency: 64,
            subscription: super::WorkerSubscriptionPlan {
                transport: bladb_core::worker::TriggerTransportConfig {
                    kind: bladb_core::worker::TriggerTransportKind::JetStream,
                    stream: Some("BLADB_IOT_EVENTS".into()),
                    consumer: Some("iot-telemetry-counter-updater".into()),
                    subject: Some("events.iot.telemetry.received".into()),
                },
                subject: Some("events.iot.telemetry.received".into()),
                consumer: Some("iot-telemetry-counter-updater".into()),
                stream: Some("BLADB_IOT_EVENTS".into()),
            },
            steps: vec![
                bladb_core::worker::WorkerStep {
                    r#use: "redis".into(),
                    action: "setOnlineState".into(),
                    collection: None,
                    topic: None,
                    queue: None,
                    table: None,
                    key_template: Some(
                        "iot:{event.actor.tenantId}:devices:{event.payload.deviceId}:online"
                            .into(),
                    ),
                    delay_ms: None,
                },
                bladb_core::worker::WorkerStep {
                    r#use: "redis".into(),
                    action: "recomputeCounter".into(),
                    collection: None,
                    topic: None,
                    queue: None,
                    table: None,
                    key_template: Some("iot:{event.actor.tenantId}:active-count".into()),
                    delay_ms: None,
                },
            ],
            nats_url: Some("nats://nats.bladb.svc.cluster.local:4222".into()),
            worker_loop: WorkerLoopConfig {
                max_batch: 24,
                idle_sleep_ms: 35,
            },
            labels: Default::default(),
        };

        assert_eq!(plan.steps[0].r#use, "redis");
        assert_eq!(plan.steps[0].action, "setOnlineState");
        assert_eq!(
            plan.steps[0].key_template.as_deref(),
            Some("iot:{event.actor.tenantId}:devices:{event.payload.deviceId}:online")
        );
        assert_eq!(plan.steps[1].action, "recomputeCounter");
        assert_eq!(plan.worker_loop.max_batch, 24);
    }

    #[test]
    fn loads_real_example_worker_runtime_config_file() {
        let path = workspace_root()
            .join("apps/examples/flash-sale/runtime/order.payment-timeout-handler.runtime.yaml");
        let config = WorkerRuntimeConfig::from_path(&path).expect("load worker runtime config");

        assert_eq!(config.worker, "order.payment-timeout-handler");
        assert_eq!(
            config.manifest_path,
            workspace_root().join("apps/examples/flash-sale/workers/flash-sale.workers.yaml")
        );
        assert_eq!(config.worker_loop.max_batch, 64);
        assert_eq!(config.worker_loop.idle_sleep_ms, 25);
        assert_eq!(
            config.overrides.labels.get("app").map(String::as_str),
            Some("flashsale-workers")
        );
    }

    #[test]
    fn jetstream_worker_requires_nats_url() {
        let config = WorkerRuntimeConfig {
            worker: "telemetry.counter-updater".into(),
            manifest_path: workspace_root()
                .join("apps/examples/iot-realtime/workers/iot-realtime.workers.yaml"),
            nats_url: None,
            worker_loop: WorkerLoopConfig::default(),
            serve: super::WorkerServeConfig::default(),
            overrides: WorkerRuntimeOverrides::default(),
        };

        let error = config.build_plan().expect_err("expected missing nats url");
        assert_eq!(
            error,
            WorkerRuntimePlanError::MissingTransportConfig {
                field: "natsUrl".into(),
                worker: "telemetry.counter-updater".into(),
            }
        );
    }
}
