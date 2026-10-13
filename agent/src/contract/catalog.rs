//! The shared operation catalog, embedded from `contracts/v1/catalog.json`.
//!
//! Embedding the catalog means the agent binary is self-contained and the
//! accepted protocol versions, operation list and parameter rules cannot drift
//! from the committed contract without a compile-time source change.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Raw catalog file contents, embedded at compile time.
pub const CATALOG_JSON: &str = include_str!("../../../contracts/v1/catalog.json");

#[derive(Debug, Clone, Deserialize)]
pub struct Catalog {
    #[serde(rename = "$schema")]
    #[allow(dead_code)]
    pub schema: Option<String>,
    pub protocol_versions: Vec<i64>,
    pub operations: BTreeMap<String, OperationDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OperationDefinition {
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub parameters: BTreeMap<String, ParameterRule>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub retry: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParameterRule {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub max: Option<usize>,
    #[serde(default)]
    pub values: Option<Vec<String>>,
    #[serde(default)]
    pub max_items: Option<usize>,
}

impl Catalog {
    /// Parse the embedded catalog. Used once at startup; failures indicate a
    /// build/contract mismatch rather than runtime input.
    pub fn embedded() -> Catalog {
        serde_json::from_str(CATALOG_JSON).expect("embedded contracts/v1/catalog.json must parse")
    }

    pub fn supports_protocol(&self, version: i64) -> bool {
        self.protocol_versions.contains(&version)
    }

    pub fn definition(&self, operation: &str) -> Option<&OperationDefinition> {
        self.operations.get(operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_matches_expected_shape() {
        let catalog = Catalog::embedded();
        assert_eq!(catalog.protocol_versions, vec![1]);
        assert!(catalog.definition("application.restart").is_some());
        assert!(catalog.definition("server.obliterate").is_none());
        assert_eq!(
            catalog
                .definition("application.deploy")
                .and_then(|definition| definition.resource.as_deref()),
            Some("application")
        );
    }
}
