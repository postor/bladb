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
    pub steps: Vec<WorkerStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WorkerTrigger {
    Event { topic: String, backend: String },
    Queue { queue: String, backend: String },
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
}

pub fn parse_worker_manifest(yaml: &str) -> Result<WorkerManifest, WorkerManifestError> {
    let manifest: WorkerManifest =
        serde_yaml::from_str(yaml).map_err(|error| WorkerManifestError::Parse(error.to_string()))?;

    let mut seen = HashSet::new();
    for worker in &manifest.workers {
        if !seen.insert(worker.name.clone()) {
            return Err(WorkerManifestError::DuplicateWorkerName(
                worker.name.clone(),
            ));
        }
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::{parse_worker_manifest, WorkerManifestError, WorkerTrigger};

    #[test]
    fn flash_sale_worker_manifest_parses_from_real_fixture() {
        let yaml = include_str!("../../../apps/examples/flash-sale/workers/flash-sale.workers.yaml");
        let manifest = parse_worker_manifest(yaml).expect("parse flash sale workers");

        assert_eq!(manifest.workers.len(), 3);
        assert_eq!(manifest.workers[0].name, "order.analytics-sync");
        assert_eq!(
            manifest.workers[0].trigger,
            WorkerTrigger::Event {
                topic: "order.created".into(),
                backend: "kafka".into()
            }
        );
        assert_eq!(manifest.workers[2].dead_letter.as_ref().and_then(|dlq| dlq.queue.as_deref()), Some("order.payment.timeout.dlq"));
    }

    #[test]
    fn iot_worker_manifest_parses_and_preserves_key_templates() {
        let yaml = include_str!("../../../apps/examples/iot-realtime/workers/iot-realtime.workers.yaml");
        let manifest = parse_worker_manifest(yaml).expect("parse iot workers");

        assert_eq!(manifest.workers.len(), 3);
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
      backend: kafka
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
      - use: kafka
        action: produce
        topic: next
  - name: same-worker
    trigger:
      type: queue
      queue: retry
      backend: mq
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
