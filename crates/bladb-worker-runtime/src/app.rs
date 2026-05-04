use crate::{
    config::{WorkerRuntimeConfig, WorkerRuntimePlanError},
    executor::StepExecutorRegistry,
    runner::WorkerRuntimeRunner,
    service::WorkerRuntimeService,
    transport::WorkerTransportConsumer,
};
use serde_json::Value;

#[derive(Clone)]
pub struct WorkerRuntimeApp {
    service: WorkerRuntimeService,
}

impl WorkerRuntimeApp {
    pub fn from_config(
        config: WorkerRuntimeConfig,
        executors: StepExecutorRegistry,
    ) -> Result<Self, WorkerRuntimePlanError> {
        let metrics_bind_addr = config.serve.metrics_bind_addr.clone();
        let plan = config.build_plan()?;
        Ok(Self {
            service: WorkerRuntimeService::new(plan, metrics_bind_addr, executors),
        })
    }

    pub fn service(&self) -> &WorkerRuntimeService {
        &self.service
    }

    pub fn runner(&self) -> WorkerRuntimeRunner {
        WorkerRuntimeRunner::new(self.service.clone())
    }

    pub fn transport_consumer(&self) -> WorkerTransportConsumer {
        WorkerTransportConsumer::new(
            self.runner(),
            self.service.plan().worker_loop.clone(),
            self.service.plan().retry_max_attempts,
            self.service.plan().dead_letter_subject.clone(),
        )
    }

    pub fn status_json(&self) -> Value {
        serde_json::to_value(self.service.status()).unwrap_or_else(|_| {
            serde_json::json!({
                "worker": self.service.status().worker,
                "error": "failed to render worker runtime status"
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::WorkerRuntimeApp;
    use crate::{config::WorkerRuntimeConfig, executor::StepExecutorRegistry};
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn app_bootstraps_from_real_example_worker_runtime_config() {
        let config =
            WorkerRuntimeConfig::from_path(workspace_root().join(
                "apps/examples/flash-sale/runtime/order.payment-timeout-handler.runtime.yaml",
            ))
            .expect("load worker runtime config");

        let app = WorkerRuntimeApp::from_config(config, StepExecutorRegistry::new(vec![]))
            .expect("bootstrap worker app");
        let status = app.status_json();

        assert_eq!(status["worker"], "order.payment-timeout-handler");
        assert_eq!(
            status["triggerSubject"],
            "queue.flashsale.order.payment.timeout"
        );
        assert_eq!(status["metricsBindAddr"], "0.0.0.0:9091");
        assert_eq!(app.transport_consumer().loop_config().max_batch, 64);
    }
}
