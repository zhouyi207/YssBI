mod catalog;
mod common;
mod editor;
mod execution;
mod history;
mod resources;
mod results;

pub use catalog::{get_compatible_node_catalog, get_localized_node_catalog};
pub use editor::{export_graph_subgraph, hydrate_editor_graph, mutate_graph_document};
pub use execution::{
    PinPreviewGenerationDto, allocate_pin_preview_generation, cancel_graph_run,
    execute_graph_document,
};
pub use history::{get_project_history_status, redo_graph_document, undo_graph_document};
pub use resources::{
    create_event, create_function, duplicate_graph, remove_graph, rename_graph_resource,
    save_project_graph, unload_project_graph, update_function_signature,
};
pub use results::{
    get_pin_result_history, get_result_descriptor, get_result_page, get_result_value,
};

#[allow(unused_imports)]
pub(crate) use editor::{export_graph_subgraph_from_state, mutate_graph_document_with_emitter};
#[allow(unused_imports)]
pub(crate) use results::result_value_to_json;

#[cfg(test)]
use catalog::get_localized_node_catalog_from_state;
#[cfg(test)]
use common::{mutation_conflict_to_command_error, parse_opaque_u64};
#[cfg(test)]
use editor::{hydrate_editor_graph_from_state, parse_editor_mutation_request};
#[cfg(test)]
use execution::{
    execution_channel_command_error, execution_channel_event_dto, execution_command_error,
};
#[cfg(test)]
use history::{
    get_project_history_status_from_state, redo_graph_document_with_emitter,
    undo_graph_document_with_emitter,
};
#[cfg(test)]
use resources::{
    create_graph_resource_with_emitter, duplicate_graph_resource_with_emitter,
    remove_graph_resource_with_emitter, rename_graph_resource_with_emitter,
    save_project_graph_with_emitter, update_function_signature_with_emitter,
};
#[cfg(test)]
use results::{
    MAX_INLINE_RESULT_JSON_BYTES, get_pin_result_history_from_state,
    get_result_descriptor_from_state, get_result_page_from_state, get_result_value_from_state,
};

#[cfg(test)]
#[path = "command_node_system_reroute_tests.rs"]
mod command_node_system_reroute_tests;
#[cfg(test)]
#[path = "command_node_system_subgraph_tests.rs"]
mod command_node_system_subgraph_tests;
#[cfg(test)]
#[path = "command_node_system/tests.rs"]
mod tests;
