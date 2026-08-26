mod graph_operations;
mod operation_ledger;
mod rename_operations;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub(crate) use operation_ledger::ResourceOperationLedger;
pub(crate) use operation_ledger::ResourceOperationReservation;
pub(crate) use rename_operations::{remap_graph_document_references, remap_variable_scope_path};

#[cfg(test)]
pub(crate) use test_support::{ResourceMutationTestHook, ResourceMutationTestPoint};

#[cfg(test)]
fn fixture_result_path(
    result: &crate::event::ResourceMutationResultDto,
) -> Option<crate::graph_document::GraphResourcePath> {
    let paths = match &result.projection_status {
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths,
        } => expected_graph_paths,
        crate::event::ProjectionStatusDto::Incomplete {
            invalidated_graph_paths,
        } => invalidated_graph_paths,
    };
    paths
        .iter()
        .find(|path| path.starts_with("events/") || path.starts_with("functions/"))
        .and_then(|path| crate::graph_document::GraphResourcePath::new(path.clone()).ok())
}
