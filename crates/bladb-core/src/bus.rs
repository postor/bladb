use crate::{
    cluster::ModuleCategory,
    event::EventEnvelope,
    protocol::{GatewayRequest, RequestBody},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteHint {
    pub cluster: String,
    pub category: ModuleCategory,
    pub runtime: String,
    pub service: String,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub route_key: Option<String>,
    #[serde(default)]
    pub shard: Option<u16>,
    #[serde(default)]
    pub sticky: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSnapshot {
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub permission_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleRpcRequest {
    pub trace_id: String,
    pub policy: String,
    pub route: RouteHint,
    pub auth: AuthSnapshot,
    pub request: GatewayRequest,
    pub body: RequestBody,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleRpcResponse {
    pub trace_id: String,
    pub cluster: String,
    pub runtime: String,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerJob {
    pub worker: String,
    pub attempt: u32,
    pub trigger_subject: String,
    #[serde(default)]
    pub trigger_stream: Option<String>,
    #[serde(default)]
    pub trigger_consumer: Option<String>,
    pub event: EventEnvelope,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerStepResult {
    pub step_index: usize,
    pub backend: String,
    pub action: String,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerExecutionReport {
    pub worker: String,
    pub attempt: u32,
    pub event_id: String,
    pub success: bool,
    pub results: Vec<WorkerStepResult>,
}

#[cfg(test)]
mod tests {
    use super::{
        AuthSnapshot, ModuleRpcRequest, ModuleRpcResponse, RouteHint, WorkerExecutionReport,
        WorkerJob, WorkerStepResult,
    };
    use crate::{
        cluster::ModuleCategory,
        event::{ActorContext, EventEnvelope},
        protocol::{Engine, GatewayRequest, RequestBody, RequestKind},
    };
    use serde_json::json;

    #[test]
    fn module_rpc_messages_round_trip() {
        let message = ModuleRpcRequest {
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

        let value = serde_json::to_value(&message).expect("serialize module rpc request");
        assert_eq!(value["traceId"], "trace_01");
        assert_eq!(value["route"]["cluster"], "flashsale.orders-sql");

        let round_trip: ModuleRpcRequest =
            serde_json::from_value(value).expect("deserialize module rpc request");
        assert_eq!(round_trip, message);

        let response = ModuleRpcResponse {
            trace_id: "trace_01".into(),
            cluster: "flashsale.orders-sql".into(),
            runtime: "sql".into(),
            data: json!([{ "id": "ord_01" }]),
        };
        let response_value =
            serde_json::to_value(&response).expect("serialize module rpc response");
        assert_eq!(response_value["runtime"], "sql");
    }

    #[test]
    fn worker_job_and_execution_report_round_trip() {
        let job = WorkerJob {
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
                    "temp": 36
                }),
            },
        };

        let report = WorkerExecutionReport {
            worker: "telemetry.counter-updater".into(),
            attempt: 2,
            event_id: "evt_01".into(),
            success: true,
            results: vec![WorkerStepResult {
                step_index: 0,
                backend: "redis".into(),
                action: "setOnlineState".into(),
                data: json!({ "ok": true }),
            }],
        };

        let job_value = serde_json::to_value(&job).expect("serialize worker job");
        assert_eq!(job_value["triggerSubject"], "events.iot.telemetry.received");
        let report_value = serde_json::to_value(&report).expect("serialize worker report");
        assert_eq!(report_value["results"][0]["backend"], "redis");

        let round_trip: WorkerJob =
            serde_json::from_value(job_value).expect("deserialize worker job");
        assert_eq!(round_trip, job);
    }
}
