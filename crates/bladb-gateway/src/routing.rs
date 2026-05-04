use crate::{AuthContext, Authorization, PreparedRequest};
use bladb_core::{
    cluster::{
        ModuleCategory, ModuleClusterDefinition, RoutingStrategy, TopologyManifest,
        TopologyManifestError,
    },
    protocol::GatewayRequest,
};
use serde_json::{json, Value};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSelection {
    pub cluster: String,
    pub category: ModuleCategory,
    pub runtime: String,
    pub service: String,
    pub namespace: Option<String>,
    pub route_key: Option<String>,
    pub shard: Option<u16>,
    pub sticky: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoutedRequest {
    pub authorization: Authorization,
    pub route: RouteSelection,
    pub body: bladb_core::protocol::RequestBody,
}

#[derive(Debug, Clone)]
pub struct ModuleRegistry {
    manifest: TopologyManifest,
    policy_index: HashMap<String, usize>,
}

impl ModuleRegistry {
    pub fn new(manifest: TopologyManifest) -> Self {
        let policy_index = manifest
            .module_clusters
            .iter()
            .enumerate()
            .flat_map(|(index, cluster)| {
                cluster
                    .policies
                    .iter()
                    .map(move |policy| (policy.clone(), index))
            })
            .collect();

        Self {
            manifest,
            policy_index,
        }
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, ModuleRegistryInitError> {
        let manifest = bladb_core::cluster::parse_topology_manifest(yaml)?;
        Ok(Self::new(manifest))
    }

    pub fn resolve(
        &self,
        authorization: &Authorization,
        request: &GatewayRequest,
        prepared: &PreparedRequest,
        auth: &AuthContext,
    ) -> Result<RouteSelection, RouteError> {
        let cluster = self
            .policy_index
            .get(&authorization.policy_name)
            .and_then(|index| self.manifest.module_clusters.get(*index))
            .ok_or_else(|| RouteError::UnmappedPolicy(authorization.policy_name.clone()))?;

        let engine = engine_name(request);
        if cluster.runtime != engine {
            return Err(RouteError::RuntimeMismatch {
                cluster: cluster.name.clone(),
                engine: engine.into(),
                runtime: cluster.runtime.clone(),
            });
        }

        let route_key = derive_route_key(cluster, request, prepared, auth)?;
        let shard = match (&cluster.routing.strategy, route_key.as_deref()) {
            (RoutingStrategy::Hash { virtual_shards }, Some(route_key)) => {
                Some(route_to_shard(route_key, *virtual_shards))
            }
            _ => None,
        };

        Ok(RouteSelection {
            cluster: cluster.name.clone(),
            category: cluster.category.clone(),
            runtime: cluster.runtime.clone(),
            service: cluster.discovery.service.clone(),
            namespace: cluster.discovery.namespace.clone(),
            route_key,
            shard,
            sticky: cluster.routing.sticky,
        })
    }

    pub fn clusters(&self) -> &[ModuleClusterDefinition] {
        &self.manifest.module_clusters
    }
}

#[derive(Debug, Error)]
pub enum ModuleRegistryInitError {
    #[error(transparent)]
    Topology(#[from] TopologyManifestError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteError {
    #[error("policy `{0}` is not mapped to a module cluster")]
    UnmappedPolicy(String),
    #[error("request engine `{engine}` does not match cluster `{cluster}` runtime `{runtime}`")]
    RuntimeMismatch {
        cluster: String,
        engine: String,
        runtime: String,
    },
    #[error("route field `{field}` is missing for cluster `{cluster}`")]
    MissingRouteField { cluster: String, field: String },
    #[error("route field `{field}` is not scalar for cluster `{cluster}`")]
    NonScalarRouteField { cluster: String, field: String },
}

pub fn route_prepared_request(
    registry: &ModuleRegistry,
    request: &GatewayRequest,
    prepared: PreparedRequest,
    auth: &AuthContext,
) -> Result<RoutedRequest, RouteError> {
    let authorization = prepared.authorization.clone();
    let route = registry.resolve(&authorization, request, &prepared, auth)?;

    Ok(RoutedRequest {
        authorization,
        route,
        body: prepared.body,
    })
}

fn derive_route_key(
    cluster: &ModuleClusterDefinition,
    request: &GatewayRequest,
    prepared: &PreparedRequest,
    auth: &AuthContext,
) -> Result<Option<String>, RouteError> {
    if cluster.routing.route_by.is_empty() {
        return Ok(None);
    }

    let context = json!({
        "actor": {
            "uid": auth.uid.clone(),
            "tenantId": auth.tenant_id.clone(),
            "roles": auth.roles.clone(),
            "permissionVersion": auth.permission_version.clone(),
        },
        "request": request,
        "prepared": {
            "body": prepared.body.clone(),
        },
        "event": prepared.body.payload.clone(),
    });

    let mut values = Vec::with_capacity(cluster.routing.route_by.len());
    for field in &cluster.routing.route_by {
        let value = read_path(&context, field).ok_or_else(|| RouteError::MissingRouteField {
            cluster: cluster.name.clone(),
            field: field.clone(),
        })?;
        let rendered = render_scalar(value).ok_or_else(|| RouteError::NonScalarRouteField {
            cluster: cluster.name.clone(),
            field: field.clone(),
        })?;
        values.push(rendered);
    }

    Ok(Some(values.join(":")))
}

fn read_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(value, |current, segment| match current {
            Value::Object(object) => object.get(segment),
            _ => None,
        })
}

fn render_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => Some("null".into()),
        _ => None,
    }
}

fn route_to_shard(route_key: &str, virtual_shards: u16) -> u16 {
    let mut hasher = DefaultHasher::new();
    route_key.hash(&mut hasher);
    (hasher.finish() % u64::from(virtual_shards)) as u16
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

#[cfg(test)]
mod tests {
    use super::{route_prepared_request, AuthContext, ModuleRegistry, RouteError};
    use crate::Gateway;
    use bladb_core::protocol::GatewayRequest;
    use serde_json::json;

    #[test]
    fn routes_flash_sale_stock_request_to_redis_cluster() {
        let policy_yaml =
            include_str!("../../../apps/examples/flash-sale/policies/flash-sale.policy.yaml");
        let topology_yaml =
            include_str!("../../../apps/examples/flash-sale/topology/flash-sale.topology.yaml");
        let gateway = Gateway::from_yaml(policy_yaml).expect("flash-sale gateway");
        let registry = ModuleRegistry::from_yaml(topology_yaml).expect("flash-sale topology");
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
        .expect("parse stock request");

        let auth = AuthContext {
            uid: Some("u_2001".into()),
            tenant_id: Some("tenant_flashsale".into()),
            roles: vec!["buyer".into()],
            permission_version: Some("v1".into()),
        };
        let prepared = gateway
            .prepare(&request, &auth)
            .expect("prepare stock request");
        let routed = route_prepared_request(&registry, &request, prepared, &auth)
            .expect("route stock request");

        assert_eq!(routed.route.cluster, "flashsale.stock-redis");
        assert_eq!(routed.route.runtime, "redis");
        assert_eq!(routed.route.service, "bladb-module-stock");
        assert_eq!(routed.route.route_key.as_deref(), Some("camera-pro"));
        assert!(routed.route.shard.is_some());
    }

    #[test]
    fn routes_iot_command_request_to_mqtt_cluster() {
        let policy_yaml =
            include_str!("../../../apps/examples/iot-realtime/policies/iot-realtime.policy.yaml");
        let topology_yaml =
            include_str!("../../../apps/examples/iot-realtime/topology/iot-realtime.topology.yaml");
        let gateway = Gateway::from_yaml(policy_yaml).expect("iot gateway");
        let registry = ModuleRegistry::from_yaml(topology_yaml).expect("iot topology");
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
        .expect("parse iot request");

        let auth = AuthContext {
            uid: Some("u_1001".into()),
            tenant_id: Some("tenant_a".into()),
            roles: vec!["operator".into()],
            permission_version: Some("v1".into()),
        };
        let prepared = gateway
            .prepare(&request, &auth)
            .expect("prepare iot request");
        let routed = route_prepared_request(&registry, &request, prepared, &auth)
            .expect("route iot request");

        assert_eq!(routed.route.cluster, "iot.commands-mqtt");
        assert_eq!(
            routed.route.route_key.as_deref(),
            Some("tenant_a:device-001")
        );
        assert!(routed.route.sticky);
    }

    #[test]
    fn rejects_policy_without_cluster_mapping() {
        let topology_yaml = r#"
moduleClusters:
  - name: only-one
    category: data
    runtime: redis
    policies: [flashsale.stock.read]
    discovery:
      kind: service
      service: redis-a
    routing:
      strategy:
        kind: single
    consistency:
      reads: primary
      writes: primary
    failover:
      mode: retrySameNode
      maxAttempts: 1
"#;
        let policy_yaml =
            include_str!("../../../apps/examples/flash-sale/policies/flash-sale.policy.yaml");
        let gateway = Gateway::from_yaml(policy_yaml).expect("flash-sale gateway");
        let registry = ModuleRegistry::from_yaml(topology_yaml).expect("topology");
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
            "name": "flashsale:camera-pro:stock",
            "amount": 1
        }))
        .expect("parse request");

        let auth = AuthContext {
            uid: Some("u_2001".into()),
            tenant_id: Some("tenant_flashsale".into()),
            roles: vec!["buyer".into()],
            permission_version: Some("v1".into()),
        };
        let prepared = gateway.prepare(&request, &auth).expect("prepare request");
        let error = route_prepared_request(&registry, &request, prepared, &auth)
            .expect_err("expected route error");

        assert_eq!(
            error,
            RouteError::UnmappedPolicy("flashsale.stock.decr".into())
        );
    }
}
