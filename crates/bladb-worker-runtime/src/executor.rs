use crate::config::CompiledWorkerPlan;
use bladb_core::bus::WorkerJob;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct StepInvocation {
    pub plan: CompiledWorkerPlan,
    pub step_index: usize,
    pub job: WorkerJob,
}

pub trait StepExecutor: Send + Sync {
    fn handles_backend(&self, backend: &str) -> bool;
    fn execute(&self, invocation: &StepInvocation) -> Result<Value, WorkerExecutionError>;
}

#[derive(Clone)]
pub struct StepExecutorRegistry {
    executors: Vec<Arc<dyn StepExecutor>>,
}

impl StepExecutorRegistry {
    pub fn new(executors: Vec<Arc<dyn StepExecutor>>) -> Self {
        Self { executors }
    }

    pub fn execute_step(&self, invocation: &StepInvocation) -> Result<Value, WorkerExecutionError> {
        let step = invocation
            .plan
            .steps
            .get(invocation.step_index)
            .ok_or_else(|| {
                WorkerExecutionError::internal(format!(
                    "worker `{}` has no step at index {}",
                    invocation.plan.worker, invocation.step_index
                ))
            })?;

        self.executors
            .iter()
            .find(|executor| executor.handles_backend(&step.r#use))
            .ok_or_else(|| {
                WorkerExecutionError::internal(format!(
                    "no step executor registered for backend `{}`",
                    step.r#use
                ))
            })?
            .execute(invocation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerExecutionError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl WorkerExecutionError {
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            code: "INVALID_REQUEST",
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            code: "INTERNAL_ERROR",
            message: message.into(),
        }
    }

    pub fn execution(
        backend: impl AsRef<str>,
        action: impl AsRef<str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: 500,
            code: "EXECUTION_ERROR",
            message: format!(
                "{}:{} {}",
                backend.as_ref(),
                action.as_ref(),
                message.into()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StepExecutor, StepExecutorRegistry, StepInvocation, WorkerExecutionError};
    use crate::config::{CompiledWorkerPlan, WorkerLoopConfig, WorkerSubscriptionPlan};
    use bladb_core::worker::{
        TriggerTransportConfig, TriggerTransportKind, WorkerDeploymentConfig, WorkerStep,
        WorkerTrigger,
    };
    use serde_json::json;
    use std::sync::Arc;

    struct RedisExecutor;

    impl StepExecutor for RedisExecutor {
        fn handles_backend(&self, backend: &str) -> bool {
            backend == "redis"
        }

        fn execute(
            &self,
            invocation: &StepInvocation,
        ) -> Result<serde_json::Value, WorkerExecutionError> {
            let step = &invocation.plan.steps[invocation.step_index];
            Ok(json!({
                "worker": invocation.plan.worker,
                "backend": step.r#use,
                "action": step.action,
                "eventId": invocation.job.event.event_id,
                "payload": invocation.job.event.payload,
            }))
        }
    }

    fn compiled_plan() -> CompiledWorkerPlan {
        CompiledWorkerPlan {
            worker: "telemetry.counter-updater".into(),
            source: "mqtt.devices.telemetry".into(),
            trigger: WorkerTrigger::Event {
                topic: "telemetry.received".into(),
                transport: TriggerTransportConfig {
                    kind: TriggerTransportKind::JetStream,
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
            deployment: WorkerDeploymentConfig {
                min_replicas: 2,
                max_replicas: Some(15),
                max_concurrency: Some(64),
                rolling_max_unavailable: Some("1".into()),
            },
            max_concurrency: 64,
            subscription: WorkerSubscriptionPlan {
                transport: TriggerTransportConfig {
                    kind: TriggerTransportKind::JetStream,
                    stream: Some("BLADB_IOT_EVENTS".into()),
                    consumer: Some("iot-telemetry-counter-updater".into()),
                    subject: Some("events.iot.telemetry.received".into()),
                },
                subject: Some("events.iot.telemetry.received".into()),
                consumer: Some("iot-telemetry-counter-updater".into()),
                stream: Some("BLADB_IOT_EVENTS".into()),
            },
            steps: vec![WorkerStep {
                r#use: "redis".into(),
                action: "setOnlineState".into(),
                collection: None,
                topic: None,
                queue: None,
                table: None,
                key_template: Some(
                    "iot:{event.actor.tenantId}:devices:{event.payload.deviceId}:online".into(),
                ),
                delay_ms: None,
            }],
            nats_url: Some("nats://nats.bladb.svc.cluster.local:4222".into()),
            worker_loop: WorkerLoopConfig::default(),
            labels: Default::default(),
        }
    }

    fn worker_job() -> WorkerJob {
        WorkerJob {
            worker: "telemetry.counter-updater".into(),
            attempt: 2,
            trigger_subject: "events.iot.telemetry.received".into(),
            trigger_stream: Some("BLADB_IOT_EVENTS".into()),
            trigger_consumer: Some("iot-telemetry-counter-updater".into()),
            event: bladb_core::event::EventEnvelope {
                event_id: "evt_01".into(),
                event_type: "telemetry.received".into(),
                source: "mqtt.devices.telemetry".into(),
                trace_id: "trace_01".into(),
                partition_key: Some("tenant_a:device-001".into()),
                ordering_key: Some("device-001".into()),
                actor: bladb_core::event::ActorContext {
                    kind: "device".into(),
                    uid: Some("u_1001".into()),
                    tenant_id: Some("tenant_a".into()),
                    roles: vec!["operator".into()],
                    worker: None,
                },
                payload: json!({
                    "deviceId": "device-001",
                    "tenantId": "tenant_a"
                }),
            },
        }
    }

    #[test]
    fn step_executor_registry_dispatches_to_matching_backend_executor() {
        let registry = StepExecutorRegistry::new(vec![Arc::new(RedisExecutor)]);
        let invocation = StepInvocation {
            plan: compiled_plan(),
            step_index: 0,
            job: worker_job(),
        };

        let value = registry
            .execute_step(&invocation)
            .expect("execute worker step");
        assert_eq!(value["worker"], "telemetry.counter-updater");
        assert_eq!(value["backend"], "redis");
        assert_eq!(value["action"], "setOnlineState");
        assert_eq!(value["eventId"], "evt_01");
    }

    #[test]
    fn step_executor_registry_reports_missing_backend_executor() {
        let registry = StepExecutorRegistry::new(vec![Arc::new(RedisExecutor)]);
        let mut plan = compiled_plan();
        plan.steps[0].r#use = "mongo".into();
        let invocation = StepInvocation {
            plan,
            step_index: 0,
            job: worker_job(),
        };

        let error = registry
            .execute_step(&invocation)
            .expect_err("expected missing backend executor");
        assert_eq!(
            error,
            WorkerExecutionError::internal("no step executor registered for backend `mongo`")
        );
    }

    #[test]
    fn step_executor_registry_reports_missing_step_index() {
        let registry = StepExecutorRegistry::new(vec![Arc::new(RedisExecutor)]);
        let invocation = StepInvocation {
            plan: compiled_plan(),
            step_index: 3,
            job: worker_job(),
        };

        let error = registry
            .execute_step(&invocation)
            .expect_err("expected missing step");
        assert_eq!(
            error,
            WorkerExecutionError::internal(
                "worker `telemetry.counter-updater` has no step at index 3"
            )
        );
    }
}
