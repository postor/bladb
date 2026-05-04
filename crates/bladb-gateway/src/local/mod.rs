mod app;
mod app_api;
mod auth;
mod config;
mod flash_sale;
mod iot;

use crate::RuntimeError;
use bladb_core::protocol::ErrorCode;
use serde_json::Value;

pub use app::LocalGatewayApp;
pub(crate) use app_api::{AppApiHandler, AppApiRequest};
pub use auth::{InMemoryAuthService, InMemoryUserConfig};
pub use config::{GatewayRuntimeConfig, LocalGatewayConfig, LocalGatewayModulesConfig};
pub use flash_sale::{FlashSaleModule, FlashSaleModuleConfig};
pub use iot::{IotModule, IotModuleConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    pub status: u16,
    pub code: ErrorCode,
    pub message: String,
}

impl AppError {
    pub(crate) fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: 401,
            code: ErrorCode::AuthExpired,
            message: message.into(),
        }
    }

    pub(crate) fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            code: ErrorCode::InvalidRequest,
            message: message.into(),
        }
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            code: ErrorCode::InvalidRequest,
            message: message.into(),
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            code: ErrorCode::InternalError,
            message: message.into(),
        }
    }

    pub(crate) fn lock_runtime<T>(_: std::sync::PoisonError<T>) -> RuntimeError {
        RuntimeError::internal("shared state lock poisoned")
    }
}

impl From<RuntimeError> for AppError {
    fn from(value: RuntimeError) -> Self {
        Self {
            status: value.status,
            code: value.code,
            message: value.message,
        }
    }
}

pub(crate) fn value_as_string(
    value: Option<&Value>,
    field: &'static str,
) -> Result<String, RuntimeError> {
    value
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| RuntimeError::invalid_request(format!("{field} is missing")))
}

pub(crate) fn value_as_i64(
    value: Option<&Value>,
    field: &'static str,
) -> Result<i64, RuntimeError> {
    value
        .and_then(Value::as_i64)
        .ok_or_else(|| RuntimeError::invalid_request(format!("{field} is missing")))
}

pub(crate) fn now_label() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{}", duration.as_secs()),
        Err(_) => "0".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemoryAuthService, InMemoryUserConfig, LocalGatewayApp, LocalGatewayConfig};
    use crate::AuthContext;
    use bladb_core::protocol::GatewayRequest;
    use serde_json::json;
    use std::{path::Path, sync::Arc};

    fn example_runtime_configs() -> Vec<super::GatewayRuntimeConfig> {
        vec![
            super::GatewayRuntimeConfig {
                name: "flash-sale".into(),
                policy_yaml: include_str!(
                    "../../../../apps/examples/flash-sale/policies/flash-sale.policy.yaml"
                )
                .into(),
                topology_yaml: include_str!(
                    "../../../../apps/examples/flash-sale/topology/flash-sale.topology.yaml"
                )
                .into(),
                default_auth: AuthContext {
                    uid: Some("u_2001".into()),
                    tenant_id: Some("tenant_flashsale".into()),
                    roles: vec!["buyer".into()],
                    permission_version: Some("v1".into()),
                },
            },
            super::GatewayRuntimeConfig {
                name: "iot-realtime".into(),
                policy_yaml: include_str!(
                    "../../../../apps/examples/iot-realtime/policies/iot-realtime.policy.yaml"
                )
                .into(),
                topology_yaml: include_str!(
                    "../../../../apps/examples/iot-realtime/topology/iot-realtime.topology.yaml"
                )
                .into(),
                default_auth: AuthContext {
                    uid: Some("u_1001".into()),
                    tenant_id: Some("tenant_a".into()),
                    roles: vec!["operator".into()],
                    permission_version: Some("v1".into()),
                },
            },
        ]
    }

    fn test_auth_service() -> Arc<InMemoryAuthService> {
        Arc::new(InMemoryAuthService::from_user_configs(vec![
            InMemoryUserConfig {
                app: "flash-sale".into(),
                uid: "u_2001".into(),
                tenant_id: "tenant_flashsale".into(),
                email: "buyer@flash-sale.demo".into(),
                password: "demo123".into(),
                display_name: "Flash Buyer".into(),
                roles: vec!["buyer".into()],
            },
            InMemoryUserConfig {
                app: "iot-realtime".into(),
                uid: "u_1001".into(),
                tenant_id: "tenant_a".into(),
                email: "operator@iot.demo".into(),
                password: "demo123".into(),
                display_name: "IoT Operator".into(),
                roles: vec!["operator".into()],
            },
        ]))
    }

    #[test]
    fn local_app_handles_flash_sale_order_reads() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_auth_service())
                .expect("build local gateway app");
        let request: GatewayRequest = serde_json::from_value(json!({
            "kind": "query",
            "engine": "sql",
            "action": "select",
            "meta": {
                "policy": "flashsale.orders.read-mine",
                "resource": "orders.readMine"
            },
            "statement": "select id, status, quantity from orders where uid = ? and sku = ?",
            "values": [
                { "$ctx": "uid", "token": "UID" },
                "camera-pro"
            ]
        }))
        .expect("parse flash-sale request");

        let response = app
            .handle_execute(request)
            .expect("execute flash-sale request");
        assert!(response.as_array().is_some_and(|rows| rows.len() >= 2));
    }

    #[test]
    fn local_app_resolves_iot_command_templates() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_auth_service())
                .expect("build local gateway app");
        let request: GatewayRequest = serde_json::from_value(json!({
            "kind": "stream",
            "engine": "mqtt",
            "action": "publish",
            "meta": {
                "policy": "iot.device-command.publish",
                "resource": "device.command",
                "params": {
                    "deviceId": "device-001"
                }
            },
            "topic": {
                "$template": "key",
                "parts": ["tenant/", "/devices/", "/commands"],
                "values": [
                    { "$ctx": "tenantId", "token": "TENANT_ID" },
                    "{args.deviceId}"
                ]
            },
            "payload": {
                "action": "reboot",
                "issuedBy": { "$ctx": "uid", "token": "UID" }
            }
        }))
        .expect("parse iot request");

        let response = app.handle_execute(request).expect("execute iot request");
        assert_eq!(
            response["topic"],
            "tenant/tenant_a/devices/device-001/commands"
        );
        assert_eq!(response["issuedBy"], "u_1001");
    }

    #[test]
    fn local_app_inspects_route_details() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_auth_service())
                .expect("build local gateway app");
        let request: GatewayRequest = serde_json::from_value(json!({
            "kind": "command",
            "engine": "redis",
            "action": "decrby",
            "meta": {
                "policy": "flashsale.stock.decr",
                "params": {
                    "sku": "camera-pro"
                }
            },
            "name": {
                "$template": "key",
                "parts": ["flashsale:", ":stock"],
                "values": ["{args.sku}"]
            },
            "amount": 1
        }))
        .expect("parse route request");

        let response = app.inspect_request(request).expect("inspect route");
        assert_eq!(response["policy"], "flashsale.stock.decr");
        assert_eq!(response["route"]["cluster"], "flashsale.stock-redis");
        assert_eq!(response["route"]["service"], "bladb-module-stock");
    }

    #[test]
    fn local_app_exposes_topology_snapshot() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_auth_service())
                .expect("build local gateway app");
        let response = app.topology_snapshot();
        let gateways = response.as_array().expect("topology snapshot array");
        let flash_sale_clusters = gateways[0]["clusters"]
            .as_array()
            .expect("flash-sale clusters");
        let orders_cluster = flash_sale_clusters
            .iter()
            .find(|cluster| cluster["name"] == "flashsale.orders-sql")
            .expect("orders cluster");

        assert_eq!(gateways.len(), 2);
        assert_eq!(gateways[0]["gateway"], "flash-sale");
        assert!(gateways[0]["clusters"]
            .as_array()
            .is_some_and(|clusters| !clusters.is_empty()));
        assert_eq!(orders_cluster["transport"]["protocol"], "natsService");
        assert_eq!(
            orders_cluster["transport"]["subject"],
            "rpc.flashsale.orders"
        );
        assert_eq!(
            orders_cluster["transport"]["queueGroup"],
            "bladb.flashsale.orders"
        );
        assert_eq!(orders_cluster["deployment"]["replicas"], 2);
        assert_eq!(orders_cluster["deployment"]["minReadySeconds"], 5);
        assert_eq!(
            orders_cluster["deployment"]["rolling"]["maxUnavailable"],
            "0"
        );
        assert_eq!(orders_cluster["deployment"]["rolling"]["maxSurge"], "1");
        assert_eq!(orders_cluster["deployment"]["autoscale"]["minReplicas"], 2);
        assert_eq!(orders_cluster["deployment"]["autoscale"]["maxReplicas"], 8);
        assert_eq!(
            orders_cluster["deployment"]["autoscale"]["targetCpuUtilization"],
            70
        );
        assert_eq!(orders_cluster["routing"]["strategy"]["kind"], "hash");
        assert_eq!(orders_cluster["routing"]["strategy"]["virtualShards"], 64);
        assert_eq!(orders_cluster["routing"]["routeBy"][0], "actor.tenantId");
        assert_eq!(orders_cluster["routing"]["sticky"], false);
    }

    #[test]
    fn local_gateway_config_loads_from_example_file() {
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/examples/gateway/local-gateway.yaml");
        let config = LocalGatewayConfig::from_path(&config_path).expect("load gateway config");
        let app = LocalGatewayApp::from_local_config(config).expect("build app from config");

        let topology = app.topology_snapshot();
        assert_eq!(topology.as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn local_app_dispatches_flash_sale_queue_api() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_auth_service())
                .expect("build local gateway app");
        let login = app
            .login("flash-sale", "buyer@flash-sale.demo", "demo123")
            .expect("login");
        let token = login["token"].as_str().expect("session token");
        let response = app
            .handle_app_api(
                "POST",
                "/apps/flash-sale/queue",
                Some(token),
                Some(json!({ "sku": "camera-pro", "quantity": 1 })),
            )
            .expect("queue api result")
            .expect("queue api payload");

        assert_eq!(response["sku"], "camera-pro");
        assert!(response["ticketId"].as_str().is_some());
    }

    #[test]
    fn local_app_dispatches_flash_sale_summary_api() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_auth_service())
                .expect("build local gateway app");
        let login = app
            .login("flash-sale", "buyer@flash-sale.demo", "demo123")
            .expect("login");
        let token = login["token"].as_str().expect("session token");
        let response = app
            .handle_app_api("GET", "/apps/flash-sale/summary", Some(token), None)
            .expect("summary result")
            .expect("summary payload");

        assert_eq!(response["item"]["title"], "Camera Pro");
        assert_eq!(response["item"]["sku"], "camera-pro");
        assert_eq!(response["stock"], 420);
        assert_eq!(response["wallet"], 1280);
        assert!(response["orders"]
            .as_array()
            .is_some_and(|orders| !orders.is_empty()));
    }

    #[test]
    fn local_app_dispatches_iot_command_history_api() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_auth_service())
                .expect("build local gateway app");
        let login = app
            .login("iot-realtime", "operator@iot.demo", "demo123")
            .expect("login");
        let token = login["token"].as_str().expect("session token");
        let request: GatewayRequest = serde_json::from_value(json!({
            "kind": "stream",
            "engine": "mqtt",
            "action": "publish",
            "meta": {
                "policy": "iot.device-command.publish",
                "resource": "device.command",
                "params": {
                    "deviceId": "device-001"
                }
            },
            "topic": {
                "$template": "key",
                "parts": ["tenant/", "/devices/", "/commands"],
                "values": [
                    { "$ctx": "tenantId", "token": "TENANT_ID" },
                    "{args.deviceId}"
                ]
            },
            "payload": {
                "action": "reboot",
                "issuedBy": { "$ctx": "uid", "token": "UID" }
            }
        }))
        .expect("parse iot request");

        app.handle_execute_for_token(request, Some(token))
            .expect("publish command");
        let response = app
            .handle_app_api("GET", "/apps/iot-realtime/commands", Some(token), None)
            .expect("command history result")
            .expect("command history payload");
        let history = response.as_array().expect("command history array");

        assert!(!history.is_empty());
        assert_eq!(history[0]["deviceId"], "device-001");
    }

    #[test]
    fn local_app_dispatches_iot_command_publish_api() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_auth_service())
                .expect("build local gateway app");
        let login = app
            .login("iot-realtime", "operator@iot.demo", "demo123")
            .expect("login");
        let token = login["token"].as_str().expect("session token");

        let response = app
            .handle_app_api(
                "POST",
                "/apps/iot-realtime/commands",
                Some(token),
                Some(json!({
                    "deviceId": "device-001",
                    "action": "reboot"
                })),
            )
            .expect("publish command result")
            .expect("publish command payload");

        assert_eq!(response["published"], true);
        assert_eq!(response["deviceId"], "device-001");
        assert_eq!(response["action"], "reboot");
        assert_eq!(response["issuedBy"], "u_1001");
        assert_eq!(
            response["topic"],
            "tenant/tenant_a/devices/device-001/commands"
        );
    }
}
