use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorContext {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub event_id: String,
    pub event_type: String,
    pub source: String,
    pub trace_id: String,
    pub actor: ActorContext,
    pub payload: Value,
}

impl EventEnvelope {
    pub fn idempotency_key(&self) -> &str {
        &self.event_id
    }
}

#[cfg(test)]
mod tests {
    use super::{ActorContext, EventEnvelope};
    use serde_json::json;

    #[test]
    fn event_envelope_round_trips_and_uses_event_id_for_idempotency() {
        let envelope = EventEnvelope {
            event_id: "evt_01".into(),
            event_type: "order.created".into(),
            source: "sql.orders".into(),
            trace_id: "trace_01".into(),
            actor: ActorContext {
                kind: "user".into(),
                uid: Some("u_1001".into()),
                tenant_id: Some("tenant_a".into()),
                roles: vec!["buyer".into()],
                worker: None,
            },
            payload: json!({
                "orderId": "ord_01",
                "sku": "camera-pro"
            }),
        };

        let serialized = serde_json::to_value(&envelope).expect("serialize event");
        assert_eq!(serialized["eventId"], "evt_01");
        assert_eq!(serialized["eventType"], "order.created");
        assert_eq!(serialized["actor"]["tenantId"], "tenant_a");

        let round_trip: EventEnvelope =
            serde_json::from_value(serialized).expect("deserialize event");
        assert_eq!(round_trip, envelope);
        assert_eq!(round_trip.idempotency_key(), "evt_01");
    }
}
