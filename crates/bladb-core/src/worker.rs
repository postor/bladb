use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerManifest {
    pub workers: Vec<WorkerDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerDefinition {
    pub name: String,
    pub trigger: WorkerTrigger,
    pub source: String,
    pub identity: IdentityMode,
    pub idempotency: IdempotencyConfig,
    pub retry: RetryConfig,
    #[serde(default)]
    pub dead_letter: Option<DeadLetterConfig>,
    pub timeout_ms: u64,
    #[serde(default)]
    pub deployment: WorkerDeploymentConfig,
    pub steps: Vec<WorkerStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WorkerTrigger {
    Event {
        topic: String,
        transport: TriggerTransportConfig,
    },
    Queue {
        queue: String,
        transport: TriggerTransportConfig,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerTransportConfig {
    pub kind: TriggerTransportKind,
    #[serde(default)]
    pub stream: Option<String>,
    #[serde(default)]
    pub consumer: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TriggerTransportKind {
    JetStream,
    Broker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityMode {
    pub mode: String,
    #[serde(default)]
    pub r#as: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdempotencyConfig {
    pub key_from: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeadLetterConfig {
    #[serde(default)]
    pub queue: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerDeploymentConfig {
    #[serde(default = "default_min_replicas")]
    pub min_replicas: u16,
    #[serde(default)]
    pub max_replicas: Option<u16>,
    #[serde(default)]
    pub max_concurrency: Option<u16>,
    #[serde(default)]
    pub rolling_max_unavailable: Option<String>,
}

impl Default for WorkerDeploymentConfig {
    fn default() -> Self {
        Self {
            min_replicas: default_min_replicas(),
            max_replicas: None,
            max_concurrency: None,
            rolling_max_unavailable: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerStep {
    pub r#use: String,
    pub action: String,
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub queue: Option<String>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub key_template: Option<String>,
    #[serde(default)]
    pub delay_ms: Option<u64>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkerManifestError {
    #[error("failed to parse worker manifest: {0}")]
    Parse(String),
    #[error("duplicate worker name `{0}`")]
    DuplicateWorkerName(String),
    #[error("invalid worker `{worker}`: {reason}")]
    InvalidWorker { worker: String, reason: String },
}

pub fn parse_worker_manifest(yaml: &str) -> Result<WorkerManifest, WorkerManifestError> {
    let manifest: WorkerManifest = serde_yaml::from_str(yaml)
        .map_err(|error| WorkerManifestError::Parse(error.to_string()))?;

    let mut seen = HashSet::new();
    for worker in &manifest.workers {
        if !seen.insert(worker.name.clone()) {
            return Err(WorkerManifestError::DuplicateWorkerName(
                worker.name.clone(),
            ));
        }

        validate_worker(worker)?;
    }

    Ok(manifest)
}

fn validate_worker(worker: &WorkerDefinition) -> Result<(), WorkerManifestError> {
    match &worker.trigger {
        WorkerTrigger::Event { transport, .. } | WorkerTrigger::Queue { transport, .. } => {
            if matches!(transport.kind, TriggerTransportKind::JetStream) {
                if transport.stream.is_none() {
                    return Err(WorkerManifestError::InvalidWorker {
                        worker: worker.name.clone(),
                        reason: "jetStream trigger transport requires stream".into(),
                    });
                }

                if transport.consumer.is_none() {
                    return Err(WorkerManifestError::InvalidWorker {
                        worker: worker.name.clone(),
                        reason: "jetStream trigger transport requires consumer".into(),
                    });
                }

                if transport.subject.is_none() {
                    return Err(WorkerManifestError::InvalidWorker {
                        worker: worker.name.clone(),
                        reason: "jetStream trigger transport requires subject".into(),
                    });
                }
            }
        }
    }

    if worker.deployment.min_replicas == 0 {
        return Err(WorkerManifestError::InvalidWorker {
            worker: worker.name.clone(),
            reason: "deployment minReplicas must be greater than 0".into(),
        });
    }

    if let Some(max_replicas) = worker.deployment.max_replicas {
        if max_replicas < worker.deployment.min_replicas {
            return Err(WorkerManifestError::InvalidWorker {
                worker: worker.name.clone(),
                reason: "deployment maxReplicas must be greater than or equal to minReplicas"
                    .into(),
            });
        }
    }

    Ok(())
}

fn default_min_replicas() -> u16 {
    1
}

#[cfg(test)]
mod tests {
    use super::{parse_worker_manifest, TriggerTransportKind, WorkerManifestError, WorkerTrigger};

    #[test]
    fn flash_sale_worker_manifest_parses_from_real_fixture() {
        let yaml =
            include_str!("../../../apps/examples/flash-sale/workers/flash-sale.workers.yaml");
        let manifest = parse_worker_manifest(yaml).expect("parse flash sale workers");

        assert_eq!(manifest.workers.len(), 3);
        assert_eq!(manifest.workers[0].name, "order.analytics-sync");
        assert_eq!(
            manifest.workers[0].trigger,
            WorkerTrigger::Event {
                topic: "order.created".into(),
                transport: super::TriggerTransportConfig {
                    kind: TriggerTransportKind::JetStream,
                    stream: Some("BLADB_FLASHSALE_EVENTS".into()),
                    consumer: Some("flashsale-order-analytics".into()),
                    subject: Some("events.flashsale.order.created".into()),
                }
            }
        );
        assert_eq!(manifest.workers[1].deployment.max_replicas, Some(10));
        assert_eq!(
            manifest.workers[2]
                .dead_letter
                .as_ref()
                .and_then(|dlq| dlq.subject.as_deref()),
            Some("dlq.flashsale.order.payment.timeout")
        );
    }

    #[test]
    fn iot_worker_manifest_parses_and_preserves_key_templates() {
        let yaml =
            include_str!("../../../apps/examples/iot-realtime/workers/iot-realtime.workers.yaml");
        let manifest = parse_worker_manifest(yaml).expect("parse iot workers");

        assert_eq!(manifest.workers.len(), 3);
        assert_eq!(manifest.workers[0].deployment.min_replicas, 2);
        assert_eq!(manifest.workers[1].steps.len(), 2);
        assert_eq!(
            manifest.workers[1].steps[0].key_template.as_deref(),
            Some("iot:{event.actor.tenantId}:devices:{event.payload.deviceId}:online")
        );
    }

    #[test]
    fn duplicate_worker_names_are_rejected() {
        let yaml = r#"
workers:
  - name: same-worker
    trigger:
      type: event
      topic: order.created
      transport:
        kind: jetStream
        stream: BLADB_EVENTS
        consumer: same-worker
        subject: events.order.created
    source: sql.orders
    identity:
      mode: inherit-actor
    idempotency:
      keyFrom: event.eventId
    retry:
      maxAttempts: 5
      backoff: exponential
    timeoutMs: 1000
    steps:
      - use: nats
        action: produce
        topic: next
  - name: same-worker
    trigger:
      type: queue
      queue: retry
      transport:
        kind: jetStream
        stream: BLADB_RETRY
        consumer: same-worker-retry
        subject: queue.retry
    source: mq.retry
    identity:
      mode: system
    idempotency:
      keyFrom: job.id
    retry:
      maxAttempts: 3
      backoff: exponential
    timeoutMs: 1000
    steps:
      - use: sql
        action: update
        table: jobs
"#;

        let error = parse_worker_manifest(yaml).expect_err("expected duplicate worker names");
        assert_eq!(
            error,
            WorkerManifestError::DuplicateWorkerName("same-worker".into())
        );
    }
}
