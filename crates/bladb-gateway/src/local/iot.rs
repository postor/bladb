use super::{now_label, AppApiHandler, AppApiRequest, AppError};
use crate::{ExecutionContext, ModuleRuntime, RuntimeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Mutex};

pub struct IotModule {
    state: Mutex<IotState>,
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
    next_command_id: u64,
}

impl IotModule {
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
                next_command_id: config.commands.len() as u64 + 1,
                commands: config.commands,
            }),
        }
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
        path == "/apps/iot-realtime/commands" && matches!(method, "GET" | "POST")
    }

    fn handle(&self, request: AppApiRequest) -> Result<Value, AppError> {
        let session = request
            .session
            .ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
        if session.user.app != "iot-realtime" {
            return Err(AppError::unauthorized(
                "iot command history requires an iot-realtime session",
            ));
        }

        match request.method.as_str() {
            "GET" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let commands: Vec<Value> = state
                    .commands
                    .iter()
                    .rev()
                    .filter(|command| command.issued_by == session.user.uid)
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
                            && device.owner_uid == session.user.uid
                            && device.tenant_id == session.user.tenant_id
                    })
                    .ok_or_else(|| AppError::not_found("device not found for current session"))?;
                let topic = format!("tenant/{}/devices/{}/commands", device.tenant_id, device.id);

                Self::publish_command_record(
                    &mut state,
                    device_id,
                    &topic,
                    action,
                    &session.user.uid,
                )
                .map_err(AppError::from)
            }
            _ => Err(AppError::not_found(format!(
                "unsupported app api route {} {}",
                request.method, request.path
            ))),
        }
    }
}
