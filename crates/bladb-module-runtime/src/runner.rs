use crate::{service::ModuleRuntimeService, ModuleRuntimeError};
use bladb_core::bus::{ModuleRpcRequest, ModuleRpcResponse};

pub trait ModuleRpcInbox {
    fn next_request(&mut self) -> Option<ModuleRpcRequest>;
    fn send_response(&mut self, response: Result<ModuleRpcResponse, ModuleRuntimeError>);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRunStats {
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
}

pub struct ModuleRuntimeRunner {
    service: ModuleRuntimeService,
}

impl ModuleRuntimeRunner {
    pub fn new(service: ModuleRuntimeService) -> Self {
        Self { service }
    }

    pub fn service(&self) -> &ModuleRuntimeService {
        &self.service
    }

    pub fn run_one(&self, inbox: &mut dyn ModuleRpcInbox) -> bool {
        match inbox.next_request() {
            Some(request) => {
                let response = self.service.handle_rpc(request);
                inbox.send_response(response);
                true
            }
            None => false,
        }
    }

    pub fn run_batch(&self, inbox: &mut dyn ModuleRpcInbox, max_batch: usize) -> ModuleRunStats {
        let mut stats = ModuleRunStats {
            processed: 0,
            succeeded: 0,
            failed: 0,
        };

        while stats.processed < max_batch {
            let Some(request) = inbox.next_request() else {
                break;
            };
            let response = self.service.handle_rpc(request);
            stats.processed += 1;
            if response.is_ok() {
                stats.succeeded += 1;
            } else {
                stats.failed += 1;
            }
            inbox.send_response(response);
        }

        stats
    }

    pub fn run_until_empty(&self, inbox: &mut dyn ModuleRpcInbox) -> ModuleRunStats {
        self.run_batch(inbox, usize::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::{ModuleRpcInbox, ModuleRuntimeRunner};
    use crate::{
        config::{
            AdapterBindingConfig, ModuleRuntimePlan, NatsConnectionConfig, ServeConfig,
            TransportLoopConfig,
        },
        registry::{AdapterRegistry, ModuleAdapter, ModuleRuntimeError},
        service::ModuleRuntimeService,
    };
    use bladb_core::{
        bus::{AuthSnapshot, ModuleRpcRequest, ModuleRpcResponse, RouteHint},
        cluster::{
            DeploymentConfig, DiscoveryConfig, DiscoveryKind, ModuleCategory, TransportConfig,
            TransportProtocol,
        },
        protocol::{Engine, GatewayRequest, RequestBody, RequestKind},
    };
    use serde_json::json;
    use std::collections::VecDeque;
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
                "cluster": invocation.plan.cluster,
                "traceId": invocation.trace_id,
            }))
        }
    }

    struct MemoryInbox {
        requests: VecDeque<ModuleRpcRequest>,
        responses: Vec<Result<ModuleRpcResponse, ModuleRuntimeError>>,
    }

    impl ModuleRpcInbox for MemoryInbox {
        fn next_request(&mut self) -> Option<ModuleRpcRequest> {
            self.requests.pop_front()
        }

        fn send_response(&mut self, response: Result<ModuleRpcResponse, ModuleRuntimeError>) {
            self.responses.push(response);
        }
    }

    fn service() -> ModuleRuntimeService {
        ModuleRuntimeService::new(
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
                deployment: DeploymentConfig::default(),
                nats: NatsConnectionConfig {
                    url: Some("nats://nats.bladb.svc.cluster.local:4222".into()),
                },
                transport_loop: TransportLoopConfig::default(),
                serve: ServeConfig::default(),
                adapter: AdapterBindingConfig {
                    endpoint: Some("postgres://orders@db/orders".into()),
                    database: Some("orders".into()),
                    runtime: Some("sql".into()),
                    options: Default::default(),
                },
            },
            AdapterRegistry::new(vec![Arc::new(SqlAdapter)]),
        )
    }

    fn request(trace_id: &str, statement: &str) -> ModuleRpcRequest {
        ModuleRpcRequest {
            trace_id: trace_id.into(),
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
                    statement: Some(statement.into()),
                    values: vec![json!("u_2001")],
                    ..Default::default()
                },
            },
            body: RequestBody {
                statement: Some(statement.into()),
                values: vec![json!("u_2001")],
                ..Default::default()
            },
        }
    }

    #[test]
    fn runner_processes_all_available_rpc_requests() {
        let runner = ModuleRuntimeRunner::new(service());
        let mut inbox = MemoryInbox {
            requests: VecDeque::from(vec![
                request("trace_01", "select * from orders where uid = ?"),
                request("trace_02", "select * from orders where uid = ?"),
            ]),
            responses: vec![],
        };

        let stats = runner.run_until_empty(&mut inbox);
        assert_eq!(stats.processed, 2);
        assert_eq!(stats.succeeded, 2);
        assert_eq!(stats.failed, 0);
        assert_eq!(inbox.responses.len(), 2);
    }

    #[test]
    fn runner_can_limit_processing_to_one_batch() {
        let runner = ModuleRuntimeRunner::new(service());
        let mut inbox = MemoryInbox {
            requests: VecDeque::from(vec![
                request("trace_01", "select * from orders where uid = ?"),
                request("trace_02", "select * from orders where uid = ?"),
            ]),
            responses: vec![],
        };

        let stats = runner.run_batch(&mut inbox, 1);
        assert_eq!(
            stats,
            ModuleRunStats {
                processed: 1,
                succeeded: 1,
                failed: 0,
            }
        );
        assert_eq!(inbox.requests.len(), 1);
        assert_eq!(inbox.responses.len(), 1);
    }
}
