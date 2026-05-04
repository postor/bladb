use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RequestKind {
    Query,
    Command,
    Stream,
    Queue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    Sql,
    Mongo,
    Redis,
    Mqtt,
    Kafka,
    Mq,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestMeta {
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub policy: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequest {
    pub kind: RequestKind,
    pub engine: Engine,
    pub action: String,
    #[serde(default)]
    pub meta: RequestMeta,
    #[serde(flatten)]
    pub body: RequestBody,
}

impl GatewayRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match (&self.kind, &self.engine, self.action.as_str()) {
            (RequestKind::Query, Engine::Sql, "select") => {
                require(self.body.statement.is_some(), "statement")?;
            }
            (RequestKind::Command, Engine::Sql, "insert" | "update" | "delete") => {
                require(self.body.statement.is_some(), "statement")?;
            }
            (RequestKind::Query, Engine::Mongo, "find" | "findOne") => {
                require(self.body.collection.is_some(), "collection")?;
                require(self.body.query.is_some(), "query")?;
            }
            (RequestKind::Command, Engine::Mongo, "insertOne") => {
                require(self.body.collection.is_some(), "collection")?;
                require(self.body.document.is_some(), "document")?;
            }
            (RequestKind::Query, Engine::Redis, "get") => {
                require(self.body.name.is_some(), "name")?;
            }
            (RequestKind::Command, Engine::Redis, "set") => {
                require(self.body.name.is_some(), "name")?;
                require(self.body.value.is_some(), "value")?;
            }
            (RequestKind::Command, Engine::Redis, "incrby" | "decrby") => {
                require(self.body.name.is_some(), "name")?;
                require(self.body.amount.is_some(), "amount")?;
            }
            (RequestKind::Stream, Engine::Redis, "publish") => {
                require(self.body.channel.is_some(), "channel")?;
                require(self.body.payload.is_some(), "payload")?;
            }
            (RequestKind::Stream, Engine::Mqtt, "publish") => {
                require(self.body.topic.is_some(), "topic")?;
                require(self.body.payload.is_some(), "payload")?;
            }
            (RequestKind::Stream, Engine::Kafka, "produce") => {
                require(self.body.topic.is_some(), "topic")?;
                require(self.body.payload.is_some(), "payload")?;
            }
            (RequestKind::Queue, Engine::Mq, "publish") => {
                require(self.body.queue.is_some(), "queue")?;
                require(self.body.message.is_some(), "message")?;
            }
            (RequestKind::Queue, Engine::Mq, "publishDelayed") => {
                require(self.body.queue.is_some(), "queue")?;
                require(self.body.message.is_some(), "message")?;
                require(self.body.delay_ms.is_some(), "delayMs")?;
            }
            _ => {
                return Err(ProtocolError::UnsupportedOperation {
                    kind: self.kind.clone(),
                    engine: self.engine.clone(),
                    action: self.action.clone(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryOptions {
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestBody {
    #[serde(default)]
    pub statement: Option<String>,
    #[serde(default)]
    pub values: Vec<Value>,
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub query: Option<Map<String, Value>>,
    #[serde(default)]
    pub document: Option<Map<String, Value>>,
    #[serde(default)]
    pub options: Option<QueryOptions>,
    #[serde(default)]
    pub name: Option<Value>,
    #[serde(default)]
    pub channel: Option<Value>,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub amount: Option<i64>,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(default)]
    pub topic: Option<Value>,
    #[serde(default)]
    pub queue: Option<String>,
    #[serde(default)]
    pub message: Option<Value>,
    #[serde(default)]
    pub delay_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMeta {
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySuccess {
    pub ok: bool,
    pub data: Value,
    #[serde(default)]
    pub meta: ResponseMeta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    AuthExpired,
    PolicyDenied,
    InvalidRequest,
    RateLimited,
    OffsetConflict,
    AckTimeout,
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayFailure {
    pub ok: bool,
    pub code: ErrorCode,
    pub message: String,
    #[serde(default)]
    pub meta: ResponseMeta,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProtocolError {
    #[error("missing required field `{0}`")]
    MissingField(&'static str),
    #[error("unsupported operation: kind={kind:?} engine={engine:?} action={action}")]
    UnsupportedOperation {
        kind: RequestKind,
        engine: Engine,
        action: String,
    },
}

fn require(condition: bool, field: &'static str) -> Result<(), ProtocolError> {
    if condition {
        Ok(())
    } else {
        Err(ProtocolError::MissingField(field))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Engine, ErrorCode, GatewayFailure, GatewayRequest, GatewaySuccess, ProtocolError,
        RequestKind,
    };
    use serde_json::json;

    #[test]
    fn parses_sql_query_request_with_reserved_context_values() {
        let payload = json!({
            "kind": "query",
            "engine": "sql",
            "action": "select",
            "meta": {
                "resource": "orders.readMine",
                "policy": "flashsale.orders.read-mine",
                "traceId": "trace_01"
            },
            "statement": "select * from orders where uid = ? and sku = ?",
            "values": [
                { "$ctx": "uid", "token": "UID" },
                "camera-pro"
            ]
        });

        let request: GatewayRequest = serde_json::from_value(payload).expect("parse sql request");
        assert_eq!(request.kind, RequestKind::Query);
        assert_eq!(request.engine, Engine::Sql);
        assert_eq!(request.meta.resource.as_deref(), Some("orders.readMine"));
        assert_eq!(
            request.body.statement.as_deref(),
            Some("select * from orders where uid = ? and sku = ?")
        );
        assert_eq!(request.body.values.len(), 2);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn parses_stream_and_queue_requests() {
        let mqtt_payload = json!({
            "kind": "stream",
            "engine": "mqtt",
            "action": "publish",
            "meta": {
                "resource": "device.command",
                "params": {
                    "deviceId": "device-001"
                }
            },
            "topic": "tenant/a/devices/device-001/commands",
            "payload": {
                "action": "reboot"
            }
        });
        let mq_payload = json!({
            "kind": "queue",
            "engine": "mq",
            "action": "publishDelayed",
            "queue": "order.payment.timeout",
            "delayMs": 900000,
            "message": {
                "orderId": "ord_01"
            }
        });

        let mqtt_request: GatewayRequest =
            serde_json::from_value(mqtt_payload).expect("parse mqtt stream request");
        let mq_request: GatewayRequest =
            serde_json::from_value(mq_payload).expect("parse mq queue request");

        assert_eq!(mqtt_request.kind, RequestKind::Stream);
        assert_eq!(mqtt_request.engine, Engine::Mqtt);
        assert_eq!(
            mqtt_request.body.topic,
            Some(json!("tenant/a/devices/device-001/commands"))
        );
        assert_eq!(mq_request.kind, RequestKind::Queue);
        assert_eq!(mq_request.engine, Engine::Mq);
        assert_eq!(mq_request.body.delay_ms, Some(900000));
        assert!(mqtt_request.validate().is_ok());
        assert!(mq_request.validate().is_ok());
    }

    #[test]
    fn success_and_failure_responses_round_trip() {
        let success = GatewaySuccess {
            ok: true,
            data: json!([{ "id": "ord_01" }]),
            meta: super::ResponseMeta {
                trace_id: Some("trace_01".into()),
                cursor: None,
            },
        };
        let failure = GatewayFailure {
            ok: false,
            code: ErrorCode::PolicyDenied,
            message: "policy denied".into(),
            meta: super::ResponseMeta {
                trace_id: Some("trace_01".into()),
                cursor: None,
            },
        };

        let success_value = serde_json::to_value(&success).expect("serialize success");
        let failure_value = serde_json::to_value(&failure).expect("serialize failure");

        assert_eq!(success_value["ok"], true);
        assert_eq!(failure_value["code"], "POLICY_DENIED");

        let success_round_trip: GatewaySuccess =
            serde_json::from_value(success_value).expect("deserialize success");
        let failure_round_trip: GatewayFailure =
            serde_json::from_value(failure_value).expect("deserialize failure");

        assert_eq!(success_round_trip, success);
        assert_eq!(failure_round_trip, failure);
    }

    #[test]
    fn rejects_invalid_kind_engine_action_combinations_and_missing_fields() {
        let wrong_combo = GatewayRequest {
            kind: RequestKind::Query,
            engine: Engine::Mqtt,
            action: "publish".into(),
            meta: super::RequestMeta::default(),
            body: super::RequestBody {
                topic: Some(json!("tenant/a/devices/device-001/commands")),
                payload: Some(json!({ "action": "reboot" })),
                ..super::RequestBody::default()
            },
        };
        let missing_field = GatewayRequest {
            kind: RequestKind::Queue,
            engine: Engine::Mq,
            action: "publishDelayed".into(),
            meta: super::RequestMeta::default(),
            body: super::RequestBody {
                queue: Some("order.payment.timeout".into()),
                message: Some(json!({ "orderId": "ord_01" })),
                ..super::RequestBody::default()
            },
        };

        assert_eq!(
            wrong_combo.validate(),
            Err(ProtocolError::UnsupportedOperation {
                kind: RequestKind::Query,
                engine: Engine::Mqtt,
                action: "publish".into()
            })
        );
        assert_eq!(
            missing_field.validate(),
            Err(ProtocolError::MissingField("delayMs"))
        );
    }

    #[test]
    fn accepts_sql_command_actions_for_writes() {
        let payload = json!({
            "kind": "command",
            "engine": "sql",
            "action": "insert",
            "statement": "insert into orders (uid, sku) values (?, ?)",
            "values": [
                { "$ctx": "uid", "token": "UID" },
                "camera-pro"
            ]
        });

        let request: GatewayRequest = serde_json::from_value(payload).expect("parse sql insert");
        assert_eq!(request.kind, RequestKind::Command);
        assert_eq!(request.engine, Engine::Sql);
        assert_eq!(request.action, "insert");
        assert!(request.validate().is_ok());
    }
}
