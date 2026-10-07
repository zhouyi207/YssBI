//! Call-local type and schema binding. A specialization never mutates the function document.
use crate::{
    GraphFunctionSemanticFact, GraphFunctionState, GraphSchemaObservations, GraphSchemaState,
    GraphSemanticCache,
};
use std::collections::BTreeMap;
use yss_graph_document::{FunctionParameterId, GraphResourcePath, PortAddress};
use yss_graph_resource_contract::ResourceCatalogSnapshot;
use yss_node_protocol::ResolvedType;
use yss_node_registry::NodeRegistry;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFunctionArgument {
    pub value_type: ResolvedType,
    pub schema: GraphSchemaState,
}

pub(crate) type BoundFunctionPorts = BTreeMap<PortAddress, GraphFunctionArgument>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphFunctionBindingError {
    DefinitionUnavailable,
    ArgumentSetMismatch,
    ArgumentTypeMismatch { parameter: FunctionParameterId },
}

/// Bind signature identities to the existing Entry ABI, then run the ordinary resolver.
/// Runtime observations belong to this invocation; callers must not use the definition's results.
pub fn specialize_function(
    path: &GraphResourcePath,
    definition: &GraphFunctionSemanticFact,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    arguments: &BTreeMap<FunctionParameterId, GraphFunctionArgument>,
    observations: &GraphSchemaObservations,
    cache: &mut GraphSemanticCache,
) -> Result<GraphFunctionSemanticFact, GraphFunctionBindingError> {
    let document = resources
        .function_document(path)
        .ok_or(GraphFunctionBindingError::DefinitionUnavailable)?;
    let signature = resources
        .function_signature(path)
        .ok_or(GraphFunctionBindingError::DefinitionUnavailable)?;
    if arguments.len() != signature.parameters().len()
        || definition.abi.parameters.len() != arguments.len()
    {
        return Err(GraphFunctionBindingError::ArgumentSetMismatch);
    }
    let mut bound = BoundFunctionPorts::new();
    let mut abi = definition.abi.clone();
    for (parameter, declared) in abi.parameters.iter_mut().zip(signature.parameters()) {
        if &parameter.id != declared.id() {
            return Err(GraphFunctionBindingError::ArgumentSetMismatch);
        }
        let argument = arguments
            .get(&parameter.id)
            .ok_or(GraphFunctionBindingError::ArgumentSetMismatch)?;
        if yss_graph_type_mapping::data_type_from_resolved_type(&argument.value_type).as_ref()
            != Some(declared.data_type())
        {
            return Err(GraphFunctionBindingError::ArgumentTypeMismatch {
                parameter: parameter.id.clone(),
            });
        }
        parameter.value_type = argument.value_type.clone();
        bound.insert(parameter.entry_output.clone(), argument.clone());
    }
    let semantics = crate::resolution::resolve_graph_semantics_inner(
        document,
        registry,
        resources,
        cache,
        observations,
        &bound,
        true,
    );
    if let Some(result) = &mut abi.result
        && let Some(actual) = semantics
            .concrete_interface()
            .port(&result.return_input)
            .and_then(|port| port.type_state.exact())
    {
        result.value_type = actual.clone();
    }
    let state = if semantics.ready().is_some() {
        GraphFunctionState::Ready
    } else if permits_unbound_schema(&semantics) {
        GraphFunctionState::AwaitingSchema
    } else {
        GraphFunctionState::Invalid
    };
    Ok(GraphFunctionSemanticFact {
        abi,
        semantics,
        state,
    })
}

/// Unknown fields are legitimate in a definition; missing inputs, invalid parameters,
/// missing resources and known missing columns remain definition errors.
pub(crate) fn permits_unbound_schema(semantics: &crate::GraphSemanticSnapshot) -> bool {
    !matches!(
        semantics.outcome(),
        crate::GraphResolutionOutcome::InternalFailure { .. }
    ) && semantics
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.blocking)
        .all(|diagnostic| {
            if diagnostic.code.as_str()
                != yss_graph_diagnostics::GraphDiagnosticKind::InterfaceSchemaDependencyUnresolved
                    .code()
            {
                return false;
            }
            let crate::GraphDiagnosticLocation::Port(address) = &diagnostic.primary else {
                return false;
            };
            semantics
                .concrete_interface()
                .port(address)
                .is_some_and(|port| {
                    matches!(
                        port.schema_state,
                        GraphSchemaState::Deferred
                            | GraphSchemaState::Pending(
                                crate::GraphSchemaIssue::UnresolvedUpstream
                            )
                    )
                })
        })
}

#[cfg(test)]
mod tests;
