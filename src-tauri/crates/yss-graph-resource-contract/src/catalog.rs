use crate::DataSchema;
use crate::{GraphDependencyKey, GraphDependencyManifest};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use yss_data_contract::DataType;
use yss_graph_document::{FunctionParameterId, GraphResourcePath};

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionParameterContract {
    id: FunctionParameterId,
    name: Box<str>,
    data_type: DataType,
}

impl FunctionParameterContract {
    pub fn new(id: FunctionParameterId, name: impl Into<Box<str>>, data_type: DataType) -> Self {
        Self {
            id,
            name: name.into(),
            data_type,
        }
    }

    pub fn id(&self) -> &FunctionParameterId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    parameters: Box<[FunctionParameterContract]>,
    result: Option<DataType>,
}

impl FunctionSignature {
    pub fn new(parameters: Vec<FunctionParameterContract>, result: Option<DataType>) -> Self {
        Self {
            parameters: parameters.into_boxed_slice(),
            result,
        }
    }

    pub fn parameters(&self) -> &[FunctionParameterContract] {
        &self.parameters
    }

    pub fn result(&self) -> Option<&DataType> {
        self.result.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionCatalogEntry {
    signature: FunctionSignature,
    document: Option<Arc<yss_graph_document::GraphDocument>>,
}

impl FunctionCatalogEntry {
    pub fn new(signature: FunctionSignature) -> Self {
        Self {
            signature,
            document: None,
        }
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
    reads: Option<Arc<Mutex<BTreeSet<GraphDependencyKey>>>>,
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
            reads: None,
        }
    }

    pub fn function_signature(&self, path: &GraphResourcePath) -> Option<&FunctionSignature> {
        self.record(GraphDependencyKey::Function(path.as_str().into()));
        self.functions
            .get(path)
            .map(FunctionCatalogEntry::signature)
    }

    pub fn with_function_document(
        mut self,
        path: &GraphResourcePath,
        document: yss_graph_document::GraphDocument,
    ) -> Self {
        if let Some(entry) = Arc::make_mut(&mut self.functions).get_mut(path) {
            entry.document = Some(Arc::new(document));
        }
        self
    }

    pub fn function_document(
        &self,
        path: &GraphResourcePath,
    ) -> Option<&yss_graph_document::GraphDocument> {
        self.record(GraphDependencyKey::FunctionBody(path.as_str().into()));
        self.functions.get(path)?.document.as_deref()
    }

    pub fn variable_contract(&self, resource: &GraphResourceId) -> Option<&VariableValueContract> {
        self.record(GraphDependencyKey::Variable(resource.as_str().into()));
        self.variables.get(resource)
    }

    pub fn database_schema(&self, resource: &GraphResourceId) -> Option<&DataSchema> {
        self.record(GraphDependencyKey::Database(resource.as_str().into()));
        self.databases.get(resource)
    }

    pub fn fingerprint(&self) -> &ResourceCatalogFingerprint {
        &self.fingerprint
    }

    /// Tracking is private to one resolve attempt; resource facts remain immutable.
    pub fn tracked(&self) -> Self {
        Self {
            reads: Some(Arc::new(Mutex::new(BTreeSet::new()))),
            ..self.clone()
        }
    }

    fn record(&self, key: GraphDependencyKey) {
        if let Some(reads) = &self.reads {
            reads
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(key);
        }
    }

    fn observed_fingerprint(&self, key: &GraphDependencyKey) -> Option<[u8; 32]> {
        let hash = match key {
            GraphDependencyKey::FunctionBody(identity) => {
                let document = self
                    .functions
                    .get(&GraphResourcePath::new(identity.as_ref()).ok()?)?
                    .document
                    .as_deref()?;
                yss_graph_document::semantic_document_fingerprint(document)
            }
            GraphDependencyKey::Function(identity) => {
                let entry = self
                    .functions
                    .get(&GraphResourcePath::new(identity.as_ref()).ok()?)?;
                let parameters = entry
                    .signature
                    .parameters()
                    .iter()
                    .map(|parameter| (parameter.id(), parameter.data_type()))
                    .collect::<Vec<_>>();
                yss_canonical_hash::hash_canonical(
                    "yssbi.function-contract.v1",
                    &(parameters, entry.signature.result()),
                )
            }
            GraphDependencyKey::Variable(identity) => yss_canonical_hash::hash_canonical(
                "yssbi.variable-contract.v1",
                self.variables
                    .get(&GraphResourceId::new(identity.clone()))?
                    .data_type(),
            ),
            GraphDependencyKey::Database(identity) => yss_canonical_hash::hash_canonical(
                "yssbi.database-schema.v1",
                self.databases
                    .get(&GraphResourceId::new(identity.clone()))?,
            ),
        };
        Some(hash.expect("resource contracts are canonically serializable"))
    }

    pub fn dependencies(&self) -> GraphDependencyManifest {
        let keys = self
            .reads
            .as_ref()
            .map(|reads| reads.lock().unwrap_or_else(PoisonError::into_inner).clone())
            .unwrap_or_default();
        GraphDependencyManifest(
            keys.into_iter()
                .map(|key| {
                    let observed = self.observed_fingerprint(&key);
                    (key, observed)
                })
                .collect(),
        )
    }

    pub fn matches_dependencies(&self, manifest: &GraphDependencyManifest) -> bool {
        manifest
            .entries()
            .iter()
            .all(|(key, expected)| &self.observed_fingerprint(key) == expected)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FunctionCatalogEntry, FunctionParameterContract, FunctionSignature, GraphResourceId,
        ResourceCatalogFingerprint, ResourceCatalogSnapshot, VariableValueContract,
    };
    use crate::{ColumnSchema, DataSchema};
    use std::collections::BTreeMap;
    use yss_data_contract::DataType;
    use yss_graph_document::GraphResourcePath;

    #[test]
    fn dependency_observations_ignore_unread_resources_and_recheck_absence() {
        let a = GraphResourceId::new("variables/a");
        let b = GraphResourceId::new("variables/b");
        let missing = GraphResourceId::new("variables/missing");
        let catalog = |a_type, b_type, include_missing| {
            let mut variables = BTreeMap::from([
                (a.clone(), VariableValueContract::new(a_type)),
                (b.clone(), VariableValueContract::new(b_type)),
            ]);
            if include_missing {
                variables.insert(missing.clone(), VariableValueContract::new(DataType::Int64));
            }
            ResourceCatalogSnapshot::new(
                BTreeMap::new(),
                variables,
                BTreeMap::new(),
                ResourceCatalogFingerprint::from_bytes([9; 32]),
            )
        };
        let tracked = catalog(DataType::Int64, DataType::Int64, false).tracked();
        assert!(tracked.variable_contract(&a).is_some());
        assert!(tracked.variable_contract(&missing).is_none());
        let dependencies = tracked.dependencies();
        assert_eq!(dependencies.entries().len(), 2);
        assert!(
            catalog(DataType::Int64, DataType::String, false).matches_dependencies(&dependencies)
        );
        assert!(
            !catalog(DataType::String, DataType::Int64, false).matches_dependencies(&dependencies)
        );
        assert!(
            !catalog(DataType::Int64, DataType::Int64, true).matches_dependencies(&dependencies)
        );
    }

    #[test]
    fn resource_catalog_exposes_only_graph_compile_contracts() {
        let function_path = GraphResourcePath::new("functions/Forecast.yssbi-function").unwrap();
        let variable_id = GraphResourceId::new("variables/forecast-input");
        let database_id = GraphResourceId::new("databases/forecast-source");
        let function = FunctionCatalogEntry::new(FunctionSignature::new(
            vec![FunctionParameterContract::new(
                yss_graph_document::FunctionParameterId::new("series"),
                "Series",
                DataType::DataSeries(Box::new(DataType::Float64)),
            )],
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
