#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReservedValue {
    Uid,
    TenantId,
    Roles,
    PermissionVersion,
}

impl ReservedValue {
    pub fn token(&self) -> &'static str {
        match self {
            Self::Uid => "UID",
            Self::TenantId => "TENANT_ID",
            Self::Roles => "ROLES",
            Self::PermissionVersion => "PERMISSION_VERSION",
        }
    }

    pub fn claim_key(&self) -> &'static str {
        match self {
            Self::Uid => "uid",
            Self::TenantId => "tenantId",
            Self::Roles => "roles",
            Self::PermissionVersion => "permissionVersion",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "UID" => Some(Self::Uid),
            "TENANT_ID" => Some(Self::TenantId),
            "ROLES" => Some(Self::Roles),
            "PERMISSION_VERSION" => Some(Self::PermissionVersion),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ReservedValue;

    #[test]
    fn reserved_values_expose_stable_tokens_and_claim_keys() {
        let cases = [
            (ReservedValue::Uid, "UID", "uid"),
            (ReservedValue::TenantId, "TENANT_ID", "tenantId"),
            (ReservedValue::Roles, "ROLES", "roles"),
            (
                ReservedValue::PermissionVersion,
                "PERMISSION_VERSION",
                "permissionVersion",
            ),
        ];

        for (value, token, claim_key) in cases {
            assert_eq!(value.token(), token);
            assert_eq!(value.claim_key(), claim_key);
            assert_eq!(ReservedValue::parse(token), Some(value));
        }
    }

    #[test]
    fn unknown_reserved_value_token_is_rejected() {
        assert_eq!(ReservedValue::parse("ACCOUNT_ID"), None);
    }
}
