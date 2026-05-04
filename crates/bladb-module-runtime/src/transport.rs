use crate::{
    config::TransportLoopConfig,
    runner::{ModuleRpcInbox, ModuleRunStats, ModuleRuntimeRunner},
};
use std::{thread, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleTransportTick {
    pub stats: ModuleRunStats,
    pub idle: bool,
}

pub struct ModuleTransportServer {
    runner: ModuleRuntimeRunner,
    loop_config: TransportLoopConfig,
}

impl ModuleTransportServer {
    pub fn new(runner: ModuleRuntimeRunner, loop_config: TransportLoopConfig) -> Self {
        Self {
            runner,
            loop_config,
        }
    }

    pub fn runner(&self) -> &ModuleRuntimeRunner {
        &self.runner
    }

    pub fn loop_config(&self) -> &TransportLoopConfig {
        &self.loop_config
    }

    pub fn idle_sleep(&self) -> Duration {
        Duration::from_millis(self.loop_config.idle_sleep_ms)
    }

    pub fn run_tick(&self, transport: &mut dyn ModuleRpcInbox) -> ModuleTransportTick {
        let stats = self
            .runner
            .run_batch(transport, self.loop_config.max_batch.max(1));
        ModuleTransportTick {
            idle: stats.processed == 0,
            stats,
        }
    }

    pub fn run_cycles(
        &self,
        transport: &mut dyn ModuleRpcInbox,
        cycles: usize,
    ) -> ModuleRunStats {
        let mut total = ModuleRunStats {
            processed: 0,
            succeeded: 0,
            failed: 0,
        };

        for _ in 0..cycles {
            let tick = self.run_tick(transport);
            total.processed += tick.stats.processed;
            total.succeeded += tick.stats.succeeded;
            total.failed += tick.stats.failed;
            if tick.idle {
                break;
            }
        }

        total
    }

    pub fn serve_blocking(&self, transport: &mut dyn ModuleRpcInbox) -> ! {
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
    use super::ModuleTransportServer;
    use crate::{
        config::{
            AdapterBindingConfig, ModuleRuntimePlan, NatsConnectionConfig, ServeConfig,
            TransportLoopConfig,
        },
        registry::{AdapterRegistry, ModuleAdapter, ModuleRuntimeError},
        runner::ModuleRpcInbox,
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
    use std::{collections::VecDeque, sync::Arc, time::Duration};

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
                "traceId": invocation.trace_id,
                "statement": invocation.body.statement,
            }))
        }
    }

    struct MemoryTransport {
        requests: VecDeque<ModuleRpcRequest>,
        responses: Vec<Result<ModuleRpcResponse, ModuleRuntimeError>>,
    }

    impl ModuleRpcInbox for MemoryTransport {
        fn next_request(&mut self) -> Option<ModuleRpcRequest> {
            self.requests.pop_front()
        }

        fn send_response(&mut self, response: Result<ModuleRpcResponse, ModuleRuntimeError>) {
            self.responses.push(response);
        }
    }

    fn server(max_batch: usize) -> ModuleTransportServer {
        let service = ModuleRuntimeService::new(
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
                transport_loop: TransportLoopConfig {
                    max_batch,
                    idle_sleep_ms: 15,
                },
                serve: ServeConfig::default(),
                adapter: AdapterBindingConfig {
                    endpoint: Some("postgres://orders@db/orders".into()),
                    database: Some("orders".into()),
                    runtime: Some("sql".into()),
                    options: Default::default(),
                },
            },
            AdapterRegistry::new(vec![Arc::new(SqlAdapter)]),
        );

        ModuleTransportServer::new(
            crate::runner::ModuleRuntimeRunner::new(service),
            TransportLoopConfig {
                max_batch,
                idle_sleep_ms: 15,
            },
        )
    }

    fn request(trace_id: &str) -> ModuleRpcRequest {
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
        }
    }

    #[test]
    fn transport_server_limits_each_tick_to_max_batch() {
        let server = server(1);
        let mut transport = MemoryTransport {
            requests: VecDeque::from(vec![request("trace_01"), request("trace_02")]),
            responses: vec![],
        };

        let tick = server.run_tick(&mut transport);
        assert!(!tick.idle);
        assert_eq!(tick.stats.processed, 1);
        assert_eq!(tick.stats.succeeded, 1);
        assert_eq!(transport.requests.len(), 1);
        assert_eq!(transport.responses.len(), 1);
    }

    #[test]
    fn transport_server_reports_idle_when_no_requests_are_waiting() {
        let server = server(8);
        let mut transport = MemoryTransport {
            requests: VecDeque::new(),
            responses: vec![],
        };

        let tick = server.run_tick(&mut transport);
        assert!(tick.idle);
        assert_eq!(tick.stats.processed, 0);
        assert_eq!(server.idle_sleep(), Duration::from_millis(15));
    }
}
