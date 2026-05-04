use crate::{
    config::ModuleRuntimePlan,
    registry::{AdapterRegistry, ModuleInvocation, ModuleRuntimeError},
};
use bladb_core::bus::{ModuleRpcRequest, ModuleRpcResponse};
use bladb_core::cluster::TransportProtocol;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleRuntimeStatus {
    pub cluster: String,
    pub category: String,
    pub runtime: String,
    pub service: String,
    pub namespace: Option<String>,
    pub bind_addr: String,
    pub transport_protocol: String,
    pub transport_subject: Option<String>,
    pub transport_queue_group: Option<String>,
    pub transport_stream: Option<String>,
    pub loop_max_batch: usize,
    pub loop_idle_sleep_ms: u64,
    pub replicas: u16,
    pub min_ready_seconds: Option<u32>,
    pub nats_url: Option<String>,
}

#[derive(Clone)]
pub struct ModuleRuntimeService {
    plan: ModuleRuntimePlan,
    adapters: AdapterRegistry,
}

impl ModuleRuntimeService {
    pub fn new(plan: ModuleRuntimePlan, adapters: AdapterRegistry) -> Self {
        Self { plan, adapters }
    }

    pub fn plan(&self) -> &ModuleRuntimePlan {
        &self.plan
    }

    pub fn status(&self) -> ModuleRuntimeStatus {
        ModuleRuntimeStatus {
            cluster: self.plan.cluster.clone(),
            category: format!("{:?}", self.plan.category).to_ascii_lowercase(),
            runtime: self.plan.runtime.clone(),
            service: self.plan.discovery.service.clone(),
            namespace: self.plan.discovery.namespace.clone(),
            bind_addr: self.plan.serve.bind_addr.clone(),
            transport_protocol: transport_protocol_label(&self.plan.transport.protocol).into(),
            transport_subject: self.plan.transport.subject.clone(),
            transport_queue_group: self.plan.transport.queue_group.clone(),
            transport_stream: self.plan.transport.stream.clone(),
            loop_max_batch: self.plan.transport_loop.max_batch,
            loop_idle_sleep_ms: self.plan.transport_loop.idle_sleep_ms,
            replicas: self.plan.deployment.replicas,
            min_ready_seconds: self.plan.deployment.min_ready_seconds,
            nats_url: self.plan.nats.url.clone(),
        }
    }

    pub fn handle_rpc(
        &self,
        request: ModuleRpcRequest,
    ) -> Result<ModuleRpcResponse, ModuleRuntimeError> {
        request
            .request
            .validate()
            .map_err(|error| ModuleRuntimeError::invalid_request(error.to_string()))?;

        if request.route.cluster != self.plan.cluster {
            return Err(ModuleRuntimeError::invalid_request(format!(
                "rpc request targeted cluster `{}` but runtime serves `{}`",
                request.route.cluster, self.plan.cluster
            )));
        }

        if request.route.runtime != self.plan.runtime {
            return Err(ModuleRuntimeError::invalid_request(format!(
                "rpc request runtime `{}` does not match module runtime `{}`",
                request.route.runtime, self.plan.runtime
            )));
        }

        validate_prepared_contract(&request)?;

        let trace_id = request.trace_id.clone();
        let policy = request.policy.clone();
        let gateway_request = request.request.clone();
        let body = request.body.clone();

        let data = self.adapters.execute(&ModuleInvocation {
            trace_id: trace_id.clone(),
            policy: policy.clone(),
            plan: self.plan.clone(),
            request: gateway_request,
            body,
        })?;

        Ok(ModuleRpcResponse {
            trace_id,
            cluster: self.plan.cluster.clone(),
            runtime: self.plan.runtime.clone(),
            data,
        })
    }

    pub fn healthcheck(&self) -> Value {
        serde_json::json!({
            "ok": true,
            "runtime": self.status(),
        })
    }
}

fn validate_prepared_contract(request: &ModuleRpcRequest) -> Result<(), ModuleRuntimeError> {
    let original = &request.request.body;
    let prepared = &request.body;

    if original.statement != prepared.statement
        || original.collection != prepared.collection
        || original.queue != prepared.queue
        || original.delay_ms != prepared.delay_ms
    {
        return Err(ModuleRuntimeError::invalid_request(
            "rpc request body does not match prepared request body contract",
        ));
    }

    Ok(())
}

fn transport_protocol_label(protocol: &TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::Direct => "direct",
        TransportProtocol::NatsService => "natsService",
        TransportProtocol::JetStream => "jetStream",
    }
}

#[cfg(test)]
mod tests {
    use super::ModuleRuntimeService;
    use crate::{
        config::{AdapterBindingConfig, ModuleRuntimePlan, NatsConnectionConfig, ServeConfig},
        registry::{AdapterRegistry, ModuleAdapter, ModuleRuntimeError},
    };
    use bladb_core::{
        bus::{AuthSnapshot, ModuleRpcRequest, RouteHint},
        cluster::{
            DeploymentConfig, DiscoveryConfig, DiscoveryKind, ModuleCategory, TransportConfig,
            TransportProtocol,
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
            invocation: &crate::registry::ModuleInvocation,
        ) -> Result<serde_json::Value, ModuleRuntimeError> {
            Ok(json!({
                "statement": invocation.body.statement,
                "cluster": invocation.plan.cluster,
                "policy": invocation.policy,
            }))
        }
    }

    fn plan() -> ModuleRuntimePlan {
        ModuleRuntimePlan {
            cluster: "flashsale.orders-sql".into(),
            category: ModuleCategory::Data,
            runtime: "sql".into(),
            discovery: DiscoveryConfig {
                kind: DiscoveryKind::Service,
                service: "bladb-module-orders".into(),
                namespace: Some("bladb".into()),
            },
            transport: TransportConfig {
                protocol: TransportProtocol::NatsService,
                subject: Some("rpc.flashsale.orders".into()),
                queue_group: Some("bladb.flashsale.orders".into()),
                stream: None,
                durable: None,
            },
            deployment: DeploymentConfig {
                replicas: 2,
                min_ready_seconds: Some(5),
                rolling: Default::default(),
                autoscale: None,
            },
            nats: NatsConnectionConfig {
                url: Some("nats://nats.bladb.svc.cluster.local:4222".into()),
            },
            transport_loop: crate::config::TransportLoopConfig {
                max_batch: 32,
                idle_sleep_ms: 10,
            },
            serve: ServeConfig {
                bind_addr: "0.0.0.0:9000".into(),
            },
            adapter: AdapterBindingConfig {
                endpoint: Some("postgres://orders@db/orders".into()),
                database: Some("orders".into()),
                runtime: Some("sql".into()),
                options: Default::default(),
            },
        }
    }

    #[test]
    fn service_status_includes_transport_and_deployment_shape() {
        let service = ModuleRuntimeService::new(plan(), AdapterRegistry::new(vec![]));
        let status = service.status();

        assert_eq!(status.cluster, "flashsale.orders-sql");
        assert_eq!(status.runtime, "sql");
        assert_eq!(status.transport_protocol, "natsService");
        assert_eq!(
            status.transport_subject.as_deref(),
            Some("rpc.flashsale.orders")
        );
        assert_eq!(
            status.transport_queue_group.as_deref(),
            Some("bladb.flashsale.orders")
        );
        assert_eq!(status.loop_max_batch, 32);
        assert_eq!(status.loop_idle_sleep_ms, 10);
        assert_eq!(status.replicas, 2);
    }

    #[test]
    fn service_handles_matching_rpc_request() {
        let service =
            ModuleRuntimeService::new(plan(), AdapterRegistry::new(vec![Arc::new(SqlAdapter)]));
        let request = ModuleRpcRequest {
            trace_id: "trace_01".into(),
            policy: "flashsale.orders.read-mine".into(),
            route: RouteHint {
                cluster: "flashsale.orders-sql".into(),
                category: ModuleCategory::Data,
                runtime: "sql".into(),
                service: "bladb-module-orders".into(),
                namespace: Some("bladb".into()),
                route_key: Some("tenant_flashsale".into()),
                shard: Some(7),
                sticky: false,
            },
            auth: AuthSnapshot {
                uid: Some("u_2001".into()),
                tenant_id: Some("tenant_flashsale".into()),
                roles: vec!["buyer".into()],
                permission_version: Some("v1".into()),
            },
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

        let response = service.handle_rpc(request).expect("handle rpc request");
        assert_eq!(response.cluster, "flashsale.orders-sql");
        assert_eq!(response.runtime, "sql");
        assert_eq!(response.data["cluster"], "flashsale.orders-sql");
        assert_eq!(response.data["policy"], "flashsale.orders.read-mine");
    }

    #[test]
    fn service_rejects_wrong_cluster_requests() {
        let service =
            ModuleRuntimeService::new(plan(), AdapterRegistry::new(vec![Arc::new(SqlAdapter)]));
        let request = ModuleRpcRequest {
            trace_id: "trace_01".into(),
            policy: "flashsale.orders.read-mine".into(),
            route: RouteHint {
                cluster: "flashsale.stock-redis".into(),
                category: ModuleCategory::Data,
                runtime: "redis".into(),
                service: "bladb-module-stock".into(),
                namespace: Some("bladb".into()),
                route_key: Some("camera-pro".into()),
                shard: Some(1),
                sticky: true,
            },
            auth: AuthSnapshot {
                uid: Some("u_2001".into()),
                tenant_id: Some("tenant_flashsale".into()),
                roles: vec!["buyer".into()],
                permission_version: Some("v1".into()),
            },
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

        let error = service
            .handle_rpc(request)
            .expect_err("expected cluster mismatch");
        assert_eq!(
            error,
            ModuleRuntimeError::invalid_request(
                "rpc request targeted cluster `flashsale.stock-redis` but runtime serves `flashsale.orders-sql`"
            )
        );
    }

    #[test]
    fn service_rejects_mismatched_prepared_body() {
        let service =
            ModuleRuntimeService::new(plan(), AdapterRegistry::new(vec![Arc::new(SqlAdapter)]));
        let request = ModuleRpcRequest {
            trace_id: "trace_01".into(),
            policy: "flashsale.orders.read-mine".into(),
            route: RouteHint {
                cluster: "flashsale.orders-sql".into(),
                category: ModuleCategory::Data,
                runtime: "sql".into(),
                service: "bladb-module-orders".into(),
                namespace: Some("bladb".into()),
                route_key: Some("tenant_flashsale".into()),
                shard: Some(7),
                sticky: false,
            },
            auth: AuthSnapshot {
                uid: Some("u_2001".into()),
                tenant_id: Some("tenant_flashsale".into()),
                roles: vec!["buyer".into()],
                permission_version: Some("v1".into()),
            },
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
                statement: Some("select * from orders where uid = ? and tenant_id = ?".into()),
                values: vec![json!("u_2001"), json!("tenant_flashsale")],
                ..Default::default()
            },
        };

        let error = service
            .handle_rpc(request)
            .expect_err("expected mismatched prepared body");
        assert_eq!(
            error,
            ModuleRuntimeError::invalid_request(
                "rpc request body does not match prepared request body contract"
            )
        );
    }
}
