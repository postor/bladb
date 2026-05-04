use crate::{
    config::CompiledWorkerPlan,
    executor::{StepExecutorRegistry, StepInvocation, WorkerExecutionError},
};
use bladb_core::bus::{WorkerExecutionReport, WorkerJob, WorkerStepResult};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerRuntimeStatus {
    pub worker: String,
    pub source: String,
    pub metrics_bind_addr: String,
    pub nats_url: Option<String>,
    pub trigger_subject: Option<String>,
    pub trigger_stream: Option<String>,
    pub trigger_consumer: Option<String>,
    pub loop_max_batch: usize,
    pub loop_idle_sleep_ms: u64,
    pub max_concurrency: u16,
    pub min_replicas: u16,
    pub max_replicas: Option<u16>,
    pub dead_letter_subject: Option<String>,
}

#[derive(Clone)]
pub struct WorkerRuntimeService {
    plan: CompiledWorkerPlan,
    metrics_bind_addr: String,
    executors: StepExecutorRegistry,
}

impl WorkerRuntimeService {
    pub fn new(
        plan: CompiledWorkerPlan,
        metrics_bind_addr: String,
        executors: StepExecutorRegistry,
    ) -> Self {
        Self {
            plan,
            metrics_bind_addr,
            executors,
        }
    }

    pub fn plan(&self) -> &CompiledWorkerPlan {
        &self.plan
    }

    pub fn status(&self) -> WorkerRuntimeStatus {
        WorkerRuntimeStatus {
            worker: self.plan.worker.clone(),
            source: self.plan.source.clone(),
            metrics_bind_addr: self.metrics_bind_addr.clone(),
            nats_url: self.plan.nats_url.clone(),
            trigger_subject: self.plan.subscription.subject.clone(),
            trigger_stream: self.plan.subscription.stream.clone(),
            trigger_consumer: self.plan.subscription.consumer.clone(),
            loop_max_batch: self.plan.worker_loop.max_batch,
            loop_idle_sleep_ms: self.plan.worker_loop.idle_sleep_ms,
            max_concurrency: self.plan.max_concurrency,
            min_replicas: self.plan.deployment.min_replicas,
            max_replicas: self.plan.deployment.max_replicas,
            dead_letter_subject: self.plan.dead_letter_subject.clone(),
        }
    }

    pub fn execute_job(
        &self,
        job: WorkerJob,
    ) -> Result<WorkerExecutionReport, WorkerExecutionError> {
        if job.worker != self.plan.worker {
            return Err(WorkerExecutionError::invalid_request(format!(
                "worker job targeted `{}` but runtime serves `{}`",
                job.worker, self.plan.worker
            )));
        }

        validate_job_subscription(&self.plan, &job)?;

        let mut results = Vec::with_capacity(self.plan.steps.len());
        for (step_index, step) in self.plan.steps.iter().enumerate() {
            let data = self.executors.execute_step(&StepInvocation {
                plan: self.plan.clone(),
                step_index,
                job: job.clone(),
            })?;
            results.push(WorkerStepResult {
                step_index,
                backend: step.r#use.clone(),
                action: step.action.clone(),
                data,
            });
        }

        Ok(WorkerExecutionReport {
            worker: self.plan.worker.clone(),
            attempt: job.attempt,
            event_id: job.event.event_id,
            success: true,
            results,
        })
    }

    pub fn healthcheck(&self) -> Value {
        serde_json::json!({
            "ok": true,
            "runtime": self.status(),
        })
    }
}

fn validate_job_subscription(
    plan: &CompiledWorkerPlan,
    job: &WorkerJob,
) -> Result<(), WorkerExecutionError> {
    if let Some(expected) = plan.subscription.subject.as_deref() {
        if job.trigger_subject != expected {
            return Err(WorkerExecutionError::invalid_request(format!(
                "worker job trigger subject `{}` does not match runtime subscription `{}`",
                job.trigger_subject, expected
            )));
        }
    }

    if let Some(expected) = plan.subscription.stream.as_deref() {
        if job.trigger_stream.as_deref() != Some(expected) {
            return Err(WorkerExecutionError::invalid_request(format!(
                "worker job trigger stream `{:?}` does not match runtime subscription `{}`",
                job.trigger_stream, expected
            )));
        }
    }

    if let Some(expected) = plan.subscription.consumer.as_deref() {
        if job.trigger_consumer.as_deref() != Some(expected) {
            return Err(WorkerExecutionError::invalid_request(format!(
                "worker job trigger consumer `{:?}` does not match runtime subscription `{}`",
                job.trigger_consumer, expected
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::WorkerRuntimeService;
    use crate::{
        config::{CompiledWorkerPlan, WorkerSubscriptionPlan},
        executor::{StepExecutor, StepExecutorRegistry, WorkerExecutionError},
    };
    use bladb_core::{
        bus::WorkerJob,
        event::{ActorContext, EventEnvelope},
        worker::{
            TriggerTransportConfig, TriggerTransportKind, WorkerDeploymentConfig, WorkerStep,
            WorkerTrigger,
        },
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
            invocation: &crate::executor::StepInvocation,
        ) -> Result<serde_json::Value, WorkerExecutionError> {
            let step = &invocation.plan.steps[invocation.step_index];
            Ok(json!({
                "backend": step.r#use,
                "action": step.action,
                "tenantId": invocation.job.event.payload["tenantId"],
                "attempt": invocation.job.attempt,
            }))
        }
    }

    fn plan() -> CompiledWorkerPlan {
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
            steps: vec![
                WorkerStep {
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
                },
                WorkerStep {
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
            worker_loop: crate::config::WorkerLoopConfig {
                max_batch: 12,
                idle_sleep_ms: 18,
            },
            labels: Default::default(),
        }
    }

    fn job() -> WorkerJob {
        WorkerJob {
            worker: "telemetry.counter-updater".into(),
            attempt: 2,
            trigger_subject: "events.iot.telemetry.received".into(),
            trigger_stream: Some("BLADB_IOT_EVENTS".into()),
            trigger_consumer: Some("iot-telemetry-counter-updater".into()),
            event: EventEnvelope {
                event_id: "evt_01".into(),
                event_type: "telemetry.received".into(),
                source: "mqtt.devices.telemetry".into(),
                trace_id: "trace_01".into(),
                partition_key: Some("tenant_a:device-001".into()),
                ordering_key: Some("device-001".into()),
                actor: ActorContext {
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
    fn service_status_includes_trigger_and_scaling_shape() {
        let service = WorkerRuntimeService::new(
            plan(),
            "0.0.0.0:9091".into(),
            StepExecutorRegistry::new(vec![]),
        );
        let status = service.status();

        assert_eq!(status.worker, "telemetry.counter-updater");
        assert_eq!(
            status.trigger_subject.as_deref(),
            Some("events.iot.telemetry.received")
        );
        assert_eq!(status.loop_max_batch, 12);
        assert_eq!(status.loop_idle_sleep_ms, 18);
        assert_eq!(status.max_concurrency, 64);
        assert_eq!(status.min_replicas, 2);
        assert_eq!(status.max_replicas, Some(15));
    }

    #[test]
    fn service_executes_all_worker_steps_and_builds_report() {
        let service = WorkerRuntimeService::new(
            plan(),
            "0.0.0.0:9091".into(),
            StepExecutorRegistry::new(vec![Arc::new(RedisExecutor)]),
        );

        let report = service.execute_job(job()).expect("execute worker job");
        assert_eq!(report.worker, "telemetry.counter-updater");
        assert_eq!(report.attempt, 2);
        assert_eq!(report.results.len(), 2);
        assert_eq!(report.results[0].backend, "redis");
        assert_eq!(report.results[1].action, "recomputeCounter");
        assert_eq!(report.results[0].data["attempt"], 2);
    }

    #[test]
    fn service_rejects_job_for_another_worker() {
        let service = WorkerRuntimeService::new(
            plan(),
            "0.0.0.0:9091".into(),
            StepExecutorRegistry::new(vec![Arc::new(RedisExecutor)]),
        );
        let mut wrong_job = job();
        wrong_job.worker = "telemetry.alert-evaluator".into();

        let error = service
            .execute_job(wrong_job)
            .expect_err("expected worker mismatch");
        assert_eq!(
            error,
            WorkerExecutionError::invalid_request(
                "worker job targeted `telemetry.alert-evaluator` but runtime serves `telemetry.counter-updater`"
            )
        );
    }

    #[test]
    fn service_rejects_job_with_wrong_trigger_subject() {
        let service = WorkerRuntimeService::new(
            plan(),
            "0.0.0.0:9091".into(),
            StepExecutorRegistry::new(vec![Arc::new(RedisExecutor)]),
        );
        let mut wrong_job = job();
        wrong_job.trigger_subject = "events.iot.telemetry.other".into();

        let error = service
            .execute_job(wrong_job)
            .expect_err("expected trigger mismatch");
        assert_eq!(
            error,
            WorkerExecutionError::invalid_request(
                "worker job trigger subject `events.iot.telemetry.other` does not match runtime subscription `events.iot.telemetry.received`"
            )
        );
    }
}
