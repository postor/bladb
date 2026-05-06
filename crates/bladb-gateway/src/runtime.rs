use crate::{AuthContext, RouteSelection, RoutedRequest};
use bladb_core::protocol::{ErrorCode, GatewayRequest};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionContext {
    pub request: GatewayRequest,
    pub auth: AuthContext,
    pub routed: RoutedRequest,
}

impl ExecutionContext {
    pub fn policy_name(&self) -> &str {
        &self.routed.authorization.policy_name
    }

    pub fn route(&self) -> &RouteSelection {
        &self.routed.route
    }
}

pub trait ModuleRuntime: Send + Sync {
    fn handles_cluster(&self, cluster: &str) -> bool;
    fn execute(&self, context: &ExecutionContext) -> Result<Value, RuntimeError>;
}

#[derive(Clone)]
pub struct RuntimeRegistry {
    modules: Vec<Arc<dyn ModuleRuntime>>,
}

impl RuntimeRegistry {
    pub fn new(modules: Vec<Arc<dyn ModuleRuntime>>) -> Self {
        Self { modules }
    }

    pub fn execute(&self, context: &ExecutionContext) -> Result<Value, RuntimeError> {
        self.modules
            .iter()
            .find(|module| module.handles_cluster(&context.route().cluster))
            .ok_or_else(|| {
                RuntimeError::internal(format!(
                    "cluster `{}` for policy `{}` is not implemented in runtime registry",
                    context.route().cluster,
                    context.policy_name()
                ))
            })?
            .execute(context)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    pub status: u16,
    pub code: ErrorCode,
    pub message: String,
}

impl RuntimeError {
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            code: ErrorCode::InvalidRequest,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            code: ErrorCode::InvalidRequest,
            message: message.into(),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: 403,
            code: ErrorCode::PolicyDenied,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            code: ErrorCode::InternalError,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExecutionContext, ModuleRuntime, RuntimeError, RuntimeRegistry};
    use crate::{AuthContext, Authorization, RouteSelection, RoutedRequest};
    use bladb_core::{
        cluster::ModuleCategory,
        protocol::{GatewayRequest, RequestBody},
    };
    use serde_json::json;
    use std::sync::Arc;

    struct TestModule;

    impl ModuleRuntime for TestModule {
        fn handles_cluster(&self, cluster: &str) -> bool {
            cluster == "flashsale.stock-redis"
        }

        fn execute(&self, context: &ExecutionContext) -> Result<serde_json::Value, RuntimeError> {
            Ok(json!({
                "cluster": context.route().cluster,
                "policy": context.policy_name(),
            }))
        }
    }

    #[test]
    fn runtime_registry_dispatches_to_matching_module() {
        let registry = RuntimeRegistry::new(vec![Arc::new(TestModule)]);
        let context = ExecutionContext {
            request: GatewayRequest {
                kind: bladb_core::protocol::RequestKind::Command,
                engine: bladb_core::protocol::Engine::Redis,
                action: "decrby".into(),
                meta: bladb_core::protocol::RequestMeta::default(),
                body: RequestBody {
                    amount: Some(1),
                    name: Some(json!("flashsale:camera-pro:stock")),
                    ..RequestBody::default()
                },
            },
            auth: AuthContext::default(),
            routed: RoutedRequest {
                authorization: Authorization {
                    policy_name: "flashsale.stock.decr".into(),
                },
                route: RouteSelection {
                    cluster: "flashsale.stock-redis".into(),
                    category: ModuleCategory::Data,
                    runtime: "redis".into(),
                    service: "stock".into(),
                    namespace: Some("local".into()),
                    route_key: Some("camera-pro".into()),
                    shard: Some(1),
                    sticky: true,
                },
                body: RequestBody {
                    amount: Some(1),
                    name: Some(json!("flashsale:camera-pro:stock")),
                    ..RequestBody::default()
                },
            },
        };

        let value = registry.execute(&context).expect("execute runtime");
        assert_eq!(value["cluster"], "flashsale.stock-redis");
        assert_eq!(value["policy"], "flashsale.stock.decr");
    }

    #[test]
    fn runtime_registry_reports_missing_cluster_handler() {
        let registry = RuntimeRegistry::new(vec![]);
        let context = ExecutionContext {
            request: GatewayRequest {
                kind: bladb_core::protocol::RequestKind::Query,
                engine: bladb_core::protocol::Engine::Sql,
                action: "select".into(),
                meta: bladb_core::protocol::RequestMeta::default(),
                body: RequestBody {
                    statement: Some("select 1".into()),
                    ..RequestBody::default()
                },
            },
            auth: AuthContext::default(),
            routed: RoutedRequest {
                authorization: Authorization {
                    policy_name: "flashsale.orders.read-mine".into(),
                },
                route: RouteSelection {
                    cluster: "flashsale.orders-sql".into(),
                    category: ModuleCategory::Data,
                    runtime: "sql".into(),
                    service: "orders".into(),
                    namespace: Some("local".into()),
                    route_key: Some("tenant_flashsale".into()),
                    shard: Some(3),
                    sticky: false,
                },
                body: RequestBody {
                    statement: Some("select 1".into()),
                    ..RequestBody::default()
                },
            },
        };

        let error = registry
            .execute(&context)
            .expect_err("expected runtime error");
        assert_eq!(error.status, 500);
        assert_eq!(error.code, bladb_core::protocol::ErrorCode::InternalError);
    }
}
