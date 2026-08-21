mod delta;
mod diagnostics;
mod function_editor;
mod graph_builder;
mod parameter_editors;
mod port_projection;
mod types;

#[cfg(test)]
mod tests;

use diagnostics::{diagnostic_belongs_to_node, project_diagnostic};
use parameter_editors::{
    inherited_statistics_parameter_value, project_parameter_editor, project_schema_aware_editor,
    statistics_parameter_options,
};
use port_projection::{
    can_remove_port, project_address, project_connection_capability,
    project_effective_input_binding, project_instance_kind, project_node_capabilities,
    project_schema_summary, project_type_summary, relational_scalar_type_dto,
};

pub use delta::{GraphProjectionDelta, ProjectionError};
pub(crate) use function_editor::resolve_function_data_type;
pub use function_editor::{
    FunctionEditorPinDto, FunctionEditorProjectionDto, build_function_editor_projection,
};
pub use graph_builder::build_editor_graph_projection;
pub(crate) use port_projection::project_data_type;
pub use types::*;
