use crate::config::ModuleRuntimePlan;
use bladb_core::protocol::{GatewayRequest, RequestBody};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleInvocation {
    pub trace_id: String,
    pub policy: String,
    pub plan: ModuleRuntimePlan,
    pub request: GatewayRequest,
    pub body: RequestBody,
}

pub trait ModuleAdapter: Send + Sync {
    fn handles_runtime(&self, runtime: &str) -> bool;
    fn execute(&self, invocation: &ModuleInvocation) -> Result<Value, ModuleRuntimeError>;
}

#[derive(Clone)]
pub struct AdapterRegistry {
    adapters: Vec<Arc<dyn ModuleAdapter>>,
}

impl AdapterRegistry {
    pub fn new(adapters: Vec<Arc<dyn ModuleAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn execute(&self, invocation: &ModuleInvocation) -> Result<Value, ModuleRuntimeError> {
        self.adapters
            .iter()
            .find(|adapter| adapter.handles_runtime(&invocation.plan.runtime))
            .ok_or_else(|| {
                ModuleRuntimeError::internal(format!(
                    "no module adapter registered for runtime `{}`",
                    invocation.plan.runtime
                ))
            })?
            .execute(invocation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRuntimeError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl ModuleRuntimeError {
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
}

#[cfg(test)]
mod tests {
    use super::{AdapterRegistry, ModuleAdapter, ModuleInvocation, ModuleRuntimeError};
    use crate::config::{
        AdapterBindingConfig, ModuleRuntimePlan, NatsConnectionConfig, ServeConfig,
        TransportLoopConfig,
    };
    use bladb_core::{
        cluster::{
            DeploymentConfig, DiscoveryConfig, DiscoveryKind, ModuleCategory, TransportConfig,
        },
        protocol::{Engine, GatewayRequest, RequestBody, RequestKind},
    };
    use serde_json::json;
    use std::sync::Arc;

    struct SqlAdapter;

    impl ModuleAdapter for SqlAdapter {
        fn handles_runtime(&self, runtime: &str) -> bool {
            runtime == "sql"
        }

        fn execute(
            &self,
            invocation: &ModuleInvocation,
        ) -> Result<serde_json::Value, ModuleRuntimeError> {
            Ok(json!({
                "cluster": invocation.plan.cluster,
                "runtime": invocation.plan.runtime,
                "action": invocation.request.action,
            }))
        }
    }

    fn module_plan(runtime: &str) -> ModuleRuntimePlan {
        ModuleRuntimePlan {
            cluster: "flashsale.orders-sql".into(),
            category: ModuleCategory::Data,
            runtime: runtime.into(),
            discovery: DiscoveryConfig {
                kind: DiscoveryKind::Service,
                service: "bladb-module-orders".into(),
                namespace: Some("bladb".into()),
            },
            transport: TransportConfig::default(),
            deployment: DeploymentConfig::default(),
            nats: NatsConnectionConfig::default(),
            transport_loop: TransportLoopConfig::default(),
            serve: ServeConfig::default(),
            adapter: AdapterBindingConfig::default(),
        }
    }

    #[test]
    fn adapter_registry_dispatches_to_runtime_specific_adapter() {
        let registry = AdapterRegistry::new(vec![Arc::new(SqlAdapter)]);
        let invocation = ModuleInvocation {
            trace_id: "trace_01".into(),
            policy: "flashsale.orders.read-mine".into(),
            plan: module_plan("sql"),
            request: GatewayRequest {
                kind: RequestKind::Query,
                engine: Engine::Sql,
                action: "select".into(),
                meta: Default::default(),
                body: RequestBody {
                    statement: Some("select * from orders where uid = ?".into()),
                    values: vec![json!("u_2001")],
                    ..Default::default()
                },
            },
            body: RequestBody {
                statement: Some("select * from orders where uid = ?".into()),
                values: vec![json!("u_2001")],
                ..Default::default()
            },
        };

        let value = registry
            .execute(&invocation)
            .expect("execute through adapter");
        assert_eq!(value["cluster"], "flashsale.orders-sql");
        assert_eq!(value["runtime"], "sql");
        assert_eq!(value["action"], "select");
    }

    #[test]
    fn adapter_registry_reports_missing_runtime_adapter() {
        let registry = AdapterRegistry::new(vec![Arc::new(SqlAdapter)]);
        let invocation = ModuleInvocation {
            trace_id: "trace_02".into(),
            policy: "flashsale.stock.decr".into(),
            plan: module_plan("redis"),
            request: GatewayRequest {
                kind: RequestKind::Command,
                engine: Engine::Redis,
                action: "decrby".into(),
                meta: Default::default(),
                body: RequestBody {
                    name: Some(json!("flashsale:camera-pro:stock")),
                    amount: Some(1),
                    ..Default::default()
                },
            },
            body: RequestBody {
                name: Some(json!("flashsale:camera-pro:stock")),
                amount: Some(1),
                ..Default::default()
            },
        };

        let error = registry
            .execute(&invocation)
            .expect_err("expected missing adapter");
        assert_eq!(
            error,
            ModuleRuntimeError::internal("no module adapter registered for runtime `redis`")
        );
    }
}
