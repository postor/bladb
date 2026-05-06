mod app;
mod app_api;
mod auth;
mod blog;
mod config;
mod flash_sale;
mod iot;
mod ros2;
mod user;

use crate::RuntimeError;
use bladb_core::protocol::ErrorCode;
use serde_json::Value;

pub use app::{GatewayHttpResponse, LocalGatewayApp};
pub(crate) use app_api::{AppApiHandler, AppApiRequest};
pub use auth::{InMemoryAuthService, InMemoryUserConfig};
pub(crate) use config::LocalGatewayFileConfig;
pub use config::{
    GatewayRuntimeConfig, LocalGatewayConfig, LocalGatewayModulesConfig, OfficialModulesConfig,
    OfficialUsersModuleConfig,
};
pub use blog::{BlogModule, BlogModuleConfig};
pub use flash_sale::{FlashSaleModule, FlashSaleModuleConfig};
pub use iot::{IotModule, IotModuleConfig, IotSubscription};
pub use ros2::{Ros2Module, Ros2ModuleConfig, Ros2Subscription};
pub use user::{OfficialUserModule, SessionCookie};

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
    use super::{InMemoryUserConfig, LocalGatewayApp, LocalGatewayConfig, OfficialUserModule};
    use crate::AuthContext;
    use bladb_core::protocol::GatewayRequest;
    use serde_json::json;
    use std::path::Path;

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
            super::GatewayRuntimeConfig {
                name: "ros2-bridge".into(),
                policy_yaml: include_str!(
                    "../../../../apps/examples/ros2-bridge/policies/ros2-bridge.policy.yaml"
                )
                .into(),
                topology_yaml: include_str!(
                    "../../../../apps/examples/ros2-bridge/topology/ros2-bridge.topology.yaml"
                )
                .into(),
                default_auth: AuthContext {
                    uid: Some("u_3001".into()),
                    tenant_id: Some("tenant_robotics".into()),
                    roles: vec!["operator".into()],
                    permission_version: Some("v1".into()),
                },
            },
        ]
    }

    fn test_user_module() -> OfficialUserModule {
        OfficialUserModule::from_config(
            None,
            vec![
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
                InMemoryUserConfig {
                    app: "ros2-bridge".into(),
                    uid: "u_3001".into(),
                    tenant_id: "tenant_robotics".into(),
                    email: "operator@ros2.demo".into(),
                    password: "demo123".into(),
                    display_name: "Robot Operator".into(),
                    roles: vec!["operator".into()],
                },
            ],
        )
        .expect("build test user module")
    }

    fn seeded_flash_sale_user() -> InMemoryUserConfig {
        InMemoryUserConfig {
            app: "flash-sale".into(),
            uid: "u_2001".into(),
            tenant_id: "tenant_flashsale".into(),
            email: "buyer@flash-sale.demo".into(),
            password: "demo123".into(),
            display_name: "Flash Buyer".into(),
            roles: vec!["buyer".into()],
        }
    }

    fn official_users_config() -> super::config::OfficialUsersModuleConfig {
        super::config::OfficialUsersModuleConfig {
            enabled: true,
            session: super::config::OfficialUsersSessionConfig {
                transport: Some("gateway-auth".into()),
                launcher_url: None,
            },
            jwt: super::config::OfficialUsersJwtConfig {
                algorithm: Some("HS256".into()),
                secret: Some("${BLADB_JWT_SECRET}".into()),
                public_key_file: None,
                private_key_file: None,
            },
            password: super::config::OfficialUsersPasswordConfig {
                algorithm: Some("argon2id".into()),
            },
            storage: super::config::OfficialUsersStorageConfig {
                engine: Some("mysql".into()),
                mysql: Some(super::config::OfficialUsersMysqlConfig {
                    dsn: Some("${BLADB_USERS_MYSQL_DSN}".into()),
                }),
                mongodb: None,
            },
            mailer: super::config::OfficialUsersMailerConfig {
                provider: None,
                from: None,
                smtp: None,
            },
            features: super::config::OfficialUsersFeaturesConfig {
                register: true,
                login: true,
                verify_email: false,
                reset_password: false,
            },
        }
    }

    #[test]
    fn local_app_handles_flash_sale_order_reads() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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

        assert_eq!(gateways.len(), 3);
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
        assert_eq!(gateways[2]["gateway"], "ros2-bridge");
    }

    #[test]
    fn local_gateway_config_loads_from_example_file() {
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/examples/gateway/local-gateway.yaml");
        let config = LocalGatewayConfig::from_path(&config_path).expect("load gateway config");
        let app = LocalGatewayApp::from_local_config(config).expect("build app from config");

        let topology = app.topology_snapshot();
        assert_eq!(topology.as_array().map(Vec::len), Some(4));
    }

    #[test]
    fn local_gateway_config_loads_from_root_bladb_yaml() {
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bladb.yml");
        let startup = crate::startup::load_gateway_startup(Some(&config_path), Path::new("."))
            .expect("load unified gateway startup");
        let app = match startup {
            crate::startup::GatewayStartup::Standalone { app, .. } => {
                LocalGatewayApp::from_local_config(app).expect("build app from standalone config")
            }
            other => panic!("expected standalone startup, got {other:?}"),
        };

        let topology = app.topology_snapshot();
        assert_eq!(topology.as_array().map(Vec::len), Some(4));
    }

    #[test]
    fn local_gateway_supports_official_users_without_other_module_runtimes() {
        let app = LocalGatewayApp::from_local_config(LocalGatewayConfig {
            runtimes: vec![],
            auth_users: vec![seeded_flash_sale_user()],
            modules: super::LocalGatewayModulesConfig::default(),
            official_users: Some(official_users_config()),
        })
        .expect("build app from official users only config");

        let login = app
            .user_login("flash-sale", "buyer@flash-sale.demo", "demo123")
            .expect("official users login");
        let token = login["token"].as_str().expect("session token");
        let me = app.user_me(token).expect("official users me");

        assert_eq!(app.topology_snapshot().as_array().map(Vec::len), Some(0));
        assert_eq!(me["user"]["app"], "flash-sale");
        assert_eq!(me["user"]["email"], "buyer@flash-sale.demo");
    }

    #[test]
    fn official_users_feature_flags_gate_login_and_register_flows() {
        let mut official_users = official_users_config();
        official_users.features.login = false;
        official_users.features.register = false;

        let app = LocalGatewayApp::from_local_config(LocalGatewayConfig {
            runtimes: vec![],
            auth_users: vec![seeded_flash_sale_user()],
            modules: super::LocalGatewayModulesConfig::default(),
            official_users: Some(official_users),
        })
        .expect("build app from official users only config");

        let login_error = app
            .user_login("flash-sale", "buyer@flash-sale.demo", "demo123")
            .expect_err("login should be disabled");
        assert_eq!(login_error.status, 404);
        assert!(login_error.message.contains("login"));

        let register_error = app
            .user_register(
                "flash-sale",
                "new-buyer@flash-sale.demo",
                "demo123",
                "New Buyer",
            )
            .expect_err("register should be disabled");
        assert_eq!(register_error.status, 404);
        assert!(register_error.message.contains("register"));
    }

    #[test]
    fn official_users_logout_revokes_server_side_session() {
        let app = LocalGatewayApp::from_local_config(LocalGatewayConfig {
            runtimes: vec![],
            auth_users: vec![seeded_flash_sale_user()],
            modules: super::LocalGatewayModulesConfig::default(),
            official_users: Some(official_users_config()),
        })
        .expect("build app from official users only config");

        let login = app
            .user_login("flash-sale", "buyer@flash-sale.demo", "demo123")
            .expect("official users login");
        let token = login["token"].as_str().expect("session token").to_string();

        let logout = app.user_logout(&token).expect("logout");
        assert_eq!(logout["revoked"], true);

        let error = app
            .user_me(&token)
            .expect_err("logged out token should be rejected");
        assert_eq!(error.status, 401);
    }

    #[test]
    fn local_app_dispatches_blog_public_api_without_bearer_token() {
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/examples/gateway/local-gateway.yaml");
        let config = LocalGatewayConfig::from_path(&config_path).expect("load gateway config");
        let app = LocalGatewayApp::from_local_config(config).expect("build app from config");

        let response = app
            .handle_app_api("GET", "/apps/blog/posts", None, None)
            .expect("blog public api result")
            .expect("blog public api payload");
        let posts = response.as_array().expect("blog public post list");

        assert!(!posts.is_empty());
        assert!(posts.iter().all(|post| post["published"] == true));
        assert!(posts.iter().all(|post| post["tenantId"] == "tenant_blog"));
    }

    #[test]
    fn local_app_dispatches_flash_sale_queue_api() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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
    fn local_app_exposes_user_module_login_register_and_me_flow() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
                .expect("build local gateway app");

        let login = app
            .user_login("flash-sale", "buyer@flash-sale.demo", "demo123")
            .expect("user login");
        let login_token = login["token"].as_str().expect("login token");
        let me = app.user_me(login_token).expect("user me");

        assert_eq!(me["user"]["email"], "buyer@flash-sale.demo");
        assert_eq!(me["user"]["app"], "flash-sale");

        let registered = app
            .user_register(
                "flash-sale",
                "new-buyer@flash-sale.demo",
                "demo123",
                "New Buyer",
            )
            .expect("user register");
        let registered_token = registered["token"].as_str().expect("registered token");
        let registered_me = app.user_me(registered_token).expect("registered user me");

        assert_eq!(registered_me["user"]["email"], "new-buyer@flash-sale.demo");
        assert_eq!(registered_me["user"]["displayName"], "New Buyer");
    }

    #[test]
    fn local_app_supports_user_logout_and_revokes_the_session() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
                .expect("build local gateway app");

        let login = app
            .user_login("flash-sale", "buyer@flash-sale.demo", "demo123")
            .expect("user login");
        let login_token = login["token"].as_str().expect("login token");

        let logout = app.user_logout(login_token).expect("user logout");
        assert_eq!(logout["revoked"], true);

        let error = app
            .user_me(login_token)
            .expect_err("revoked session should fail");
        assert_eq!(error.status, 401);
        assert_eq!(error.code, bladb_core::protocol::ErrorCode::AuthExpired);
    }

    #[test]
    fn local_app_dispatches_flash_sale_summary_api() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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
    fn local_app_restores_anonymous_flash_sale_identity_through_cookie_and_me() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
                .expect("build local gateway app");

        let summary = app
            .handle_app_api_http("GET", "/apps/flash-sale/summary", None, None, None)
            .expect("anonymous summary response");
        let cookie = summary
            .session_cookie
            .expect("anonymous session cookie should be set");
        let uid = summary.data.as_ref().expect("summary payload")["identity"]["uid"]
            .as_str()
            .expect("anonymous uid")
            .to_string();

        let cookie_value = cookie
            .header_value()
            .split(';')
            .next()
            .and_then(|pair| pair.split_once('='))
            .map(|(_, value)| value.to_string())
            .expect("cookie token");

        let me = app
            .user_me_http(Some("flash-sale"), None, Some(cookie_value.as_str()))
            .expect("cookie me");

        assert_eq!(me.data["user"]["uid"], uid);
        assert_eq!(me.data["anonymous"], true);
        assert!(me.session_cookie.is_some());
    }

    #[test]
    fn local_app_dispatches_iot_command_history_api() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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
    fn bearer_execute_requires_token_even_when_runtime_has_default_auth() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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

        let error = app
            .handle_execute_for_token(request, None)
            .expect_err("missing bearer token should be rejected");

        assert_eq!(error.status, 401);
        assert_eq!(error.code, bladb_core::protocol::ErrorCode::AuthExpired);
        assert_eq!(error.message, "missing bearer token");
    }

    #[test]
    fn bearer_route_inspection_requires_token_even_when_runtime_has_default_auth() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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

        let error = app
            .inspect_request_for_token(request, None)
            .expect_err("missing bearer token should be rejected");

        assert_eq!(error.status, 401);
        assert_eq!(error.code, bladb_core::protocol::ErrorCode::AuthExpired);
        assert_eq!(error.message, "missing bearer token");
    }

    #[test]
    fn local_app_dispatches_iot_command_publish_api() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
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

    #[test]
    fn local_app_resolves_ros2_publish_templates() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
                .expect("build local gateway app");
        let request: GatewayRequest = serde_json::from_value(json!({
            "kind": "stream",
            "engine": "mqtt",
            "action": "publish",
            "meta": {
                "policy": "ros2.topic.publish",
                "resource": "ros2.topic.publish",
                "params": {
                    "robotId": "robot-001",
                    "topicName": "cmd_vel"
                }
            },
            "topic": {
                "$template": "key",
                "parts": ["tenant/", "/robots/", "/ros2/", ""],
                "values": [
                    { "$ctx": "tenantId", "token": "TENANT_ID" },
                    "{args.robotId}",
                    "{args.topicName}"
                ]
            },
            "payload": {
                "messageType": "geometry_msgs/msg/Twist",
                "linear": { "x": 0.45, "y": 0, "z": 0 },
                "angular": { "x": 0, "y": 0, "z": 0.2 },
                "issuedBy": { "$ctx": "uid", "token": "UID" }
            }
        }))
        .expect("parse ros2 request");

        let response = app.handle_execute(request).expect("execute ros2 request");
        assert_eq!(response["published"], true);
        assert_eq!(
            response["fullTopic"],
            "tenant/tenant_robotics/robots/robot-001/ros2/cmd_vel"
        );
        assert_eq!(response["issuedBy"], "u_3001");
    }

    #[test]
    fn local_app_dispatches_ros2_publish_and_subscribe_apis() {
        let app =
            LocalGatewayApp::with_standard_modules(example_runtime_configs(), test_user_module())
                .expect("build local gateway app");
        let login = app
            .login("ros2-bridge", "operator@ros2.demo", "demo123")
            .expect("login");
        let token = login["token"].as_str().expect("session token");

        let publish_response = app
            .handle_app_api(
                "POST",
                "/apps/ros2-bridge/messages",
                Some(token),
                Some(json!({
                    "robotId": "robot-001",
                    "topicName": "cmd_vel",
                    "messageType": "geometry_msgs/msg/Twist",
                    "payload": {
                        "linear": { "x": 0.4, "y": 0, "z": 0 },
                        "angular": { "x": 0, "y": 0, "z": 0.15 }
                    }
                })),
            )
            .expect("ros2 publish result")
            .expect("ros2 publish payload");

        assert_eq!(publish_response["published"], true);
        assert_eq!(publish_response["robotId"], "robot-001");

        let latest_response = app
            .handle_app_api(
                "GET",
                "/apps/ros2-bridge/messages/cmd_vel/latest",
                Some(token),
                None,
            )
            .expect("ros2 latest result")
            .expect("ros2 latest payload");
        assert_eq!(latest_response["topicName"], "cmd_vel");

        let recent_response = app
            .handle_app_api(
                "GET",
                "/apps/ros2-bridge/messages/cmd_vel",
                Some(token),
                None,
            )
            .expect("ros2 recent result")
            .expect("ros2 recent payload");
        let history = recent_response.as_array().expect("ros2 history array");

        assert!(!history.is_empty());
        assert_eq!(history[0]["topicName"], "cmd_vel");
    }
}
