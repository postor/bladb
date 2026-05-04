use crate::{
    config::{ModuleRuntimeConfig, ModuleRuntimePlanError},
    registry::AdapterRegistry,
    runner::ModuleRuntimeRunner,
    service::ModuleRuntimeService,
    transport::ModuleTransportServer,
};
use serde_json::Value;

#[derive(Clone)]
pub struct ModuleRuntimeApp {
    service: ModuleRuntimeService,
}

impl ModuleRuntimeApp {
    pub fn from_config(
        config: ModuleRuntimeConfig,
        adapters: AdapterRegistry,
    ) -> Result<Self, ModuleRuntimePlanError> {
        let plan = config.build_plan()?;
        Ok(Self {
            service: ModuleRuntimeService::new(plan, adapters),
        })
    }

    pub fn service(&self) -> &ModuleRuntimeService {
        &self.service
    }

    pub fn runner(&self) -> ModuleRuntimeRunner {
        ModuleRuntimeRunner::new(self.service.clone())
    }

    pub fn transport_server(&self) -> ModuleTransportServer {
        ModuleTransportServer::new(self.runner(), self.service.plan().transport_loop.clone())
    }

    pub fn status_json(&self) -> Value {
        serde_json::to_value(self.service.status()).unwrap_or_else(|_| {
            serde_json::json!({
                "cluster": self.service.plan().cluster,
                "error": "failed to render module runtime status"
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ModuleRuntimeApp;
    use crate::{config::ModuleRuntimeConfig, registry::AdapterRegistry};
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn app_bootstraps_from_real_example_runtime_config() {
        let config = ModuleRuntimeConfig::from_path(
            workspace_root().join("apps/examples/flash-sale/runtime/flashsale.orders.runtime.yaml"),
        )
        .expect("load module runtime config");

        let app =
            ModuleRuntimeApp::from_config(config, AdapterRegistry::new(vec![])).expect("bootstrap module app");
        let status = app.status_json();

        assert_eq!(status["cluster"], "flashsale.orders-sql");
        assert_eq!(status["runtime"], "sql");
        assert_eq!(status["service"], "bladb-module-orders");
        assert_eq!(app.transport_server().loop_config().max_batch, 64);
    }
}
