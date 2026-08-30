use super::schema::DataSchema;
use crate::graph_document::GraphResourcePath;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use yss_data_contract::DataType;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GraphResourceId(Box<str>);

impl GraphResourceId {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GraphResourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    parameters: Box<[DataType]>,
    result: Option<DataType>,
}

impl FunctionSignature {
    pub fn new(parameters: Vec<DataType>, result: Option<DataType>) -> Self {
        Self {
            parameters: parameters.into_boxed_slice(),
            result,
        }
    }

    pub fn parameters(&self) -> &[DataType] {
        &self.parameters
    }

    pub fn result(&self) -> Option<&DataType> {
        self.result.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCatalogEntry {
    signature: FunctionSignature,
}

impl FunctionCatalogEntry {
    pub fn new(signature: FunctionSignature) -> Self {
        Self { signature }
    }

    pub fn signature(&self) -> &FunctionSignature {
        &self.signature
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableValueContract {
    data_type: DataType,
}

impl VariableValueContract {
    pub fn new(data_type: DataType) -> Self {
        Self { data_type }
    }

    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceCatalogFingerprint([u8; 32]);

impl ResourceCatalogFingerprint {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct ResourceCatalogSnapshot {
    functions: Arc<BTreeMap<GraphResourcePath, FunctionCatalogEntry>>,
    variables: Arc<BTreeMap<GraphResourceId, VariableValueContract>>,
    databases: Arc<BTreeMap<GraphResourceId, DataSchema>>,
    fingerprint: ResourceCatalogFingerprint,
}

impl ResourceCatalogSnapshot {
    pub fn new(
        functions: BTreeMap<GraphResourcePath, FunctionCatalogEntry>,
        variables: BTreeMap<GraphResourceId, VariableValueContract>,
        databases: BTreeMap<GraphResourceId, DataSchema>,
        fingerprint: ResourceCatalogFingerprint,
    ) -> Self {
        Self {
            functions: Arc::new(functions),
            variables: Arc::new(variables),
            databases: Arc::new(databases),
            fingerprint,
        }
    }

    pub fn function_signature(&self, path: &GraphResourcePath) -> Option<&FunctionSignature> {
        self.functions
            .get(path)
            .map(FunctionCatalogEntry::signature)
    }

    pub fn variable_contract(&self, resource: &GraphResourceId) -> Option<&VariableValueContract> {
        self.variables.get(resource)
    }

    pub fn database_schema(&self, resource: &GraphResourceId) -> Option<&DataSchema> {
        self.databases.get(resource)
    }

    pub fn fingerprint(&self) -> &ResourceCatalogFingerprint {
        &self.fingerprint
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FunctionCatalogEntry, FunctionSignature, GraphResourceId, ResourceCatalogFingerprint,
        ResourceCatalogSnapshot, VariableValueContract,
    };
    use crate::graph::schema::{ColumnSchema, DataSchema};
    use crate::graph_document::GraphResourcePath;
    use std::collections::BTreeMap;
    use yss_data_contract::DataType;

    #[test]
    fn resource_catalog_exposes_only_graph_compile_contracts() {
        let function_path = GraphResourcePath::new("functions/Forecast.yssbi-function").unwrap();
        let variable_id = GraphResourceId::new("variables/forecast-input");
        let database_id = GraphResourceId::new("databases/forecast-source");
        let function = FunctionCatalogEntry::new(FunctionSignature::new(
            vec![DataType::DataSeries(Box::new(DataType::Float64))],
            Some(DataType::Float64),
        ));
        let variable = VariableValueContract::new(DataType::Float64);
        let database = DataSchema {
            columns: vec![ColumnSchema {
                name: "sales".to_owned(),
                data_type: DataType::Float64,
            }],
        };
        let fingerprint = ResourceCatalogFingerprint::from_bytes([7; 32]);
        let catalog = ResourceCatalogSnapshot::new(
            BTreeMap::from([(function_path.clone(), function.clone())]),
            BTreeMap::from([(variable_id.clone(), variable.clone())]),
            BTreeMap::from([(database_id.clone(), database.clone())]),
            fingerprint,
        );

        assert_eq!(
            catalog.function_signature(&function_path),
            Some(function.signature())
        );
        assert_eq!(catalog.variable_contract(&variable_id), Some(&variable));
        assert_eq!(catalog.database_schema(&database_id), Some(&database));
        assert_eq!(catalog.fingerprint(), &fingerprint);
    }
}
