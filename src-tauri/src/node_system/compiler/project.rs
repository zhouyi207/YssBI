use super::dynamic_interface::{
    InterfaceResolver, InterfaceResolverError, InterfaceResolverMember, InterfaceResolverRequest,
    InterfaceResolverSet, SchemaFieldIdentityGuarantee,
};
use crate::node_system::document::{
    DynamicMemberLocator, FunctionDocument, FunctionParameterId, GraphResourcePath,
};
use crate::node_system::protocol::InterfaceResolverId;
use std::sync::Arc;

pub const FUNCTION_CALL_ARGUMENTS_RESOLVER: &str = "yssbi.project.function.call.arguments";
pub const FUNCTION_CALL_RESULTS_RESOLVER: &str = "yssbi.project.function.call.results";
pub const FUNCTION_ENTRY_PARAMETERS_RESOLVER: &str = "yssbi.project.function.entry.parameters";
pub const FUNCTION_RETURN_RESULTS_RESOLVER: &str = "yssbi.project.function.return.results";

pub fn builtin_function_interface_resolver_ids() -> Box<[InterfaceResolverId]> {
    [
        FUNCTION_CALL_ARGUMENTS_RESOLVER,
        FUNCTION_CALL_RESULTS_RESOLVER,
        FUNCTION_ENTRY_PARAMETERS_RESOLVER,
        FUNCTION_RETURN_RESULTS_RESOLVER,
    ]
    .map(resolver_id)
    .into()
}

pub fn build_builtin_interface_resolvers() -> InterfaceResolverSet {
    let mut resolvers = InterfaceResolverSet::new();
    for (id, projection) in [
        (
            FUNCTION_CALL_ARGUMENTS_RESOLVER,
            FunctionInterfaceProjection::Parameters,
        ),
        (
            FUNCTION_CALL_RESULTS_RESOLVER,
            FunctionInterfaceProjection::Result,
        ),
        (
            FUNCTION_ENTRY_PARAMETERS_RESOLVER,
            FunctionInterfaceProjection::Parameters,
        ),
        (
            FUNCTION_RETURN_RESULTS_RESOLVER,
            FunctionInterfaceProjection::Result,
        ),
    ] {
        resolvers
            .insert(
                resolver_id(id),
                Arc::new(FunctionInterfaceResolver { projection }),
            )
            .expect("built-in function resolver IDs are unique");
    }
    resolvers
}

#[derive(Clone, Copy)]
enum FunctionInterfaceProjection {
    Parameters,
    Result,
}

struct FunctionInterfaceResolver {
    projection: FunctionInterfaceProjection,
}

impl InterfaceResolver for FunctionInterfaceResolver {
    fn resolve(
        &self,
        request: InterfaceResolverRequest<'_>,
    ) -> Result<Box<[InterfaceResolverMember]>, InterfaceResolverError> {
        let function = function_path(&request)?;
        let resolved = request
            .resources
            .resolve_function(&function)
            .map_err(|error| InterfaceResolverError::from_resource(&error))?;
        let document = resolved.value.function.clone();
        Ok(match self.projection {
            FunctionInterfaceProjection::Parameters => {
                parameter_members(request.basis, &function, &document)
            }
            FunctionInterfaceProjection::Result => {
                result_members(request.basis, &function, &document)
            }
        })
    }
}

fn function_path(
    request: &InterfaceResolverRequest<'_>,
) -> Result<GraphResourcePath, InterfaceResolverError> {
    let node = request
        .document
        .nodes
        .get(&request.node_id)
        .ok_or_else(|| InterfaceResolverError::new("resolver node is missing from the document"))?;
    ["function", "target"]
        .into_iter()
        .find_map(|name| {
            node.parameters
                .iter()
                .find(|(key, _)| key.as_str() == name)
                .and_then(|(_, value)| value.as_str())
        })
        .filter(|path| !path.is_empty() && path.trim() == *path)
        .map(|path| GraphResourcePath(path.into()))
        .ok_or_else(|| {
            InterfaceResolverError::new(
                "function interface resolver requires a non-empty function or target parameter",
            )
        })
}

fn parameter_members(
    basis: &crate::node_system::analysis::CompilationBasis<
        crate::node_system::document::GraphRevision,
    >,
    function: &GraphResourcePath,
    document: &FunctionDocument,
) -> Box<[InterfaceResolverMember]> {
    document
        .signature
        .parameters
        .iter()
        .map(|parameter| InterfaceResolverMember {
            basis: basis.clone(),
            locator: DynamicMemberLocator::FunctionParameter {
                function: function.clone(),
                parameter: parameter.id.clone(),
            },
            label: parameter.name.clone(),
            identity: SchemaFieldIdentityGuarantee::Stable,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn result_members(
    basis: &crate::node_system::analysis::CompilationBasis<
        crate::node_system::document::GraphRevision,
    >,
    function: &GraphResourcePath,
    document: &FunctionDocument,
) -> Box<[InterfaceResolverMember]> {
    document
        .signature
        .return_type
        .as_ref()
        .map(|return_type| InterfaceResolverMember {
            basis: basis.clone(),
            locator: DynamicMemberLocator::FunctionParameter {
                function: function.clone(),
                parameter: FunctionParameterId("return".into()),
            },
            label: return_type.clone(),
            identity: SchemaFieldIdentityGuarantee::Stable,
        })
        .into_iter()
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn resolver_id(value: &str) -> InterfaceResolverId {
    InterfaceResolverId::new(value).expect("built-in resolver ID is valid")
}
