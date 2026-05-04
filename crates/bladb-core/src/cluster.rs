use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyManifest {
    pub module_clusters: Vec<ModuleClusterDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleClusterDefinition {
    pub name: String,
    pub category: ModuleCategory,
    pub runtime: String,
    #[serde(default)]
    pub policies: Vec<String>,
    pub discovery: DiscoveryConfig,
    #[serde(default)]
    pub transport: TransportConfig,
    pub routing: RoutingConfig,
    pub consistency: ConsistencyConfig,
    pub failover: FailoverConfig,
    #[serde(default)]
    pub deployment: DeploymentConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModuleCategory {
    Data,
    Stream,
    Queue,
    Worker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryConfig {
    pub kind: DiscoveryKind,
    pub service: String,
    #[serde(default)]
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscoveryKind {
    Static,
    Service,
    Registry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportConfig {
    pub protocol: TransportProtocol,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub queue_group: Option<String>,
    #[serde(default)]
    pub stream: Option<String>,
    #[serde(default)]
    pub durable: Option<String>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            protocol: TransportProtocol::Direct,
            subject: None,
            queue_group: None,
            stream: None,
            durable: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransportProtocol {
    Direct,
    NatsService,
    JetStream,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingConfig {
    pub strategy: RoutingStrategy,
    #[serde(default)]
    pub route_by: Vec<String>,
    #[serde(default)]
    pub sticky: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RoutingStrategy {
    Single,
    Hash {
        #[serde(rename = "virtualShards")]
        virtual_shards: u16,
    },
    Broadcast,
}

impl RoutingStrategy {
    pub fn is_partitioned(&self) -> bool {
        matches!(self, Self::Hash { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyConfig {
    pub reads: ConsistencyMode,
    pub writes: ConsistencyMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsistencyMode {
    Primary,
    ReplicaPreferred,
    Quorum,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverConfig {
    pub mode: FailoverMode,
    pub max_attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FailoverMode {
    None,
    RetrySameNode,
    RetryOtherReplica,
    RetryOtherZone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentConfig {
    #[serde(default = "default_replicas")]
    pub replicas: u16,
    #[serde(default)]
    pub min_ready_seconds: Option<u32>,
    #[serde(default)]
    pub rolling: RollingUpdateConfig,
    #[serde(default)]
    pub autoscale: Option<AutoscaleConfig>,
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            replicas: default_replicas(),
            min_ready_seconds: None,
            rolling: RollingUpdateConfig::default(),
            autoscale: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollingUpdateConfig {
    #[serde(default = "default_max_unavailable")]
    pub max_unavailable: String,
    #[serde(default = "default_max_surge")]
    pub max_surge: String,
}

impl Default for RollingUpdateConfig {
    fn default() -> Self {
        Self {
            max_unavailable: default_max_unavailable(),
            max_surge: default_max_surge(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoscaleConfig {
    pub min_replicas: u16,
    pub max_replicas: u16,
    #[serde(default)]
    pub target_cpu_utilization: Option<u8>,
    #[serde(default)]
    pub target_queue_depth: Option<u32>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TopologyManifestError {
    #[error("failed to parse topology manifest: {0}")]
    Parse(String),
    #[error("duplicate module cluster name `{0}`")]
    DuplicateClusterName(String),
    #[error("policy `{policy}` is assigned to more than one cluster: `{first}` and `{second}`")]
    DuplicatePolicyAssignment {
        policy: String,
        first: String,
        second: String,
    },
    #[error("invalid routing config for cluster `{cluster}`: {reason}")]
    InvalidRouting { cluster: String, reason: String },
    #[error("invalid transport config for cluster `{cluster}`: {reason}")]
    InvalidTransport { cluster: String, reason: String },
    #[error("invalid deployment config for cluster `{cluster}`: {reason}")]
    InvalidDeployment { cluster: String, reason: String },
}

pub fn parse_topology_manifest(yaml: &str) -> Result<TopologyManifest, TopologyManifestError> {
    let manifest: TopologyManifest = serde_yaml::from_str(yaml)
        .map_err(|error| TopologyManifestError::Parse(error.to_string()))?;

    let mut seen = HashSet::new();
    let mut policy_index = HashMap::new();
    for cluster in &manifest.module_clusters {
        if !seen.insert(cluster.name.clone()) {
            return Err(TopologyManifestError::DuplicateClusterName(
                cluster.name.clone(),
            ));
        }

        for policy in &cluster.policies {
            if let Some(previous) = policy_index.insert(policy.clone(), cluster.name.clone()) {
                return Err(TopologyManifestError::DuplicatePolicyAssignment {
                    policy: policy.clone(),
                    first: previous,
                    second: cluster.name.clone(),
                });
            }
        }

        validate_routing(cluster)?;
        validate_transport(cluster)?;
        validate_deployment(cluster)?;
    }

    Ok(manifest)
}

fn validate_transport(cluster: &ModuleClusterDefinition) -> Result<(), TopologyManifestError> {
    match cluster.transport.protocol {
        TransportProtocol::Direct => Ok(()),
        TransportProtocol::NatsService => {
            if cluster.transport.subject.is_none() {
                return Err(TopologyManifestError::InvalidTransport {
                    cluster: cluster.name.clone(),
                    reason: "natsService transport requires subject".into(),
                });
            }

            Ok(())
        }
        TransportProtocol::JetStream => {
            if cluster.transport.stream.is_none() {
                return Err(TopologyManifestError::InvalidTransport {
                    cluster: cluster.name.clone(),
                    reason: "jetStream transport requires stream".into(),
                });
            }

            if cluster.transport.subject.is_none() {
                return Err(TopologyManifestError::InvalidTransport {
                    cluster: cluster.name.clone(),
                    reason: "jetStream transport requires subject".into(),
                });
            }

            Ok(())
        }
    }
}

fn validate_deployment(cluster: &ModuleClusterDefinition) -> Result<(), TopologyManifestError> {
    if cluster.deployment.replicas == 0 {
        return Err(TopologyManifestError::InvalidDeployment {
            cluster: cluster.name.clone(),
            reason: "replicas must be greater than 0".into(),
        });
    }

    if let Some(autoscale) = &cluster.deployment.autoscale {
        if autoscale.min_replicas == 0 {
            return Err(TopologyManifestError::InvalidDeployment {
                cluster: cluster.name.clone(),
                reason: "autoscale minReplicas must be greater than 0".into(),
            });
        }

        if autoscale.max_replicas < autoscale.min_replicas {
            return Err(TopologyManifestError::InvalidDeployment {
                cluster: cluster.name.clone(),
                reason: "autoscale maxReplicas must be greater than or equal to minReplicas"
                    .into(),
            });
        }
    }

    Ok(())
}

fn default_replicas() -> u16 {
    1
}

fn default_max_unavailable() -> String {
    "25%".into()
}

fn default_max_surge() -> String {
    "25%".into()
}

fn validate_routing(cluster: &ModuleClusterDefinition) -> Result<(), TopologyManifestError> {
    match &cluster.routing.strategy {
        RoutingStrategy::Single | RoutingStrategy::Broadcast => Ok(()),
        RoutingStrategy::Hash { virtual_shards } => {
            if *virtual_shards == 0 {
                return Err(TopologyManifestError::InvalidRouting {
                    cluster: cluster.name.clone(),
                    reason: "virtualShards must be greater than 0".into(),
                });
            }

            if cluster.routing.route_by.is_empty() {
                return Err(TopologyManifestError::InvalidRouting {
                    cluster: cluster.name.clone(),
                    reason: "hash routing requires at least one routeBy field".into(),
                });
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_topology_manifest, ConsistencyMode, DiscoveryKind, ModuleCategory, RoutingStrategy,
        TransportProtocol,
        TopologyManifestError,
    };

    #[test]
    fn flash_sale_topology_parses_from_real_fixture() {
        let yaml =
            include_str!("../../../apps/examples/flash-sale/topology/flash-sale.topology.yaml");
        let manifest = parse_topology_manifest(yaml).expect("parse flash-sale topology");

        assert_eq!(manifest.module_clusters.len(), 4);
        assert_eq!(manifest.module_clusters[0].category, ModuleCategory::Data);
        assert_eq!(
            manifest.module_clusters[0].policies,
            vec!["flashsale.items.find".to_string()]
        );
        assert_eq!(manifest.module_clusters[1].runtime, "sql");
        assert_eq!(
            manifest.module_clusters[1].policies,
            vec![
                "flashsale.orders.read-mine".to_string(),
                "flashsale.orders.create".to_string(),
            ]
        );
        assert_eq!(manifest.module_clusters[2].runtime, "redis");
        assert_eq!(
            manifest.module_clusters[2].discovery.kind,
            DiscoveryKind::Service
        );
        assert_eq!(
            manifest.module_clusters[2].routing.strategy,
            RoutingStrategy::Hash {
                virtual_shards: 128,
            }
        );
        assert_eq!(
            manifest.module_clusters[1].transport.protocol,
            TransportProtocol::NatsService
        );
        assert_eq!(
            manifest.module_clusters[1].transport.subject.as_deref(),
            Some("rpc.flashsale.orders")
        );
        assert_eq!(manifest.module_clusters[1].deployment.replicas, 2);
        assert_eq!(
            manifest.module_clusters[3].consistency.writes,
            ConsistencyMode::Local
        );
        assert_eq!(
            manifest.module_clusters[3].transport.protocol,
            TransportProtocol::JetStream
        );
    }

    #[test]
    fn iot_topology_preserves_hash_route_fields() {
        let yaml =
            include_str!("../../../apps/examples/iot-realtime/topology/iot-realtime.topology.yaml");
        let manifest = parse_topology_manifest(yaml).expect("parse iot topology");

        assert_eq!(manifest.module_clusters.len(), 5);
        assert_eq!(
            manifest.module_clusters[2].policies,
            vec!["iot.device-command.publish".to_string()]
        );
        assert_eq!(
            manifest.module_clusters[3].routing.route_by,
            vec![
                "event.actor.tenantId".to_string(),
                "event.payload.deviceId".to_string(),
            ]
        );
        assert_eq!(
            manifest.module_clusters[2].transport.protocol,
            TransportProtocol::NatsService
        );
        assert_eq!(
            manifest.module_clusters[3].transport.stream.as_deref(),
            Some("BLADB_IOT_EVENTS")
        );
        assert!(manifest.module_clusters[3]
            .routing
            .strategy
            .is_partitioned());
    }

    #[test]
    fn nats_service_transport_requires_subject() {
        let yaml = r#"
moduleClusters:
  - name: bad-cluster
    category: data
    runtime: redis
    policies: [flashsale.stock.read]
    discovery:
      kind: service
      service: redis-a
    transport:
      protocol: natsService
      queueGroup: bladb.stock
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

        let error = parse_topology_manifest(yaml).expect_err("expected invalid transport");
        assert_eq!(
            error,
            TopologyManifestError::InvalidTransport {
                cluster: "bad-cluster".into(),
                reason: "natsService transport requires subject".into(),
            }
        );
    }

    #[test]
    fn jetstream_transport_requires_stream() {
        let yaml = r#"
moduleClusters:
  - name: bad-worker
    category: worker
    runtime: worker
    policies: []
    discovery:
      kind: service
      service: worker-a
    transport:
      protocol: jetStream
      subject: events.flashsale.>
    routing:
      strategy:
        kind: single
    consistency:
      reads: local
      writes: local
    failover:
      mode: retryOtherZone
      maxAttempts: 1
"#;

        let error = parse_topology_manifest(yaml).expect_err("expected invalid transport");
        assert_eq!(
            error,
            TopologyManifestError::InvalidTransport {
                cluster: "bad-worker".into(),
                reason: "jetStream transport requires stream".into(),
            }
        );
    }

    #[test]
    fn duplicate_cluster_names_are_rejected() {
        let yaml = r#"
moduleClusters:
  - name: same-cluster
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
  - name: same-cluster
    category: worker
    runtime: worker
    policies: [flashsale.workflow.run]
    discovery:
      kind: service
      service: worker-a
    routing:
      strategy:
        kind: single
    consistency:
      reads: local
      writes: local
    failover:
      mode: none
      maxAttempts: 0
"#;

        let error = parse_topology_manifest(yaml).expect_err("expected duplicate cluster name");
        assert_eq!(
            error,
            TopologyManifestError::DuplicateClusterName("same-cluster".into())
        );
    }

    #[test]
    fn duplicate_policy_assignments_are_rejected() {
        let yaml = r#"
moduleClusters:
  - name: cluster-a
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
  - name: cluster-b
    category: data
    runtime: redis
    policies: [flashsale.stock.read]
    discovery:
      kind: service
      service: redis-b
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

        let error =
            parse_topology_manifest(yaml).expect_err("expected duplicate policy assignment");
        assert_eq!(
            error,
            TopologyManifestError::DuplicatePolicyAssignment {
                policy: "flashsale.stock.read".into(),
                first: "cluster-a".into(),
                second: "cluster-b".into(),
            }
        );
    }

    #[test]
    fn hash_routing_requires_route_keys() {
        let yaml = r#"
moduleClusters:
  - name: bad-cluster
    category: data
    runtime: redis
    policies: [flashsale.stock.read]
    discovery:
      kind: service
      service: redis-a
    routing:
      strategy:
        kind: hash
        virtualShards: 64
    consistency:
      reads: primary
      writes: primary
    failover:
      mode: retrySameNode
      maxAttempts: 1
"#;

        let error = parse_topology_manifest(yaml).expect_err("expected invalid routing");
        assert_eq!(
            error,
            TopologyManifestError::InvalidRouting {
                cluster: "bad-cluster".into(),
                reason: "hash routing requires at least one routeBy field".into(),
            }
        );
    }
}
