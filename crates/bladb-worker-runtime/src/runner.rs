use crate::{
    service::WorkerRuntimeService,
    WorkerExecutionError,
};
use bladb_core::bus::{WorkerExecutionReport, WorkerJob};

pub trait WorkerJobInbox {
    fn next_job(&mut self) -> Option<WorkerJob>;
    fn send_report(&mut self, report: Result<WorkerExecutionReport, WorkerExecutionError>);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRunStats {
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
}

pub struct WorkerRuntimeRunner {
    service: WorkerRuntimeService,
}

impl WorkerRuntimeRunner {
    pub fn new(service: WorkerRuntimeService) -> Self {
        Self { service }
    }

    pub fn service(&self) -> &WorkerRuntimeService {
        &self.service
    }

    pub fn execute_job(
        &self,
        job: WorkerJob,
    ) -> Result<WorkerExecutionReport, WorkerExecutionError> {
        self.service.execute_job(job)
    }

    pub fn run_one(&self, inbox: &mut dyn WorkerJobInbox) -> bool {
        match inbox.next_job() {
            Some(job) => {
                let report = self.service.execute_job(job);
                inbox.send_report(report);
                true
            }
            None => false,
        }
    }

    pub fn run_batch(&self, inbox: &mut dyn WorkerJobInbox, max_batch: usize) -> WorkerRunStats {
        let mut stats = WorkerRunStats {
            processed: 0,
            succeeded: 0,
            failed: 0,
        };

        while stats.processed < max_batch {
            let Some(job) = inbox.next_job() else {
                break;
            };
            let report = self.service.execute_job(job);
            stats.processed += 1;
            if report.is_ok() {
                stats.succeeded += 1;
            } else {
                stats.failed += 1;
            }
            inbox.send_report(report);
        }

        stats
    }

    pub fn run_until_empty(&self, inbox: &mut dyn WorkerJobInbox) -> WorkerRunStats {
        self.run_batch(inbox, usize::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkerJobInbox, WorkerRuntimeRunner};
    use crate::{
        config::{CompiledWorkerPlan, WorkerLoopConfig, WorkerSubscriptionPlan},
        executor::{StepExecutor, StepExecutorRegistry, WorkerExecutionError},
        service::WorkerRuntimeService,
    };
    use bladb_core::{
        bus::{WorkerExecutionReport, WorkerJob},
        event::{ActorContext, EventEnvelope},
        worker::{
            TriggerTransportConfig, TriggerTransportKind, WorkerDeploymentConfig, WorkerStep,
            WorkerTrigger,
        },
    };
    use serde_json::json;
    use std::collections::VecDeque;
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
            Ok(json!({
                "eventId": invocation.job.event.event_id,
                "attempt": invocation.job.attempt,
            }))
        }
    }

    struct MemoryInbox {
        jobs: VecDeque<WorkerJob>,
        reports: Vec<Result<WorkerExecutionReport, WorkerExecutionError>>,
    }

    impl WorkerJobInbox for MemoryInbox {
        fn next_job(&mut self) -> Option<WorkerJob> {
            self.jobs.pop_front()
        }

        fn send_report(&mut self, report: Result<WorkerExecutionReport, WorkerExecutionError>) {
            self.reports.push(report);
        }
    }

    fn service() -> WorkerRuntimeService {
        WorkerRuntimeService::new(
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
                        "iot:{event.actor.tenantId}:devices:{event.payload.deviceId}:online"
                            .into(),
                    ),
                    delay_ms: None,
                }],
                nats_url: Some("nats://nats.bladb.svc.cluster.local:4222".into()),
                worker_loop: WorkerLoopConfig::default(),
                labels: Default::default(),
            },
            "0.0.0.0:9091".into(),
            StepExecutorRegistry::new(vec![Arc::new(RedisExecutor)]),
        )
    }

    fn job(trace_id: &str) -> WorkerJob {
        WorkerJob {
            worker: "telemetry.counter-updater".into(),
            attempt: 1,
            trigger_subject: "events.iot.telemetry.received".into(),
            trigger_stream: Some("BLADB_IOT_EVENTS".into()),
            trigger_consumer: Some("iot-telemetry-counter-updater".into()),
            event: EventEnvelope {
                event_id: trace_id.into(),
                event_type: "telemetry.received".into(),
                source: "mqtt.devices.telemetry".into(),
                trace_id: trace_id.into(),
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
    fn runner_processes_all_available_jobs() {
        let runner = WorkerRuntimeRunner::new(service());
        let mut inbox = MemoryInbox {
            jobs: VecDeque::from(vec![job("evt_01"), job("evt_02")]),
            reports: vec![],
        };

        let stats = runner.run_until_empty(&mut inbox);
        assert_eq!(stats.processed, 2);
        assert_eq!(stats.succeeded, 2);
        assert_eq!(stats.failed, 0);
        assert_eq!(inbox.reports.len(), 2);
    }

    #[test]
    fn runner_can_limit_job_processing_to_one_batch() {
        let runner = WorkerRuntimeRunner::new(service());
        let mut inbox = MemoryInbox {
            jobs: VecDeque::from(vec![job("evt_01"), job("evt_02")]),
            reports: vec![],
        };

        let stats = runner.run_batch(&mut inbox, 1);
        assert_eq!(
            stats,
            WorkerRunStats {
                processed: 1,
                succeeded: 1,
                failed: 0,
            }
        );
        assert_eq!(inbox.jobs.len(), 1);
        assert_eq!(inbox.reports.len(), 1);
    }
}
