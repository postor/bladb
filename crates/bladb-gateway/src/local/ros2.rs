use super::{now_label, AppApiHandler, AppApiRequest, AppError};
use crate::{ExecutionContext, ModuleRuntime, RuntimeError};
use bladb_core::protocol::ErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    any::Any,
    sync::{mpsc, Mutex},
    time::Duration,
};

pub struct Ros2Module {
    state: Mutex<Ros2State>,
}

pub enum Ros2Subscription {
    Local(mpsc::Receiver<Ros2MessageConfig>),
    Proxy { url: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ros2ModuleConfig {
    #[serde(default)]
    pub robots: Vec<Ros2RobotConfig>,
    #[serde(default)]
    pub allowed_topics: Vec<String>,
    #[serde(default)]
    pub messages: Vec<Ros2MessageConfig>,
    #[serde(default)]
    pub backend_base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ros2RobotConfig {
    pub id: String,
    pub tenant_id: String,
    pub owner_uid: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ros2MessageConfig {
    pub id: String,
    pub tenant_id: String,
    pub robot_id: String,
    pub topic_name: String,
    pub full_topic: String,
    pub message_type: String,
    pub payload: Value,
    pub issued_by: String,
    pub created_at: String,
}

struct Ros2State {
    robots: Vec<Ros2RobotConfig>,
    allowed_topics: Vec<String>,
    messages: Vec<Ros2MessageConfig>,
    subscribers: Vec<Ros2Subscriber>,
    backend_base_url: Option<String>,
    next_message_id: u64,
}

struct Ros2Subscriber {
    tenant_id: String,
    topic_name: String,
    sender: mpsc::Sender<Ros2MessageConfig>,
}

impl Ros2Module {
    fn backend_base_url(state: &Ros2State) -> Option<String> {
        state.backend_base_url.clone()
    }

    fn backend_url(base_url: &str, path: &str) -> String {
        format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn proxy_http_error(error: ureq::Error) -> AppError {
        match error {
            ureq::Error::Status(status, response) => {
                let payload: Result<Value, _> = serde_json::from_reader(response.into_reader());
                if let Ok(value) = payload {
                    let message = value
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("ros2 backend request failed")
                        .to_string();
                    return AppError {
                        status,
                        code: if status >= 500 {
                            bladb_core::protocol::ErrorCode::InternalError
                        } else {
                            bladb_core::protocol::ErrorCode::InvalidRequest
                        },
                        message,
                    };
                }

                AppError {
                    status,
                    code: if status >= 500 {
                        ErrorCode::InternalError
                    } else {
                        ErrorCode::InvalidRequest
                    },
                    message: "ros2 backend request failed".into(),
                }
            }
            ureq::Error::Transport(transport) => {
                AppError::internal(format!("failed to reach ros2 backend: {transport}"))
            }
        }
    }

    fn proxy_backend_json(
        method: &str,
        base_url: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, AppError> {
        let url = Self::backend_url(base_url, path);
        let agent = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();
        let request = agent.request(method, &url);
        let response = match body {
            Some(payload) => request
                .set("content-type", "application/json")
                .send_string(&payload.to_string()),
            None => request.call(),
        }
        .map_err(Self::proxy_http_error)?;

        let payload: Value = serde_json::from_reader(response.into_reader()).map_err(|error| {
            AppError::internal(format!("failed to parse ros2 backend response: {error}"))
        })?;

        Ok(payload.get("data").cloned().unwrap_or(payload))
    }

    fn proxy_publish_message(
        base_url: &str,
        tenant_id: &str,
        uid: &str,
        body: &Value,
    ) -> Result<Value, AppError> {
        let robot_id = body
            .get("robotId")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::invalid_request("robotId is missing"))?;
        let topic_name = body
            .get("topicName")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::invalid_request("topicName is missing"))?;
        let message_type = body
            .get("messageType")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::invalid_request("messageType is missing"))?;
        let payload = body
            .get("payload")
            .cloned()
            .ok_or_else(|| AppError::invalid_request("payload is missing"))?;

        let mut payload_object = payload
            .as_object()
            .cloned()
            .ok_or_else(|| AppError::invalid_request("payload must be an object"))?;
        payload_object.insert("tenantId".into(), json!(tenant_id));
        payload_object.insert("issuedBy".into(), json!(uid));

        Self::proxy_backend_json(
            "POST",
            base_url,
            "/messages",
            Some(&json!({
                "robotId": robot_id,
                "topicName": topic_name,
                "messageType": message_type,
                "payload": Value::Object(payload_object),
                "issuedBy": uid
            })),
        )
    }

    fn proxy_recent_messages(base_url: &str, topic_name: &str) -> Result<Value, AppError> {
        Self::proxy_backend_json("GET", base_url, &format!("/messages/{topic_name}"), None)
    }

    fn proxy_latest_message(base_url: &str, topic_name: &str) -> Result<Value, AppError> {
        Self::proxy_backend_json(
            "GET",
            base_url,
            &format!("/messages/{topic_name}/latest"),
            None,
        )
    }

    pub fn new() -> Self {
        Self::from_config(Ros2ModuleConfig {
            robots: vec![Ros2RobotConfig {
                id: "robot-001".into(),
                tenant_id: "tenant_robotics".into(),
                owner_uid: "u_3001".into(),
                name: "Warehouse AMR 01".into(),
            }],
            allowed_topics: vec!["cmd_vel".into(), "battery_state".into(), "odom".into()],
            messages: vec![
                Ros2MessageConfig {
                    id: "ros2msg_0001".into(),
                    tenant_id: "tenant_robotics".into(),
                    robot_id: "robot-001".into(),
                    topic_name: "cmd_vel".into(),
                    full_topic: "tenant/tenant_robotics/robots/robot-001/ros2/cmd_vel".into(),
                    message_type: "geometry_msgs/msg/Twist".into(),
                    payload: json!({
                        "linear": { "x": 0.3, "y": 0, "z": 0 },
                        "angular": { "x": 0, "y": 0, "z": 0.1 }
                    }),
                    issued_by: "u_3001".into(),
                    created_at: "2026-05-05T09:10:00Z".into(),
                },
                Ros2MessageConfig {
                    id: "ros2msg_0002".into(),
                    tenant_id: "tenant_robotics".into(),
                    robot_id: "robot-001".into(),
                    topic_name: "battery_state".into(),
                    full_topic: "tenant/tenant_robotics/robots/robot-001/ros2/battery_state".into(),
                    message_type: "sensor_msgs/msg/BatteryState".into(),
                    payload: json!({
                        "percentage": 0.84,
                        "voltage": 24.6
                    }),
                    issued_by: "u_3001".into(),
                    created_at: "2026-05-05T09:11:00Z".into(),
                },
            ],
            backend_base_url: None,
        })
    }

    pub fn from_config(config: Ros2ModuleConfig) -> Self {
        Self {
            state: Mutex::new(Ros2State {
                next_message_id: config.messages.len() as u64 + 1,
                robots: config.robots,
                allowed_topics: config.allowed_topics,
                messages: config.messages,
                subscribers: vec![],
                backend_base_url: config.backend_base_url,
            }),
        }
    }

    pub fn can_stream_path(path: &str) -> bool {
        path.starts_with("/apps/ros2-bridge/messages/") && path.ends_with("/stream")
    }

    pub fn open_message_stream(
        &self,
        session: &crate::local::auth::AuthSession,
        path: &str,
    ) -> Result<Option<Ros2Subscription>, AppError> {
        if !Self::can_stream_path(path) {
            return Ok(None);
        }

        let topic_name = path
            .trim_start_matches("/apps/ros2-bridge/messages/")
            .trim_end_matches("/stream")
            .trim_end_matches('/');
        if topic_name.is_empty() {
            return Err(AppError::invalid_request("topicName is missing"));
        }

        let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
        Self::ensure_allowed_topic(&state, topic_name).map_err(AppError::from)?;

        if let Some(base_url) = Self::backend_base_url(&state) {
            return Ok(Some(Ros2Subscription::Proxy {
                url: Self::backend_url(&base_url, &format!("/messages/{topic_name}/stream")),
            }));
        }

        let (sender, receiver) = mpsc::channel();
        state.subscribers.push(Ros2Subscriber {
            tenant_id: session.user.tenant_id.clone(),
            topic_name: topic_name.to_string(),
            sender,
        });
        Ok(Some(Ros2Subscription::Local(receiver)))
    }

    fn ensure_allowed_topic(state: &Ros2State, topic_name: &str) -> Result<(), RuntimeError> {
        if state.allowed_topics.iter().any(|topic| topic == topic_name) {
            Ok(())
        } else {
            Err(RuntimeError::invalid_request(format!(
                "topic `{topic_name}` is not allowed for this tenant"
            )))
        }
    }

    fn ensure_robot_access<'a>(
        state: &'a Ros2State,
        tenant_id: &str,
        uid: &str,
        robot_id: &str,
    ) -> Result<&'a Ros2RobotConfig, RuntimeError> {
        state
            .robots
            .iter()
            .find(|robot| {
                robot.id == robot_id && robot.tenant_id == tenant_id && robot.owner_uid == uid
            })
            .ok_or_else(|| RuntimeError::not_found("robot not found for current session"))
    }

    fn publish_message(
        state: &mut Ros2State,
        tenant_id: &str,
        uid: &str,
        robot_id: &str,
        topic_name: &str,
        message_type: &str,
        payload: Value,
    ) -> Result<Value, RuntimeError> {
        Self::ensure_allowed_topic(state, topic_name)?;
        let _robot = Self::ensure_robot_access(state, tenant_id, uid, robot_id)?;
        let full_topic = format!("tenant/{tenant_id}/robots/{robot_id}/ros2/{topic_name}");
        let id = format!("ros2msg_{:04}", state.next_message_id);
        state.next_message_id += 1;
        let message = Ros2MessageConfig {
            id: id.clone(),
            tenant_id: tenant_id.to_string(),
            robot_id: robot_id.to_string(),
            topic_name: topic_name.to_string(),
            full_topic: full_topic.clone(),
            message_type: message_type.to_string(),
            payload,
            issued_by: uid.to_string(),
            created_at: now_label(),
        };
        state.messages.push(message.clone());
        state.subscribers.retain(|subscriber| {
            if subscriber.tenant_id == tenant_id && subscriber.topic_name == topic_name {
                subscriber.sender.send(message.clone()).is_ok()
            } else {
                true
            }
        });

        Ok(json!({
            "published": true,
            "messageId": message.id,
            "robotId": message.robot_id,
            "topicName": message.topic_name,
            "fullTopic": message.full_topic,
            "messageType": message.message_type,
            "payload": message.payload,
            "issuedBy": message.issued_by,
            "createdAt": message.created_at
        }))
    }

    fn latest_message(
        state: &Ros2State,
        tenant_id: &str,
        topic_name: &str,
    ) -> Result<Value, RuntimeError> {
        let message = state
            .messages
            .iter()
            .rev()
            .find(|message| message.tenant_id == tenant_id && message.topic_name == topic_name)
            .ok_or_else(|| RuntimeError::not_found("ros2 topic snapshot not found"))?;

        Ok(json!({
            "robotId": message.robot_id,
            "topicName": message.topic_name,
            "fullTopic": message.full_topic,
            "messageType": message.message_type,
            "payload": message.payload,
            "issuedBy": message.issued_by,
            "createdAt": message.created_at
        }))
    }

    fn recent_messages(state: &Ros2State, tenant_id: &str, topic_name: &str) -> Value {
        Value::Array(
            state
                .messages
                .iter()
                .rev()
                .filter(|message| {
                    message.tenant_id == tenant_id && message.topic_name == topic_name
                })
                .take(12)
                .map(|message| {
                    json!({
                        "id": message.id,
                        "robotId": message.robot_id,
                        "topicName": message.topic_name,
                        "fullTopic": message.full_topic,
                        "messageType": message.message_type,
                        "payload": message.payload,
                        "issuedBy": message.issued_by,
                        "createdAt": message.created_at
                    })
                })
                .collect(),
        )
    }
}

impl ModuleRuntime for Ros2Module {
    fn handles_cluster(&self, cluster: &str) -> bool {
        cluster.starts_with("ros2.")
    }

    fn execute(&self, context: &ExecutionContext) -> Result<Value, RuntimeError> {
        let policy = context.policy_name();
        let tenant_id = context
            .auth
            .tenant_id
            .as_deref()
            .ok_or_else(|| RuntimeError::invalid_request("tenantId is missing"))?;
        let uid = context
            .auth
            .uid
            .as_deref()
            .ok_or_else(|| RuntimeError::invalid_request("uid is missing"))?;

        match policy {
            "ros2.topic.publish" => {
                let mut state = self.state.lock().map_err(AppError::lock_runtime)?;
                let topic = context
                    .routed
                    .body
                    .topic
                    .as_ref()
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("topic is missing"))?;
                let payload = context
                    .routed
                    .body
                    .payload
                    .clone()
                    .ok_or_else(|| RuntimeError::invalid_request("payload is missing"))?;
                let robot_id = context
                    .request
                    .meta
                    .params
                    .get("robotId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("robotId is missing"))?;
                let topic_name = context
                    .request
                    .meta
                    .params
                    .get("topicName")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("topicName is missing"))?;
                if topic != format!("tenant/{tenant_id}/robots/{robot_id}/ros2/{topic_name}") {
                    return Err(RuntimeError::invalid_request(
                        "prepared ros2 topic does not match tenant-scoped topic contract",
                    ));
                }

                let message_type = payload
                    .get("messageType")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RuntimeError::invalid_request("messageType is missing"))?;
                let payload_body = payload
                    .as_object()
                    .map(|value| {
                        let mut cloned = value.clone();
                        cloned.remove("messageType");
                        Value::Object(cloned)
                    })
                    .ok_or_else(|| RuntimeError::invalid_request("payload must be an object"))?;

                Self::publish_message(
                    &mut state,
                    tenant_id,
                    uid,
                    robot_id,
                    topic_name,
                    message_type,
                    payload_body,
                )
            }
            "ros2.topic.read-latest" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let topic_name = context
                    .request
                    .meta
                    .params
                    .get("topicName")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        context
                            .routed
                            .body
                            .query
                            .as_ref()
                            .and_then(|query| query.get("topicName"))
                            .and_then(Value::as_str)
                    })
                    .ok_or_else(|| RuntimeError::invalid_request("topicName is missing"))?;
                Self::latest_message(&state, tenant_id, topic_name)
            }
            "ros2.topic.read-recent" => {
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                let topic_name = context
                    .request
                    .meta
                    .params
                    .get("topicName")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        context
                            .routed
                            .body
                            .query
                            .as_ref()
                            .and_then(|query| query.get("topicName"))
                            .and_then(Value::as_str)
                    })
                    .ok_or_else(|| RuntimeError::invalid_request("topicName is missing"))?;
                Ok(Self::recent_messages(&state, tenant_id, topic_name))
            }
            _ => Err(RuntimeError::internal(format!(
                "unsupported ros2 policy `{policy}`"
            ))),
        }
    }
}

impl AppApiHandler for Ros2Module {
    fn can_handle(&self, method: &str, path: &str) -> bool {
        if path == "/apps/ros2-bridge/messages" {
            return matches!(method, "GET" | "POST");
        }

        path.starts_with("/apps/ros2-bridge/messages/") && method == "GET"
    }

    fn handle(&self, request: AppApiRequest) -> Result<Value, AppError> {
        let session = request
            .session
            .ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
        if session.user.app != "ros2-bridge" {
            return Err(AppError::unauthorized(
                "ros2 bridge app api requires a ros2-bridge session",
            ));
        }

        match (request.method.as_str(), request.path.as_str()) {
            ("POST", "/apps/ros2-bridge/messages") => {
                let body = request
                    .body
                    .as_ref()
                    .ok_or_else(|| AppError::invalid_request("message payload is missing"))?;
                let backend_base_url = {
                    let state = self.state.lock().map_err(AppError::lock_runtime)?;
                    Self::backend_base_url(&state)
                };
                if let Some(base_url) = backend_base_url {
                    return Self::proxy_publish_message(
                        &base_url,
                        &session.user.tenant_id,
                        &session.user.uid,
                        body,
                    );
                }
                let robot_id = body
                    .get("robotId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::invalid_request("robotId is missing"))?;
                let topic_name = body
                    .get("topicName")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::invalid_request("topicName is missing"))?;
                let message_type = body
                    .get("messageType")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::invalid_request("messageType is missing"))?;
                let payload = body
                    .get("payload")
                    .cloned()
                    .ok_or_else(|| AppError::invalid_request("payload is missing"))?;
                let mut state = self.state.lock().map_err(AppError::lock_runtime)?;

                Self::publish_message(
                    &mut state,
                    &session.user.tenant_id,
                    &session.user.uid,
                    robot_id,
                    topic_name,
                    message_type,
                    payload,
                )
                .map_err(AppError::from)
            }
            ("GET", "/apps/ros2-bridge/messages") => {
                let backend_base_url = {
                    let state = self.state.lock().map_err(AppError::lock_runtime)?;
                    Self::backend_base_url(&state)
                };
                if let Some(base_url) = backend_base_url {
                    return Self::proxy_recent_messages(&base_url, "cmd_vel");
                }
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                Ok(Self::recent_messages(
                    &state,
                    &session.user.tenant_id,
                    "cmd_vel",
                ))
            }
            ("GET", path) if path.ends_with("/latest") => {
                let topic_name = path
                    .trim_start_matches("/apps/ros2-bridge/messages/")
                    .trim_end_matches("/latest")
                    .trim_end_matches('/');
                let backend_base_url = {
                    let state = self.state.lock().map_err(AppError::lock_runtime)?;
                    Self::backend_base_url(&state)
                };
                if let Some(base_url) = backend_base_url {
                    return Self::proxy_latest_message(&base_url, topic_name);
                }
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                Self::latest_message(&state, &session.user.tenant_id, topic_name)
                    .map_err(AppError::from)
            }
            ("GET", path) => {
                let topic_name = path.trim_start_matches("/apps/ros2-bridge/messages/");
                let backend_base_url = {
                    let state = self.state.lock().map_err(AppError::lock_runtime)?;
                    Self::backend_base_url(&state)
                };
                if let Some(base_url) = backend_base_url {
                    return Self::proxy_recent_messages(&base_url, topic_name);
                }
                let state = self.state.lock().map_err(AppError::lock_runtime)?;
                Ok(Self::recent_messages(
                    &state,
                    &session.user.tenant_id,
                    topic_name,
                ))
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
    use super::{Ros2MessageConfig, Ros2Module, Ros2ModuleConfig, Ros2RobotConfig, Ros2Subscription};
    use crate::{
        local::{auth::AuthSession, AppApiHandler, AppApiRequest, AppError},
        AuthContext, Authorization, ExecutionContext, ModuleRuntime, RouteSelection, RoutedRequest,
        RuntimeError,
    };
    use bladb_core::{
        cluster::ModuleCategory,
        protocol::{Engine, GatewayRequest, RequestBody, RequestKind},
    };
    use serde_json::json;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        thread,
        time::Duration,
    };

    fn module() -> Ros2Module {
        Ros2Module::from_config(Ros2ModuleConfig {
            robots: vec![Ros2RobotConfig {
                id: "robot-001".into(),
                tenant_id: "tenant_robotics".into(),
                owner_uid: "u_3001".into(),
                name: "Warehouse AMR 01".into(),
            }],
            allowed_topics: vec!["cmd_vel".into(), "battery_state".into()],
            messages: vec![Ros2MessageConfig {
                id: "ros2msg_0001".into(),
                tenant_id: "tenant_robotics".into(),
                robot_id: "robot-001".into(),
                topic_name: "cmd_vel".into(),
                full_topic: "tenant/tenant_robotics/robots/robot-001/ros2/cmd_vel".into(),
                message_type: "geometry_msgs/msg/Twist".into(),
                payload: json!({
                    "linear": { "x": 0.3, "y": 0, "z": 0 },
                    "angular": { "x": 0, "y": 0, "z": 0.1 }
                }),
                issued_by: "u_3001".into(),
                created_at: "2026-05-05T09:10:00Z".into(),
            }],
            backend_base_url: None,
        })
    }

    fn execution_context() -> ExecutionContext {
        ExecutionContext {
            request: GatewayRequest {
                kind: RequestKind::Stream,
                engine: Engine::Mqtt,
                action: "publish".into(),
                meta: bladb_core::protocol::RequestMeta {
                    resource: Some("ros2.topic.publish".into()),
                    policy: Some("ros2.topic.publish".into()),
                    trace_id: None,
                    params: std::collections::BTreeMap::from([
                        ("robotId".into(), json!("robot-001")),
                        ("topicName".into(), json!("cmd_vel")),
                    ]),
                },
                body: RequestBody {
                    topic: Some(json!(
                        "tenant/tenant_robotics/robots/robot-001/ros2/cmd_vel"
                    )),
                    payload: Some(json!({
                        "messageType": "geometry_msgs/msg/Twist",
                        "linear": { "x": 0.6, "y": 0, "z": 0 },
                        "angular": { "x": 0, "y": 0, "z": 0.2 }
                    })),
                    ..Default::default()
                },
            },
            auth: AuthContext {
                uid: Some("u_3001".into()),
                tenant_id: Some("tenant_robotics".into()),
                roles: vec!["operator".into()],
                permission_version: Some("v1".into()),
            },
            routed: RoutedRequest {
                authorization: Authorization {
                    policy_name: "ros2.topic.publish".into(),
                },
                route: RouteSelection {
                    cluster: "ros2.bridge-mqtt".into(),
                    category: ModuleCategory::Stream,
                    runtime: "mqtt".into(),
                    service: "bladb-module-ros2-bridge".into(),
                    namespace: Some("bladb".into()),
                    route_key: Some("tenant_robotics".into()),
                    shard: Some(1),
                    sticky: true,
                },
                body: RequestBody {
                    topic: Some(json!(
                        "tenant/tenant_robotics/robots/robot-001/ros2/cmd_vel"
                    )),
                    payload: Some(json!({
                        "messageType": "geometry_msgs/msg/Twist",
                        "linear": { "x": 0.6, "y": 0, "z": 0 },
                        "angular": { "x": 0, "y": 0, "z": 0.2 }
                    })),
                    ..Default::default()
                },
            },
        }
    }

    fn session() -> AuthSession {
        let service = crate::local::auth::InMemoryAuthService::from_user_configs(vec![
            crate::local::auth::InMemoryUserConfig {
                app: "ros2-bridge".into(),
                uid: "u_3001".into(),
                tenant_id: "tenant_robotics".into(),
                email: "operator@ros2.demo".into(),
                password: "demo123".into(),
                display_name: "Robot Operator".into(),
                roles: vec!["operator".into()],
            },
        ]);

        service
            .login("ros2-bridge", "operator@ros2.demo", "demo123")
            .expect("ros2 session")
    }

    #[test]
    fn ros2_module_publishes_tenant_scoped_message() {
        let module = module();
        let response = module
            .execute(&execution_context())
            .expect("publish message");
        assert_eq!(response["published"], true);
        assert_eq!(response["robotId"], "robot-001");
        assert_eq!(response["topicName"], "cmd_vel");
    }

    #[test]
    fn ros2_module_exposes_recent_messages_via_app_api() {
        let module = module();
        let response = module
            .handle(AppApiRequest {
                method: "GET".into(),
                path: "/apps/ros2-bridge/messages/cmd_vel".into(),
                body: None,
                session: Some(session()),
            })
            .expect("recent messages");

        let rows = response.as_array().expect("recent messages array");
        assert!(!rows.is_empty());
        assert_eq!(rows[0]["topicName"], "cmd_vel");
    }

    #[test]
    fn ros2_module_rejects_unknown_topic_publish() {
        let module = module();
        let response = module.handle(AppApiRequest {
            method: "POST".into(),
            path: "/apps/ros2-bridge/messages".into(),
            body: Some(json!({
                "robotId": "robot-001",
                "topicName": "secret_topic",
                "messageType": "std_msgs/msg/String",
                "payload": { "data": "forbidden" }
            })),
            session: Some(session()),
        });

        let error = response.expect_err("expected invalid topic");
        assert_eq!(
            error,
            AppError::from(RuntimeError::invalid_request(
                "topic `secret_topic` is not allowed for this tenant"
            ))
        );
    }

    #[test]
    fn ros2_module_subscribers_receive_published_messages() {
        let module = module();
        let subscription = module
            .open_message_stream(&session(), "/apps/ros2-bridge/messages/cmd_vel/stream")
            .expect("open local ros2 stream")
            .expect("local ros2 stream");

        let receiver = match subscription {
            Ros2Subscription::Local(receiver) => receiver,
            Ros2Subscription::Proxy { .. } => panic!("expected local subscription"),
        };

        module
            .handle(AppApiRequest {
                method: "POST".into(),
                path: "/apps/ros2-bridge/messages".into(),
                body: Some(json!({
                    "robotId": "robot-001",
                    "topicName": "cmd_vel",
                    "messageType": "geometry_msgs/msg/Twist",
                    "payload": {
                        "linear": { "x": 0.7, "y": 0, "z": 0 },
                        "angular": { "x": 0, "y": 0, "z": 0.25 }
                    }
                })),
                session: Some(session()),
            })
            .expect("publish local ros2 message");

        let message = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("receive published ros2 message");
        assert_eq!(message.topic_name, "cmd_vel");
        assert_eq!(message.robot_id, "robot-001");
        assert_eq!(message.payload["linear"]["x"], json!(0.7));
    }

    #[test]
    fn ros2_module_returns_proxy_subscription_when_backend_is_configured() {
        let module = Ros2Module::from_config(Ros2ModuleConfig {
            robots: vec![],
            allowed_topics: vec!["cmd_vel".into()],
            messages: vec![],
            backend_base_url: Some("http://ros2-backend:8080".into()),
        });

        let subscription = module
            .open_message_stream(&session(), "/apps/ros2-bridge/messages/cmd_vel/stream")
            .expect("open proxy ros2 stream")
            .expect("proxy ros2 stream");

        match subscription {
            Ros2Subscription::Local(_) => panic!("expected proxy subscription"),
            Ros2Subscription::Proxy { url } => {
                assert_eq!(url, "http://ros2-backend:8080/messages/cmd_vel/stream");
            }
        }
    }

    #[test]
    fn ros2_module_proxies_publish_to_backend_when_backend_base_url_is_configured() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind proxy backend");
        let address = listener.local_addr().expect("proxy backend addr");
        let (request_tx, request_rx) = mpsc::channel();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept proxy request");
            let mut request_bytes = Vec::new();
            let mut buffer = [0_u8; 1024];
            let header_end;
            let content_length;

            loop {
                let bytes_read = stream.read(&mut buffer).expect("read proxy request");
                if bytes_read == 0 {
                    panic!("proxy request closed before headers were fully read");
                }
                request_bytes.extend_from_slice(&buffer[..bytes_read]);
                if let Some(index) = request_bytes.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    header_end = index;
                    let headers = String::from_utf8_lossy(&request_bytes[..index]);
                    content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            if name.eq_ignore_ascii_case("content-length") {
                                value.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    break;
                }
            }

            let body_start = header_end + 4;
            while request_bytes.len() < body_start + content_length {
                let bytes_read = stream.read(&mut buffer).expect("read proxy request body");
                if bytes_read == 0 {
                    panic!("proxy request closed before body was fully read");
                }
                request_bytes.extend_from_slice(&buffer[..bytes_read]);
            }

            let request = String::from_utf8_lossy(&request_bytes).to_string();
            request_tx.send(request).expect("send captured request");

            let response_body = json!({
                "ok": true,
                "data": {
                    "published": true,
                    "messageId": "ros2msg_0099",
                    "robotId": "robot-001",
                    "topicName": "cmd_vel",
                    "fullTopic": "tenant/tenant_robotics/robots/robot-001/ros2/cmd_vel",
                    "messageType": "geometry_msgs/msg/Twist",
                    "payload": {
                        "linear": { "x": 0.1, "y": 0, "z": 0 }
                    },
                    "issuedBy": "u_3001",
                    "createdAt": "2026-05-05T12:00:00Z"
                }
            })
            .to_string();

            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            )
            .expect("write proxy response");
            stream.flush().expect("flush proxy response");
        });

        let module = Ros2Module::from_config(Ros2ModuleConfig {
            robots: vec![],
            allowed_topics: vec!["cmd_vel".into()],
            messages: vec![],
            backend_base_url: Some(format!("http://{address}")),
        });

        let response = module
            .handle(AppApiRequest {
                method: "POST".into(),
                path: "/apps/ros2-bridge/messages".into(),
                body: Some(json!({
                    "robotId": "robot-001",
                    "topicName": "cmd_vel",
                    "messageType": "geometry_msgs/msg/Twist",
                    "payload": {
                        "linear": { "x": 0.1 },
                        "issuedBy": "forged-from-browser"
                    }
                })),
                session: Some(session()),
            })
            .expect("expected proxied publish response");

        server.join().expect("proxy server join");
        let captured_request = request_rx.recv().expect("captured proxy request");
        let captured_body = captured_request
            .split("\r\n\r\n")
            .nth(1)
            .expect("captured proxy request body");

        assert_eq!(
            response,
            json!({
                "published": true,
                "messageId": "ros2msg_0099",
                "robotId": "robot-001",
                "topicName": "cmd_vel",
                "fullTopic": "tenant/tenant_robotics/robots/robot-001/ros2/cmd_vel",
                "messageType": "geometry_msgs/msg/Twist",
                "payload": {
                    "linear": { "x": 0.1, "y": 0, "z": 0 }
                },
                "issuedBy": "u_3001",
                "createdAt": "2026-05-05T12:00:00Z"
            })
        );
        assert!(captured_request.starts_with("POST /messages HTTP/1.1"));
        assert!(captured_body.contains("\"robotId\":\"robot-001\""));
        assert!(captured_body.contains("\"topicName\":\"cmd_vel\""));
        assert!(captured_body.contains("\"issuedBy\":\"u_3001\""));
        assert!(!captured_body.contains("forged-from-browser"));
    }
}
