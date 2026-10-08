use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidSemanticId {
    kind: &'static str,
    value: String,
    reason: &'static str,
}

impl InvalidSemanticId {
    fn new(kind: &'static str, value: &str, reason: &'static str) -> Self {
        Self {
            kind,
            value: value.to_string(),
            reason,
        }
    }
}

impl fmt::Display for InvalidSemanticId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {} '{}': {}", self.kind, self.value, self.reason)
    }
}

impl std::error::Error for InvalidSemanticId {}

macro_rules! semantic_id {
    ($name:ident, $kind:literal, $validate:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: impl Into<Box<str>>) -> Result<Self, InvalidSemanticId> {
                let value = value.into();
                ($validate)(&value, $kind)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                Self::new(Box::<str>::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = InvalidSemanticId;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
    };
}

fn validate_local(value: &str, kind: &'static str) -> Result<(), InvalidSemanticId> {
    validate_common(kind, value)?;
    if !valid_segment(value) {
        return Err(InvalidSemanticId::new(
            kind,
            value,
            "expected lowercase ASCII letters, digits, or underscores",
        ));
    }
    Ok(())
}

fn validate_namespaced(
    value: &str,
    kind: &'static str,
    minimum_segments: usize,
) -> Result<(), InvalidSemanticId> {
    validate_common(kind, value)?;
    let mut segments = value.split('.');
    if segments.clone().take(minimum_segments).count() < minimum_segments
        || segments.any(|segment| !valid_segment(segment))
    {
        return Err(InvalidSemanticId::new(
            kind,
            value,
            "expected dot-separated lowercase ASCII segments",
        ));
    }
    Ok(())
}

fn validate_common(kind: &'static str, value: &str) -> Result<(), InvalidSemanticId> {
    if value.is_empty() {
        return Err(InvalidSemanticId::new(kind, value, "value is empty"));
    }
    if value.trim() != value {
        return Err(InvalidSemanticId::new(
            kind,
            value,
            "leading or trailing whitespace is not allowed",
        ));
    }
    Ok(())
}

fn valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

semantic_id!(NodeTypeId, "node type id", |value, kind| {
    validate_namespaced(value, kind, 3)
});
semantic_id!(PortKey, "port key", validate_local);
semantic_id!(ParameterKey, "parameter key", validate_local);
semantic_id!(ParameterGroupKey, "parameter group key", validate_local);
semantic_id!(TypeParameterId, "type parameter id", validate_local);
semantic_id!(NodeCategoryId, "node category id", |value, kind| {
    validate_namespaced(value, kind, 1)
});
semantic_id!(I18nKey, "i18n key", |value, kind| validate_namespaced(
    value, kind, 2
));
semantic_id!(TypeId, "type id", |value, kind| validate_namespaced(
    value, kind, 2
));
semantic_id!(
    ProviderId,
    "provider id",
    |value, kind| validate_namespaced(value, kind, 1)
);
semantic_id!(IconId, "icon id", |value, kind| validate_namespaced(
    value, kind, 1
));
semantic_id!(NodeStyleId, "node style id", |value, kind| {
    validate_namespaced(value, kind, 1)
});
semantic_id!(TypeConstructorId, "type constructor id", |value, kind| {
    validate_namespaced(value, kind, 2)
});
semantic_id!(TypeClassId, "type class id", |value, kind| {
    validate_namespaced(value, kind, 2)
});
semantic_id!(
    InterfaceResolverId,
    "interface resolver id",
    |value, kind| validate_namespaced(value, kind, 2)
);
semantic_id!(SchemaResolverId, "schema resolver id", |value, kind| {
    validate_namespaced(value, kind, 2)
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_ids_round_trip_as_json_strings() {
        let id = TypeConstructorId::new("yssbi.data_series").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"yssbi.data_series\"");
        assert_eq!(
            serde_json::from_str::<TypeConstructorId>(&json).unwrap(),
            id
        );
    }

    #[test]
    fn semantic_ids_reject_display_text_and_whitespace() {
        assert!(NodeTypeId::new("Value:Constants:Int64").is_err());
        assert!(NodeTypeId::new(" yssbi.value.int64").is_err());
        assert!(PortKey::new("Result Value").is_err());
        assert!(ParameterKey::new("Display Name").is_err());
        assert!(TypeId::new("Float64").is_err());
        assert!(I18nKey::new("title").is_err());

        for invalid in ["", " yssbi.value.int64", "Value:Constants:Int64", "value"] {
            assert!(serde_json::from_value::<NodeTypeId>(serde_json::json!(invalid)).is_err());
        }
        for invalid in ["", "value ", "Result Value"] {
            assert!(serde_json::from_value::<PortKey>(serde_json::json!(invalid)).is_err());
        }
        assert!(
            serde_json::from_value::<crate::ParameterValues>(serde_json::json!({"bad key": 1}))
                .is_err()
        );
    }
}
