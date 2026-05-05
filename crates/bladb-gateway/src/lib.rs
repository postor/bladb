pub mod local;
pub mod routing;
pub mod runtime;
pub mod startup;

use bladb_core::policy::{
    parse_policy_manifest, PolicyDefinition, PolicyManifest, PolicyManifestError,
};
use bladb_core::protocol::{GatewayRequest, ProtocolError, RequestBody};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

pub use local::{
    AppError, FlashSaleModule, FlashSaleModuleConfig, GatewayRuntimeConfig, InMemoryAuthService,
    InMemoryUserConfig, IotModule, IotModuleConfig, LocalGatewayApp, LocalGatewayConfig,
    LocalGatewayModulesConfig, Ros2Module, Ros2ModuleConfig, Ros2Subscription,
};
pub use routing::{
    route_prepared_request, ModuleRegistry, ModuleRegistryInitError, RouteError, RouteSelection,
    RoutedRequest,
};
pub use runtime::{ExecutionContext, ModuleRuntime, RuntimeError, RuntimeRegistry};
pub use startup::{
    discover_bladb_config, load_gateway_startup, GatewayStartup, GatewayStartupError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorization {
    pub policy_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthContext {
    pub uid: Option<String>,
    pub tenant_id: Option<String>,
    pub roles: Vec<String>,
    pub permission_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRequest {
    pub authorization: Authorization,
    pub body: RequestBody,
}

#[derive(Debug, Clone)]
pub struct Gateway {
    manifest: PolicyManifest,
    policy_index: HashMap<String, usize>,
}

impl Gateway {
    pub fn from_manifest(manifest: PolicyManifest) -> Self {
        let policy_index = manifest
            .policies
            .iter()
            .enumerate()
            .map(|(index, policy)| (policy.name.clone(), index))
            .collect();

        Self {
            manifest,
            policy_index,
        }
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, GatewayInitError> {
        let manifest = parse_policy_manifest(yaml)?;
        Ok(Self::from_manifest(manifest))
    }

    pub fn authorize(&self, request: &GatewayRequest) -> Result<Authorization, GatewayError> {
        request.validate().map_err(GatewayError::InvalidRequest)?;

        let policy = if let Some(policy_name) = request.meta.policy.as_deref() {
            let policy = self
                .policy_index
                .get(policy_name)
                .and_then(|index| self.manifest.policies.get(*index))
                .ok_or_else(|| GatewayError::UnknownPolicy(policy_name.to_string()))?;

            if !policy_matches_request(policy, request) {
                return Err(GatewayError::PolicyMismatch {
                    policy: policy_name.to_string(),
                });
            }

            policy
        } else {
            self.manifest
                .policies
                .iter()
                .find(|policy| policy_matches_request(policy, request))
                .ok_or(GatewayError::NoPolicyMatch)?
        };

        Ok(Authorization {
            policy_name: policy.name.clone(),
        })
    }

    pub fn prepare(
        &self,
        request: &GatewayRequest,
        auth: &AuthContext,
    ) -> Result<PreparedRequest, GatewayError> {
        let authorization = self.authorize(request)?;
        let params = &request.meta.params;

        Ok(PreparedRequest {
            authorization,
            body: RequestBody {
                statement: request.body.statement.clone(),
                values: request
                    .body
                    .values
                    .iter()
                    .map(|value| resolve_value(value, auth, params))
                    .collect::<Result<Vec<_>, _>>()?,
                collection: request.body.collection.clone(),
                query: request
                    .body
                    .query
                    .as_ref()
                    .map(|map| resolve_map(map, auth, params))
                    .transpose()?,
                document: request
                    .body
                    .document
                    .as_ref()
                    .map(|map| resolve_map(map, auth, params))
                    .transpose()?,
                options: request.body.options.clone(),
                name: request
                    .body
                    .name
                    .as_ref()
                    .map(|value| resolve_value(value, auth, params))
                    .transpose()?,
                channel: request
                    .body
                    .channel
                    .as_ref()
                    .map(|value| resolve_value(value, auth, params))
                    .transpose()?,
                value: request
                    .body
                    .value
                    .as_ref()
                    .map(|value| resolve_value(value, auth, params))
                    .transpose()?,
                amount: request.body.amount,
                payload: request
                    .body
                    .payload
                    .as_ref()
                    .map(|value| resolve_value(value, auth, params))
                    .transpose()?,
                topic: request
                    .body
                    .topic
                    .as_ref()
                    .map(|value| resolve_value(value, auth, params))
                    .transpose()?,
                queue: request.body.queue.clone(),
                message: request
                    .body
                    .message
                    .as_ref()
                    .map(|value| resolve_value(value, auth, params))
                    .transpose()?,
                delay_ms: request.body.delay_ms,
            },
        })
    }
}

#[derive(Debug, Error)]
pub enum GatewayInitError {
    #[error(transparent)]
    Policy(#[from] PolicyManifestError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GatewayError {
    #[error(transparent)]
    InvalidRequest(#[from] ProtocolError),
    #[error("unknown policy `{0}`")]
    UnknownPolicy(String),
    #[error("request does not satisfy policy `{policy}`")]
    PolicyMismatch { policy: String },
    #[error("no matching policy found")]
    NoPolicyMatch,
    #[error("missing auth context `{0}`")]
    MissingAuthContext(&'static str),
    #[error("missing request parameter `{0}`")]
    MissingRequestParam(String),
    #[error("template value `{0}` cannot be rendered as text")]
    NonScalarTemplateValue(String),
}

fn policy_matches_request(policy: &PolicyDefinition, request: &GatewayRequest) -> bool {
    if policy.match_rule.engine != engine_name(request) {
        return false;
    }

    match engine_name(request) {
        "sql" => sql_policy_matches(policy, request),
        "mongo" => mongo_policy_matches(policy, request),
        "redis" => redis_policy_matches(policy, request),
        "mqtt" => stream_policy_matches(policy, request),
        "kafka" => stream_policy_matches(policy, request),
        "mq" => queue_policy_matches(policy, request),
        _ => false,
    }
}

fn engine_name(request: &GatewayRequest) -> &'static str {
    match request.engine {
        bladb_core::protocol::Engine::Sql => "sql",
        bladb_core::protocol::Engine::Mongo => "mongo",
        bladb_core::protocol::Engine::Redis => "redis",
        bladb_core::protocol::Engine::Mqtt => "mqtt",
        bladb_core::protocol::Engine::Kafka => "kafka",
        bladb_core::protocol::Engine::Mq => "mq",
    }
}

fn sql_policy_matches(policy: &PolicyDefinition, request: &GatewayRequest) -> bool {
    let statement = match request.body.statement.as_deref() {
        Some(statement) => statement,
        None => return false,
    };

    let operation = sql_operation(statement);
    if policy.match_rule.operation.as_deref() != Some(operation) {
        return false;
    }

    if policy.match_rule.tables.is_empty() {
        return true;
    }

    let tables = sql_tables(statement);
    policy
        .match_rule
        .tables
        .iter()
        .all(|table| tables.iter().any(|candidate| candidate == table))
}

fn mongo_policy_matches(policy: &PolicyDefinition, request: &GatewayRequest) -> bool {
    policy.match_rule.collection.as_deref() == request.body.collection.as_deref()
        && policy.match_rule.action.as_deref() == Some(request.action.as_str())
}

fn redis_policy_matches(policy: &PolicyDefinition, request: &GatewayRequest) -> bool {
    policy.match_rule.command.as_deref() == Some(request.action.as_str())
}

fn stream_policy_matches(policy: &PolicyDefinition, request: &GatewayRequest) -> bool {
    policy.match_rule.action.as_deref() == Some(request.action.as_str())
}

fn queue_policy_matches(policy: &PolicyDefinition, request: &GatewayRequest) -> bool {
    policy.match_rule.action.as_deref() == Some(request.action.as_str())
}

fn sql_operation(statement: &str) -> &'static str {
    match statement
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "insert" => "insert",
        "update" => "update",
        "delete" => "delete",
        _ => "select",
    }
}

fn sql_tables(statement: &str) -> Vec<String> {
    let lower = statement.to_ascii_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();

    if tokens.is_empty() {
        return vec![];
    }

    match tokens[0] {
        "select" => tokens
            .windows(2)
            .find(|window| window[0] == "from")
            .map(|window| vec![sanitize_identifier(window[1])])
            .unwrap_or_default(),
        "insert" => tokens
            .windows(2)
            .find(|window| window[0] == "into")
            .map(|window| vec![sanitize_identifier(window[1])])
            .unwrap_or_default(),
        "update" => tokens
            .get(1)
            .map(|table| vec![sanitize_identifier(table)])
            .unwrap_or_default(),
        "delete" => tokens
            .windows(2)
            .find(|window| window[0] == "from")
            .map(|window| vec![sanitize_identifier(window[1])])
            .unwrap_or_default(),
        _ => vec![],
    }
}

fn sanitize_identifier(raw: &str) -> String {
    raw.trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .to_string()
}

fn resolve_map(
    map: &Map<String, Value>,
    auth: &AuthContext,
    params: &BTreeMap<String, Value>,
) -> Result<Map<String, Value>, GatewayError> {
    map.iter()
        .map(|(key, value)| Ok((key.clone(), resolve_value(value, auth, params)?)))
        .collect()
}

fn resolve_value(
    value: &Value,
    auth: &AuthContext,
    params: &BTreeMap<String, Value>,
) -> Result<Value, GatewayError> {
    match value {
        Value::Array(values) => values
            .iter()
            .map(|entry| resolve_value(entry, auth, params))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(object) if is_context_binding(object) => {
            resolve_context_binding(object, auth)
        }
        Value::Object(object) if is_template_binding(object) => {
            resolve_template_binding(object, auth, params).map(Value::String)
        }
        Value::Object(object) => resolve_map(object, auth, params).map(Value::Object),
        scalar => Ok(scalar.clone()),
    }
}

fn is_context_binding(object: &Map<String, Value>) -> bool {
    object.get("$ctx").is_some() && object.get("token").is_some()
}

fn is_template_binding(object: &Map<String, Value>) -> bool {
    object.get("$template") == Some(&Value::String("key".into()))
        && object.get("parts").is_some()
        && object.get("values").is_some()
}

fn resolve_context_binding(
    object: &Map<String, Value>,
    auth: &AuthContext,
) -> Result<Value, GatewayError> {
    let ctx = object
        .get("$ctx")
        .and_then(Value::as_str)
        .ok_or(GatewayError::MissingAuthContext("ctx"))?;

    match ctx {
        "uid" => auth
            .uid
            .clone()
            .map(Value::String)
            .ok_or(GatewayError::MissingAuthContext("uid")),
        "tenantId" => auth
            .tenant_id
            .clone()
            .map(Value::String)
            .ok_or(GatewayError::MissingAuthContext("tenantId")),
        "roles" => Ok(Value::Array(
            auth.roles.iter().cloned().map(Value::String).collect(),
        )),
        "permissionVersion" => auth
            .permission_version
            .clone()
            .map(Value::String)
            .ok_or(GatewayError::MissingAuthContext("permissionVersion")),
        other => Err(GatewayError::MissingAuthContext(match other {
            _ => "unknown",
        })),
    }
}

fn resolve_template_binding(
    object: &Map<String, Value>,
    auth: &AuthContext,
    params: &BTreeMap<String, Value>,
) -> Result<String, GatewayError> {
    let parts = object
        .get("parts")
        .and_then(Value::as_array)
        .ok_or(GatewayError::NonScalarTemplateValue("parts".into()))?;
    let values = object
        .get("values")
        .and_then(Value::as_array)
        .ok_or(GatewayError::NonScalarTemplateValue("values".into()))?;

    let mut rendered = String::new();

    for (index, part) in parts.iter().enumerate() {
        rendered.push_str(
            part.as_str()
                .ok_or_else(|| GatewayError::NonScalarTemplateValue("parts".into()))?,
        );

        if let Some(value) = values.get(index) {
            let resolved = resolve_value(value, auth, params)?;
            rendered.push_str(&render_scalar(&resolved, params)?);
        }
    }

    Ok(rendered)
}

fn render_scalar(value: &Value, params: &BTreeMap<String, Value>) -> Result<String, GatewayError> {
    match value {
        Value::String(string) => {
            if let Some(stripped) = string
                .strip_prefix("{args.")
                .and_then(|s| s.strip_suffix('}'))
            {
                let param = params
                    .get(stripped)
                    .ok_or_else(|| GatewayError::MissingRequestParam(stripped.to_string()))?;
                return render_scalar(param, params);
            }

            Ok(string.clone())
        }
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(boolean) => Ok(boolean.to_string()),
        Value::Null => Ok("null".into()),
        _ => Err(GatewayError::NonScalarTemplateValue(value.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::{sql_tables, AuthContext, Gateway, GatewayError};
    use bladb_core::protocol::GatewayRequest;
    use serde_json::json;

    #[test]
    fn authorizes_flash_sale_select_request_from_real_policy_fixture() {
        let yaml =
            include_str!("../../../apps/examples/flash-sale/policies/flash-sale.policy.yaml");
        let gateway = Gateway::from_yaml(yaml).expect("gateway from flash-sale policy");
        let request: GatewayRequest = serde_json::from_value(json!({
            "kind": "query",
            "engine": "sql",
            "action": "select",
            "meta": {
                "policy": "flashsale.orders.read-mine",
                "resource": "orders.readMine"
            },
            "statement": "select id, status from orders where uid = ? and sku = ?",
            "values": [
                { "$ctx": "uid", "token": "UID" },
                "camera-pro"
            ]
        }))
        .expect("parse flash-sale request");

        let authorization = gateway.authorize(&request).expect("authorize request");
        assert_eq!(authorization.policy_name, "flashsale.orders.read-mine");
    }

    #[test]
    fn authorizes_iot_mqtt_publish_request() {
        let yaml =
            include_str!("../../../apps/examples/iot-realtime/policies/iot-realtime.policy.yaml");
        let gateway = Gateway::from_yaml(yaml).expect("gateway from iot policy");
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
                    "device-001"
                ]
            },
            "payload": {
                "action": "reboot",
                "issuedBy": { "$ctx": "uid", "token": "UID" }
            }
        }))
        .expect("parse iot mqtt request");

        let authorization = gateway.authorize(&request).expect("authorize mqtt request");
        assert_eq!(authorization.policy_name, "iot.device-command.publish");
    }

    #[test]
    fn rejects_unknown_or_mismatched_policy_names() {
        let yaml =
            include_str!("../../../apps/examples/flash-sale/policies/flash-sale.policy.yaml");
        let gateway = Gateway::from_yaml(yaml).expect("gateway from flash-sale policy");

        let unknown: GatewayRequest = serde_json::from_value(json!({
            "kind": "query",
            "engine": "redis",
            "action": "get",
            "meta": {
                "policy": "missing.policy"
            },
            "name": "flashsale:camera-pro:stock"
        }))
        .expect("parse unknown policy request");
        let mismatched: GatewayRequest = serde_json::from_value(json!({
            "kind": "query",
            "engine": "redis",
            "action": "get",
            "meta": {
                "policy": "flashsale.orders.read-mine"
            },
            "name": "flashsale:camera-pro:stock"
        }))
        .expect("parse mismatched policy request");

        assert_eq!(
            gateway.authorize(&unknown),
            Err(GatewayError::UnknownPolicy("missing.policy".into()))
        );
        assert_eq!(
            gateway.authorize(&mismatched),
            Err(GatewayError::PolicyMismatch {
                policy: "flashsale.orders.read-mine".into()
            })
        );
    }

    #[test]
    fn infers_sql_tables_for_simple_statements() {
        assert_eq!(
            sql_tables("select * from orders where uid = ?"),
            vec!["orders"]
        );
        assert_eq!(
            sql_tables("insert into orders (uid) values (?)"),
            vec!["orders"]
        );
        assert_eq!(sql_tables("update orders set status = ?"), vec!["orders"]);
        assert_eq!(
            sql_tables("delete from orders where id = ?"),
            vec!["orders"]
        );
    }

    #[test]
    fn prepares_requests_by_resolving_context_values_and_templates() {
        let yaml =
            include_str!("../../../apps/examples/iot-realtime/policies/iot-realtime.policy.yaml");
        let gateway = Gateway::from_yaml(yaml).expect("gateway from iot policy");
        let request: GatewayRequest = serde_json::from_value(json!({
            "kind": "stream",
            "engine": "mqtt",
            "action": "publish",
            "meta": {
                "policy": "iot.device-command.publish",
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
        .expect("parse prepared request");

        let prepared = gateway
            .prepare(
                &request,
                &AuthContext {
                    uid: Some("u_1001".into()),
                    tenant_id: Some("tenant_a".into()),
                    roles: vec!["operator".into()],
                    permission_version: Some("v1".into()),
                },
            )
            .expect("prepare request");

        assert_eq!(
            prepared.authorization.policy_name,
            "iot.device-command.publish"
        );
        assert_eq!(
            prepared.body.topic,
            Some(json!("tenant/tenant_a/devices/device-001/commands"))
        );
        assert_eq!(
            prepared.body.payload,
            Some(json!({
                "action": "reboot",
                "issuedBy": "u_1001"
            }))
        );
    }
}
