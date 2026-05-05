use super::{now_label, AppApiHandler, AppApiRequest, AppError};
use crate::{ExecutionContext, ModuleRuntime, RuntimeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    any::Any,
    collections::HashMap,
    sync::{mpsc, Mutex},
};

pub struct IotModule {
    state: Mutex<IotState>,
    allow_anonymous_app_access: bool,
}

pub enum IotSubscription {
    Local(mpsc::Receiver<IotCommandConfig>),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IotModuleConfig {
    #[serde(default)]
    pub devices: Vec<IotDeviceConfig>,
    #[serde(default)]
    pub telemetry_latest: Vec<IotTelemetryConfig>,
    pub active_count: i64,
    #[serde(default)]
    pub commands: Vec<IotCommandConfig>,
    #[serde(default)]
    pub allow_anonymous_app_access: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IotDeviceConfig {
    pub id: String,
    pub name: String,
    pub status: String,
    pub owner_uid: String,
    pub tenant_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IotTelemetryConfig {
    pub device_id: String,
    pub owner_uid: String,
    pub tenant_id: String,
    pub throughput: i64,
    pub temp: i64,
    pub ts: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IotCommandConfig {
    pub id: String,
    pub device_id: String,
    pub topic: String,
    pub action: String,
    pub issued_by: String,
    pub created_at: String,
}

struct IotState {
    devices: Vec<IotDeviceConfig>,
    telemetry_latest: HashMap<String, IotTelemetryConfig>,
    active_count: i64,
    commands: Vec<IotCommandConfig>,
    subscribers: Vec<IotSubscriber>,
    next_command_id: u64,
}

struct IotSubscriber {
    tenant_id: String,
    issued_by: String,
    device_id: String,
    sender: mpsc::Sender<IotCommandConfig>,
}

impl IotModule {
    fn guest_identity(&self) -> (&'static str, &'static str) {
        ("u_1001", "tenant_a")
    }

    pub fn new() -> Self {
        Self::from_config(IotModuleConfig {
            devices: vec![
                IotDeviceConfig {
                    id: "device-001".into(),
                    name: "Boiler Room A".into(),
                    status: "online".into(),
                    owner_uid: "u_1001".into(),
                    tenant_id: "tenant_a".into(),
                },
                IotDeviceConfig {
                    id: "device-002".into(),
                    name: "Cold Chain B".into(),
                    status: "online".into(),
                    owner_uid: "u_1001".into(),
                    tenant_id: "tenant_a".into(),
                },
                IotDeviceConfig {
                    id: "device-003".into(),
                    name: "Meter Cluster C".into(),
                    status: "offline".into(),
                    owner_uid: "u_1001".into(),
                    tenant_id: "tenant_a".into(),
                },
            ],
            telemetry_latest: vec![
                IotTelemetryConfig {
                    device_id: "device-001".into(),
                    owner_uid: "u_1001".into(),
                    tenant_id: "tenant_a".into(),
                    throughput: 842,
                    temp: 36,
                    ts: "2026-05-04T19:12:00Z".into(),
                },
                IotTelemetryConfig {
                    device_id: "device-002".into(),
                    owner_uid: "u_1001".into(),
                    tenant_id: "tenant_a".into(),
                    throughput: 1204,
                    temp: 28,
                    ts: "2026-05-04T19:12:08Z".into(),
                },
                IotTelemetryConfig {
                    device_id: "device-003".into(),
                    owner_uid: "u_1001".into(),
                    tenant_id: "tenant_a".into(),
                    throughput: 0,
                    temp: 19,
                    ts: "2026-05-04T19:11:42Z".into(),
                },
            ],
            active_count: 1824,
            commands: vec![],
            allow_anonymous_app_access: true,
        })
    }

    pub fn from_config(config: IotModuleConfig) -> Self {
        Self {
            state: Mutex::new(IotState {
                devices: config.devices,
                telemetry_latest: config
                    .telemetry_latest
                    .into_iter()
                    .map(|telemetry| (telemetry.device_id.clone(), telemetry))
                    .collect(),
                active_count: config.active_count,
                subscribers: vec![],
                next_command_id: config.commands.len() as u64 + 1,
                commands: config.commands,
            }),
            allow_anonymous_app_access: config.allow_anonymous_app_access,
        }
    }

    pub fn can_stream_path(path: &str) -> bool {
        path.starts_with("/apps/iot-realtime/commands/") && path.ends_with("/stream")
    }

    pub(crate) fn open_command_stream(
        &self,
        session: Option<&crate::local::auth::AuthSession>,
        path: &str,
    ) -> Result<Option<IotSubscription>, AppError> {
        if !Self::can_stream_path(path) {
            return Ok(None);
        }

        let device_id = path
            .trim_start_matches("/apps/iot-realtime/commands/")
            .trim_end_matches("/stream")
            .trim_end_matches('/');
        if device_id.is_empty() {
            return Err(AppError::invalid_request("deviceId is missing"));
        }

        let state = self.state.lock().map_err(AppError::lock_runtime)?;
        let (issued_by, tenant_id) = if let Some(session) = session {
            (session.user.uid.as_str(), session.user.tenant_id.as_str())
        } else if self.allow_anonymous_app_access {
            self.guest_identity()
        } else {
            return Err(AppError::unauthorized("missing bearer token"));
        };
        let device = state
            .devices
            .iter()
            .find(|device| {
                device.id == device_id
                    && device.owner_uid == issued_by
                    && device.tenant_id == tenant_id
            })
            .ok_or_else(|| AppError::not_found("device not found for current viewer"))?
            .clone();
        let (sender, receiver) = mpsc::channel();
        drop(state);

        let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
        state.subscribers.push(IotSubscriber {
            tenant_id: device.tenant_id.clone(),
            issued_by: issued_by.to_string(),
            device_id: device.id.clone(),
            sender,
        });
        Ok(Some(IotSubscription::Local(receiver)))
    }

    fn publish_command_record(
        state: &mut IotState,
        device_id: &str,
        topic: &str,
        action: &str,
        issued_by: &str,
    ) -> Result<Value, RuntimeError> {
        let command_id = format!("cmd_{:04}", state.next_command_id);
        state.next_command_id += 1;

        state.commands.push(IotCommandConfig {
            id: command_id,
            device_id: device_id.to_string(),
            topic: topic.to_string(),
            action: action.to_string(),
            issued_by: issued_by.to_string(),
            created_at: now_label(),
        });

        let last = state
            .commands
            .last()
            .ok_or_else(|| RuntimeError::internal("command queue unexpectedly empty"))?;

        let message = last.clone();
        state.subscribers.retain(|subscriber| {
            if subscriber.tenant_id == message.topic.split('/').nth(1).unwrap_or_default()
                && subscriber.issued_by == message.issued_by
                && subscriber.device_id == message.device_id
            {
                subscriber.sender.send(message.clone()).is_ok()
            } else {
                true
            }
        });

        Ok(json!({
            "published": true,
            "commandId": last.id,
            "deviceId": last.device_id,
            "topic": last.topic,
            "action": last.action,
            "issuedBy": last.issued_by,
            "createdAt": last.created_at
        }))
    }
}

impl ModuleRuntime for IotModule {
    fn handles_cluster(&self, cluster: &str) -> bool {
        cluster.starts_with("iot.")
    }

    fn execute(&self, context: &ExecutionContext) -> Result<Value, RuntimeError> {
        let policy = context.policy_name();
        let body = &context.routed.body;
        match policy {
            "iot.devices.list-mine" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let query = body
                    .query
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("device query is missing"))?;
                let owner_uid = query
                    .get("ownerUid")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("ownerUid is missing"))?;
                let tenant_id = query
                    .get("tenantId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("tenantId is missing"))?;

                let devices: Vec<Value> = state
                    .devices
                    .iter()
                    .filter(|device| device.owner_uid == owner_uid && device.tenant_id == tenant_id)
                    .map(|device| {
                        json!({
                            "id": device.id,
                            "name": device.name,
                            "status": device.status
                        })
                    })
                    .collect();

                Ok(Value::Array(devices))
            }
            "iot.telemetry.read-latest" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let query = body
                    .query
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("telemetry query is missing"))?;
                let device_id = query
                    .get("deviceId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("deviceId is missing"))?;
                let owner_uid = query
                    .get("ownerUid")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("ownerUid is missing"))?;
                let tenant_id = query
                    .get("tenantId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("tenantId is missing"))?;

                let telemetry = state
                    .telemetry_latest
                    .get(device_id)
                    .filter(|telemetry| {
                        telemetry.owner_uid == owner_uid && telemetry.tenant_id == tenant_id
                    })
                    .ok_or_else(|| RuntimeError::not_found("telemetry record not found"))?;

                Ok(json!({
                    "deviceId": telemetry.device_id,
                    "throughput": telemetry.throughput,
                    "temp": telemetry.temp,
                    "ts": telemetry.ts
                }))
            }
            "iot.active-count.read" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                Ok(json!(state.active_count))
            }
            "iot.device-command.publish" => {
                let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
                let topic = body
                    .topic
                    .as_ref()
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("mqtt topic is missing"))?;
                let payload = body
                    .payload
                    .as_ref()
                    .ok_or_else(|| RuntimeError::invalid_request("mqtt payload is missing"))?;
                let action = payload
                    .get("action")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("command action is missing"))?;
                let issued_by = payload
                    .get("issuedBy")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("issuedBy is missing"))?;
                let device_id = context
                    .request
                    .meta
                    .params
                    .get("deviceId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("deviceId is missing"))?;
                Self::publish_command_record(&mut state, device_id, topic, action, issued_by)
            }
            _ => Err(RuntimeError::internal(format!(
                "unsupported iot policy `{policy}`"
            ))),
        }
    }
}

impl AppApiHandler for IotModule {
    fn can_handle(&self, method: &str, path: &str) -> bool {
        ((matches!(method, "GET"))
            && (path == "/apps/iot-realtime/devices"
                || path == "/apps/iot-realtime/active-count"
                || path.starts_with("/apps/iot-realtime/telemetry/")))
            || (path == "/apps/iot-realtime/commands" && matches!(method, "GET" | "POST"))
            || (method == "GET" && Self::can_stream_path(path))
    }

    fn handle(&self, request: AppApiRequest) -> Result<Value, AppError> {
        let (issued_by, tenant_id) = if let Some(session) = request.session.as_ref() {
            if session.user.app != "iot-realtime" {
                return Err(AppError::unauthorized(
                    "iot command history requires an iot-realtime session",
                ));
            }

            (session.user.uid.as_str(), session.user.tenant_id.as_str())
        } else if self.allow_anonymous_app_access {
            self.guest_identity()
        } else {
            return Err(AppError::unauthorized("missing bearer token"));
        };

        match request.method.as_str() {
            "GET" => {
                if request.path == "/apps/iot-realtime/devices" {
                    let state = self.state.lock().map_err(AppError::lock_runtime)?;
                    let devices: Vec<Value> = state
                        .devices
                        .iter()
                        .filter(|device| {
                            device.owner_uid == issued_by && device.tenant_id == tenant_id
                        })
                        .map(|device| {
                            json!({
                                "id": device.id,
                                "name": device.name,
                                "status": device.status
                            })
                        })
                        .collect();

                    return Ok(Value::Array(devices));
                }

                if request.path == "/apps/iot-realtime/active-count" {
                    let state = self.state.lock().map_err(AppError::lock_runtime)?;
                    return Ok(json!(state.active_count));
                }

                if request.path.starts_with("/apps/iot-realtime/telemetry/") {
                    let device_id = request
                        .path
                        .trim_start_matches("/apps/iot-realtime/telemetry/")
                        .trim();
                    if device_id.is_empty() {
                        return Err(AppError::invalid_request("deviceId is missing"));
                    }

                    let state = self.state.lock().map_err(AppError::lock_runtime)?;
                    let telemetry = state
                        .telemetry_latest
                        .get(device_id)
                        .filter(|telemetry| {
                            telemetry.owner_uid == issued_by && telemetry.tenant_id == tenant_id
                        })
                        .ok_or_else(|| AppError::not_found("telemetry record not found"))?;

                    return Ok(json!({
                        "deviceId": telemetry.device_id,
                        "throughput": telemetry.throughput,
                        "temp": telemetry.temp,
                        "ts": telemetry.ts
                    }));
                }

                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let commands: Vec<Value> = state
                    .commands
                    .iter()
                    .rev()
                    .filter(|command| {
                        command.issued_by == issued_by
                            && command.topic.split('/').nth(1) == Some(tenant_id)
                    })
                    .take(10)
                    .map(|command| {
                        json!({
                            "id": command.id,
                            "deviceId": command.device_id,
                            "topic": command.topic,
                            "action": command.action,
                            "issuedBy": command.issued_by,
                            "createdAt": command.created_at
                        })
                    })
                    .collect();

                Ok(Value::Array(commands))
            }
            "POST" => {
                let body = request
                    .body
                    .as_ref()
                    .ok_or_else(|| AppError::invalid_request("command payload is missing"))?;
                let device_id = body
                    .get("deviceId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::invalid_request("deviceId is missing"))?;
                let action = body
                    .get("action")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::invalid_request("action is missing"))?;
                let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
                let device = state
                    .devices
                    .iter()
                    .find(|device| {
                        device.id == device_id
                            && device.owner_uid == issued_by
                            && device.tenant_id == tenant_id
                    })
                    .ok_or_else(|| AppError::not_found("device not found for current viewer"))?;
                let topic = format!("tenant/{}/devices/{}/commands", device.tenant_id, device.id);

                Self::publish_command_record(
                    &mut state,
                    device_id,
                    &topic,
                    action,
                    issued_by,
                )
                .map_err(AppError::from)
            }
            _ => Err(AppError::not_found(format!(
                "unsupported app api route {} {}",
                request.method, request.path
            ))),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::auth::{AuthSession, InMemoryAuthService, InMemoryUserConfig};
    use serde_json::json;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

    fn session() -> AuthSession {
        let service = InMemoryAuthService::from_user_configs(vec![InMemoryUserConfig {
            app: "iot-realtime".into(),
            uid: "u_1001".into(),
            tenant_id: "tenant_a".into(),
            email: "operator@iot.demo".into(),
            password: "demo123".into(),
            display_name: "IoT Operator".into(),
            roles: vec!["operator".into()],
        }]);

        service
            .login("iot-realtime", "operator@iot.demo", "demo123")
            .expect("iot session")
    }

    #[test]
    fn iot_module_publish_pushes_to_matching_device_subscribers() {
        let module = IotModule::new();
        let subscription = module
            .open_command_stream(Some(&session()), "/apps/iot-realtime/commands/device-001/stream")
            .expect("open iot stream")
            .expect("iot local stream");

        let receiver = match subscription {
            IotSubscription::Local(receiver) => receiver,
        };

        let response = module
            .handle(AppApiRequest {
                method: "POST".into(),
                path: "/apps/iot-realtime/commands".into(),
                body: Some(json!({
                    "deviceId": "device-001",
                    "action": "reboot"
                })),
                session: Some(session()),
            })
            .expect("publish command");

        assert_eq!(response["published"], true);

        let message = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("receive mqtt event");
        assert_eq!(message.device_id, "device-001");
        assert_eq!(message.action, "reboot");
        assert_eq!(message.topic, "tenant/tenant_a/devices/device-001/commands");
        assert_eq!(message.issued_by, "u_1001");
    }

    #[test]
    fn iot_module_does_not_push_to_other_device_subscribers() {
        let module = IotModule::new();
        let subscription = module
            .open_command_stream(Some(&session()), "/apps/iot-realtime/commands/device-002/stream")
            .expect("open iot stream")
            .expect("iot local stream");

        let receiver = match subscription {
            IotSubscription::Local(receiver) => receiver,
        };

        module
            .handle(AppApiRequest {
                method: "POST".into(),
                path: "/apps/iot-realtime/commands".into(),
                body: Some(json!({
                    "deviceId": "device-001",
                    "action": "reboot"
                })),
                session: Some(session()),
            })
            .expect("publish command");

        let result = receiver.recv_timeout(Duration::from_millis(200));
        assert!(matches!(result, Err(RecvTimeoutError::Timeout)));
    }

    #[test]
    fn iot_module_allows_anonymous_command_history_when_enabled() {
        let module = IotModule::new();
        let response = module
            .handle(AppApiRequest {
                method: "GET".into(),
                path: "/apps/iot-realtime/commands".into(),
                body: None,
                session: None,
            })
            .expect("anonymous command history");

        assert!(response.as_array().is_some());
    }

    #[test]
    fn iot_module_allows_anonymous_stream_subscription_when_enabled() {
        let module = IotModule::new();
        let subscription = module
            .open_command_stream(None, "/apps/iot-realtime/commands/device-001/stream")
            .expect("open anonymous iot stream");

        assert!(matches!(subscription, Some(IotSubscription::Local(_))));
    }
}
