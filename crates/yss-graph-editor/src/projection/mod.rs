mod connections;
mod mapper;
mod model;

pub use connections::{
    ConnectionCandidate, ConnectionCandidates, ConnectionDecision, ConnectionIntent,
};
pub use mapper::{build_editor_projection, project_parameter_groups};
pub use model::{
    EditorColumnOption, EditorConnectionModel, EditorDiagnosticModel, EditorDiagnosticSeverity,
    EditorEffectiveInputBinding, EditorFilterColumnOption, EditorFilterLiteralType,
    EditorInputBinding, EditorNodeCapabilities, EditorNodeDisplay, EditorNodeModel,
    EditorParameterConfiguration, EditorParameterDisplay, EditorParameterGroupModel,
    EditorParameterModel, EditorPortConnectionCapabilities, EditorPortDisplay,
    EditorPortInstanceAdditionModel, EditorPortModel, EditorPortStatus, EditorPortTypeState,
    EditorProjectionBasis, EditorProjectionError, EditorProjectionInput, EditorProjectionModel,
    EditorResolutionOutcome, EditorResolutionStage, EditorSchemaField, EditorSchemaSummary,
    EditorSchemaSummaryKind, ParameterEditorKind,
};

#[cfg(test)]
mod tests;
