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
    #[serde(default)]
    pub topic: Option<KeyRule>,
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
        let mut reserved = BTreeSet::new();

        for token in self.auth.jwt.claims.keys() {
            if let Some(value) = ReservedValue::parse(token) {
                reserved.insert(value);
            }
        }

        for policy in &self.policies {
            if let Some(query) = &policy.enforce.query {
                collect_reserved_from_map(query, &mut reserved);
            }

            if let Some(where_clause) = &policy.enforce.where_clause {
                collect_reserved_from_map(where_clause, &mut reserved);
            }

            if let Some(fields) = &policy.enforce.fields {
                collect_reserved_from_map(fields, &mut reserved);
            }

            if let Some(key) = &policy.enforce.key {
                if let Some(exact) = &key.exact {
                    collect_reserved_from_string(exact, &mut reserved);
                }

                if let Some(template) = &key.template {
                    collect_reserved_from_string(template, &mut reserved);
                }

                for prefix in &key.allow_prefixes {
                    collect_reserved_from_string(prefix, &mut reserved);
                }
            }

            if let Some(topic) = &policy.enforce.topic {
                if let Some(exact) = &topic.exact {
                    collect_reserved_from_string(exact, &mut reserved);
                }

                if let Some(template) = &topic.template {
                    collect_reserved_from_string(template, &mut reserved);
                }

                for prefix in &topic.allow_prefixes {
                    collect_reserved_from_string(prefix, &mut reserved);
                }
            }
        }

        reserved
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyManifestError {
    #[error("failed to parse policy manifest: {0}")]
    Parse(String),
    #[error("duplicate policy name `{0}`")]
    DuplicatePolicyName(String),
}

pub fn parse_policy_manifest(yaml: &str) -> Result<PolicyManifest, PolicyManifestError> {
    let manifest: PolicyManifest = serde_yaml::from_str(yaml)
        .map_err(|error| PolicyManifestError::Parse(error.to_string()))?;

    let mut seen = HashSet::new();
    for policy in &manifest.policies {
        if !seen.insert(policy.name.clone()) {
            return Err(PolicyManifestError::DuplicatePolicyName(
                policy.name.clone(),
            ));
        }
    }

    Ok(manifest)
}

fn collect_reserved_from_map(map: &Map<String, Value>, reserved: &mut BTreeSet<ReservedValue>) {
    for value in map.values() {
        collect_reserved_from_value(value, reserved);
    }
}

fn collect_reserved_from_value(value: &Value, reserved: &mut BTreeSet<ReservedValue>) {
    match value {
        Value::String(string) => collect_reserved_from_string(string, reserved),
        Value::Array(values) => {
            for entry in values {
                collect_reserved_from_value(entry, reserved);
            }
        }
        Value::Object(object) => collect_reserved_from_map(object, reserved),
        _ => {}
    }
}

fn collect_reserved_from_string(input: &str, reserved: &mut BTreeSet<ReservedValue>) {
    for candidate in [
        ReservedValue::Uid,
        ReservedValue::TenantId,
        ReservedValue::Roles,
        ReservedValue::PermissionVersion,
    ] {
        let token = format!("{{{}}}", candidate.token());
        if input.contains(&token) {
            reserved.insert(candidate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_policy_manifest, PolicyManifestError};
    use crate::reserved::ReservedValue;
    use std::collections::BTreeSet;

    #[test]
    fn flash_sale_policy_manifest_parses_from_real_fixture() {
        let yaml =
            include_str!("../../../apps/examples/flash-sale/policies/flash-sale.policy.yaml");
        let manifest = parse_policy_manifest(yaml).expect("parse flash-sale policy");

        assert_eq!(manifest.policies.len(), 6);
        assert_eq!(manifest.policies[0].name, "flashsale.items.find");
        assert_eq!(manifest.policies[4].match_rule.engine, "sql");
        assert_eq!(manifest.policies[4].match_rule.tables, vec!["orders"]);
        assert_eq!(
            manifest.policies[3]
                .enforce
                .key
                .as_ref()
                .and_then(|key| key.exact.as_deref()),
            Some("{UID}_wallet")
        );
    }

    #[test]
    fn iot_policy_manifest_reports_referenced_reserved_values() {
        let yaml =
            include_str!("../../../apps/examples/iot-realtime/policies/iot-realtime.policy.yaml");
        let manifest = parse_policy_manifest(yaml).expect("parse iot policy");

        let expected = BTreeSet::from([
            ReservedValue::Uid,
            ReservedValue::TenantId,
            ReservedValue::Roles,
        ]);

        assert_eq!(manifest.referenced_reserved_values(), expected);
        assert_eq!(manifest.policies[3].match_rule.engine, "mqtt");
        assert_eq!(
            manifest.policies[3]
                .enforce
                .topic
                .as_ref()
                .and_then(|topic| topic.template.as_deref()),
            Some("tenant/{TENANT_ID}/devices/{args.deviceId}/commands")
        );
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
