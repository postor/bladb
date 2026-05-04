use crate::{
    config::WorkerLoopConfig,
    runner::WorkerRuntimeRunner,
    WorkerExecutionError,
};
use bladb_core::bus::{WorkerExecutionReport, WorkerJob};
use std::{thread, time::Duration};

pub trait WorkerRuntimeTransport {
    fn next_job(&mut self) -> Option<WorkerJob>;
    fn publish_report(&mut self, report: Result<WorkerExecutionReport, WorkerExecutionError>);
    fn ack(&mut self, job: &WorkerJob);
    fn retry(&mut self, job: &WorkerJob, error: &WorkerExecutionError);
    fn dead_letter(&mut self, job: &WorkerJob, error: &WorkerExecutionError);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerTransportTick {
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub acked: usize,
    pub retried: usize,
    pub dead_lettered: usize,
    pub idle: bool,
}

pub struct WorkerTransportConsumer {
    runner: WorkerRuntimeRunner,
    loop_config: WorkerLoopConfig,
    retry_max_attempts: u32,
    dead_letter_subject: Option<String>,
}

impl WorkerTransportConsumer {
    pub fn new(
        runner: WorkerRuntimeRunner,
        loop_config: WorkerLoopConfig,
        retry_max_attempts: u32,
        dead_letter_subject: Option<String>,
    ) -> Self {
        Self {
            runner,
            loop_config,
            retry_max_attempts,
            dead_letter_subject,
        }
    }

    pub fn runner(&self) -> &WorkerRuntimeRunner {
        &self.runner
    }

    pub fn loop_config(&self) -> &WorkerLoopConfig {
        &self.loop_config
    }

    pub fn idle_sleep(&self) -> Duration {
        Duration::from_millis(self.loop_config.idle_sleep_ms)
    }

    pub fn run_tick(&self, transport: &mut dyn WorkerRuntimeTransport) -> WorkerTransportTick {
        let mut tick = WorkerTransportTick {
            processed: 0,
            succeeded: 0,
            failed: 0,
            acked: 0,
            retried: 0,
            dead_lettered: 0,
            idle: false,
        };

        while tick.processed < self.loop_config.max_batch.max(1) {
            let Some(job) = transport.next_job() else {
                break;
            };
            tick.processed += 1;

            match self.runner.execute_job(job.clone()) {
                Ok(report) => {
                    tick.succeeded += 1;
                    transport.publish_report(Ok(report));
                    transport.ack(&job);
                    tick.acked += 1;
                }
                Err(error) => {
                    tick.failed += 1;
                    transport.publish_report(Err(error.clone()));
                    if job.attempt >= self.retry_max_attempts {
                        if self.dead_letter_subject.is_some() {
                            transport.dead_letter(&job, &error);
                            tick.dead_lettered += 1;
                        } else {
                            transport.ack(&job);
                            tick.acked += 1;
                        }
                    } else {
                        transport.retry(&job, &error);
                        tick.retried += 1;
                    }
                }
            }
        }

        tick.idle = tick.processed == 0;
        tick
    }

    pub fn run_cycles(
        &self,
        transport: &mut dyn WorkerRuntimeTransport,
        cycles: usize,
    ) -> WorkerTransportTick {
        let mut total = WorkerTransportTick {
            processed: 0,
            succeeded: 0,
            failed: 0,
            acked: 0,
            retried: 0,
            dead_lettered: 0,
            idle: false,
        };

        for _ in 0..cycles {
            let tick = self.run_tick(transport);
            total.processed += tick.processed;
            total.succeeded += tick.succeeded;
            total.failed += tick.failed;
            total.acked += tick.acked;
            total.retried += tick.retried;
            total.dead_lettered += tick.dead_lettered;
            if tick.idle {
                total.idle = true;
                break;
            }
        }

        total
    }

    pub fn serve_blocking(&self, transport: &mut dyn WorkerRuntimeTransport) -> ! {
        loop {
            let tick = self.run_tick(transport);
            if tick.idle {
                thread::sleep(self.idle_sleep());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkerRuntimeTransport, WorkerTransportConsumer};
    use crate::{
        config::{CompiledWorkerPlan, WorkerLoopConfig, WorkerSubscriptionPlan},
        executor::{StepExecutor, StepExecutorRegistry, WorkerExecutionError},
        runner::WorkerRuntimeRunner,
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
    use std::{collections::VecDeque, time::Duration, sync::Arc};

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

    struct FailingRedisExecutor;

    impl StepExecutor for FailingRedisExecutor {
        fn handles_backend(&self, backend: &str) -> bool {
            backend == "redis"
        }

        fn execute(
            &self,
            _invocation: &crate::executor::StepInvocation,
        ) -> Result<serde_json::Value, WorkerExecutionError> {
            Err(WorkerExecutionError::execution(
                "redis",
                "setOnlineState",
                "redis backend unavailable",
            ))
        }
    }

    #[derive(Default)]
    struct MemoryTransport {
        jobs: VecDeque<WorkerJob>,
        reports: Vec<Result<WorkerExecutionReport, WorkerExecutionError>>,
        acked: Vec<String>,
        retried: Vec<String>,
        dead_lettered: Vec<String>,
    }

    impl WorkerRuntimeTransport for MemoryTransport {
        fn next_job(&mut self) -> Option<WorkerJob> {
            self.jobs.pop_front()
        }

        fn publish_report(&mut self, report: Result<WorkerExecutionReport, WorkerExecutionError>) {
            self.reports.push(report);
        }

        fn ack(&mut self, job: &WorkerJob) {
            self.acked.push(job.event.event_id.clone());
        }

        fn retry(&mut self, job: &WorkerJob, _error: &WorkerExecutionError) {
            self.retried.push(job.event.event_id.clone());
        }

        fn dead_letter(&mut self, job: &WorkerJob, _error: &WorkerExecutionError) {
            self.dead_lettered.push(job.event.event_id.clone());
        }
    }

    fn plan(dead_letter_subject: Option<&str>) -> CompiledWorkerPlan {
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
            retry_max_attempts: 3,
            retry_backoff: "exponential".into(),
            dead_letter_subject: dead_letter_subject.map(str::to_string),
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
            worker_loop: WorkerLoopConfig {
                max_batch: 2,
                idle_sleep_ms: 12,
            },
            labels: Default::default(),
        }
    }

    fn consumer(
        executor: Arc<dyn StepExecutor>,
        dead_letter_subject: Option<&str>,
    ) -> WorkerTransportConsumer {
        let service = WorkerRuntimeService::new(
            plan(dead_letter_subject),
            "0.0.0.0:9091".into(),
            StepExecutorRegistry::new(vec![executor]),
        );
        let runner = WorkerRuntimeRunner::new(service);
        WorkerTransportConsumer::new(
            runner,
            WorkerLoopConfig {
                max_batch: 2,
                idle_sleep_ms: 12,
            },
            3,
            dead_letter_subject.map(str::to_string),
        )
    }

    fn job(event_id: &str, attempt: u32) -> WorkerJob {
        WorkerJob {
            worker: "telemetry.counter-updater".into(),
            attempt,
            trigger_subject: "events.iot.telemetry.received".into(),
            trigger_stream: Some("BLADB_IOT_EVENTS".into()),
            trigger_consumer: Some("iot-telemetry-counter-updater".into()),
            event: EventEnvelope {
                event_id: event_id.into(),
                event_type: "telemetry.received".into(),
                source: "mqtt.devices.telemetry".into(),
                trace_id: format!("trace_{event_id}"),
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
    fn consumer_acks_successful_jobs_and_publishes_reports() {
        let consumer = consumer(Arc::new(RedisExecutor), Some("dlq.iot.telemetry"));
        let mut transport = MemoryTransport {
            jobs: VecDeque::from(vec![job("evt_01", 1)]),
            ..Default::default()
        };

        let tick = consumer.run_tick(&mut transport);
        assert_eq!(tick.processed, 1);
        assert_eq!(tick.succeeded, 1);
        assert_eq!(tick.acked, 1);
        assert!(transport.retried.is_empty());
        assert!(transport.dead_lettered.is_empty());
        assert_eq!(transport.reports.len(), 1);
        assert!(transport.reports[0].is_ok());
    }

    #[test]
    fn consumer_retries_failures_below_max_attempts() {
        let consumer = consumer(Arc::new(FailingRedisExecutor), Some("dlq.iot.telemetry"));
        let mut transport = MemoryTransport {
            jobs: VecDeque::from(vec![job("evt_02", 1)]),
            ..Default::default()
        };

        let tick = consumer.run_tick(&mut transport);
        assert_eq!(tick.failed, 1);
        assert_eq!(tick.retried, 1);
        assert!(transport.acked.is_empty());
        assert!(transport.dead_lettered.is_empty());
        assert_eq!(transport.reports.len(), 1);
        assert!(transport.reports[0].is_err());
    }

    #[test]
    fn consumer_dead_letters_terminal_failures_when_dlq_is_configured() {
        let consumer = consumer(Arc::new(FailingRedisExecutor), Some("dlq.iot.telemetry"));
        let mut transport = MemoryTransport {
            jobs: VecDeque::from(vec![job("evt_03", 3)]),
            ..Default::default()
        };

        let tick = consumer.run_tick(&mut transport);
        assert_eq!(tick.failed, 1);
        assert_eq!(tick.dead_lettered, 1);
        assert!(transport.acked.is_empty());
        assert!(transport.retried.is_empty());
        assert_eq!(transport.dead_lettered, vec!["evt_03".to_string()]);
    }

    #[test]
    fn consumer_reports_idle_when_no_jobs_are_waiting() {
        let consumer = consumer(Arc::new(RedisExecutor), None);
        let mut transport = MemoryTransport::default();

        let tick = consumer.run_tick(&mut transport);
        assert!(tick.idle);
        assert_eq!(tick.processed, 0);
        assert_eq!(consumer.idle_sleep(), Duration::from_millis(12));
    }
}
