use crate::reserved::ReservedValue;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyManifest {
    pub auth: AuthConfig,
    pub policies: Vec<PolicyDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt: JwtConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JwtConfig {
    pub claims: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDefinition {
    pub name: String,
    #[serde(rename = "match")]
    pub match_rule: MatchRule,
    pub enforce: EnforceRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchRule {
    pub engine: String,
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub tables: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EnforceRule {
    #[serde(default)]
    pub query: Option<Map<String, Value>>,
    #[serde(default, rename = "where")]
    pub where_clause: Option<Map<String, Value>>,
    #[serde(default)]
    pub fields: Option<Map<String, Value>>,
    #[serde(default)]
    pub key: Option<KeyRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeyRule {
    #[serde(default)]
    pub exact: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub allow_prefixes: Vec<String>,
}

impl PolicyManifest {
    pub fn referenced_reserved_values(&self) -> BTreeSet<ReservedValue> {
        todo!("implemented after the first red test run")
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyManifestError {
    #[error("failed to parse policy manifest: {0}")]
    Parse(String),
    #[error("duplicate policy name `{0}`")]
    DuplicatePolicyName(String),
}

pub fn parse_policy_manifest(_yaml: &str) -> Result<PolicyManifest, PolicyManifestError> {
    todo!("implemented after the first red test run")
}

#[cfg(test)]
mod tests {
    use super::{parse_policy_manifest, PolicyManifestError};
    use crate::reserved::ReservedValue;
    use std::collections::BTreeSet;

    #[test]
    fn flash_sale_policy_manifest_parses_from_real_fixture() {
        let yaml = include_str!("../../../apps/examples/flash-sale/policies/flash-sale.policy.yaml");
        let manifest = parse_policy_manifest(yaml).expect("parse flash-sale policy");

        assert_eq!(manifest.policies.len(), 5);
        assert_eq!(manifest.policies[0].name, "flashsale.items.find");
        assert_eq!(manifest.policies[3].match_rule.engine, "sql");
        assert_eq!(manifest.policies[3].match_rule.tables, vec!["orders"]);
        assert_eq!(
            manifest
                .policies[2]
                .enforce
                .key
                .as_ref()
                .and_then(|key| key.exact.as_deref()),
            Some("{UID}_wallet")
        );
    }

    #[test]
    fn iot_policy_manifest_reports_referenced_reserved_values() {
        let yaml = include_str!("../../../apps/examples/iot-realtime/policies/iot-realtime.policy.yaml");
        let manifest = parse_policy_manifest(yaml).expect("parse iot policy");

        let expected = BTreeSet::from([
            ReservedValue::Uid,
            ReservedValue::TenantId,
            ReservedValue::Roles,
        ]);

        assert_eq!(manifest.referenced_reserved_values(), expected);
    }

    #[test]
    fn duplicate_policy_names_are_rejected() {
        let yaml = r#"
auth:
  jwt:
    claims:
      UID: uid
policies:
  - name: same-policy
    match:
      engine: redis
      command: get
    enforce:
      key:
        exact: "{UID}_wallet"
  - name: same-policy
    match:
      engine: sql
      operation: select
      tables: [orders]
    enforce:
      where:
        uid: "{UID}"
"#;

        let error = parse_policy_manifest(yaml).expect_err("expected duplicate policy names");
        assert_eq!(
            error,
            PolicyManifestError::DuplicatePolicyName("same-policy".into())
        );
    }
}
