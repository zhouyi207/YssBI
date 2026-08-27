use super::localization::{Aliases, Text};
use super::*;
use crate::graph_document::{
    DocumentNode, FunctionParameterId, GraphDocument, GraphResourcePath, NodeId, NodePosition,
};
use crate::node_system::analysis::{
    DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity,
    EditorGraphProjectionDto, LocalizationLookup, NodeDiagnostic, ResourceKey, ResourceVersion,
};
use crate::node_system::compiler::{
    COMPILER_DIAGNOSTIC_DEFINITIONS, CompileCancellationToken, CompilerDiagnosticDefinitionError,
    GraphCompiler, LoweredKernel, LoweringContext, ResourceSnapshot, ValidatedNodeConfig,
    build_builtin_interface_resolvers,
};
use crate::node_system::document::{FunctionDocument, FunctionParameter, FunctionSignature};

use crate::node_system::protocol::{
    ConnectionsPerPort, I18nKey, ManagedNodeRole, NodeInstanceDisplaySpec, NodeScope, NodeTypeId,
    OutputProduction, ParameterEditorSpec, ParameterKey, PortDirection, PortInstances, PortKind,
    ResourceDisplayKind, TypeExpr,
};
use crate::node_system::registry::{I18nManifest, ImplementationKind, StructuralNodeRole};
use crate::node_system::runtime::build_builtin_kernel_registry;

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as _;
use uuid::Uuid;

fn item<'a>(catalog: &'a LocalizedCatalog, id: &str) -> &'a LocalizedCatalogItemDto {
    catalog
        .items
        .iter()
        .find(|item| item.node_type_id.as_ref() == id)
        .unwrap()
}

struct EmptyResources;

impl ResourceSnapshot for EmptyResources {
    fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
        BTreeMap::new()
    }
}

fn editor_fixture() -> (
    GraphDocument,
    std::sync::Arc<crate::node_system::registry::NodeRegistry>,
    std::sync::Arc<BuiltinCatalog>,
) {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let node_id = NodeId::from_uuid(Uuid::from_u128(1));
    let node_type = NodeTypeId::new("yssbi.constant.bool").unwrap();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    (document, registry, catalog)
}

fn editor_projection(locale: &str) -> EditorGraphProjectionDto {
    let (document, registry, catalog) = editor_fixture();
    let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
        .compile(&document)
        .analysis;
    EditorGraphProjectionDto::from_sources(
        "functions/main",
        &analysis,
        &document,
        &registry,
        &catalog.localization(locale),
    )
    .unwrap()
}

mod builtins;
mod dynamic;
mod manifest;
mod validation;
