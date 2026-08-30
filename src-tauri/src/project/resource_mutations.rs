mod operation_ledger;
#[cfg(test)]
mod rename_operations;
#[cfg(test)]
mod test_support;

pub(crate) use operation_ledger::ResourceOperationLedger;
pub(crate) use operation_ledger::ResourceOperationReservation;
#[cfg(test)]
pub(crate) use rename_operations::{remap_graph_document_references, remap_variable_scope_path};

#[cfg(test)]
pub(crate) use test_support::{ResourceMutationTestHook, ResourceMutationTestPoint};

#[cfg(test)]
fn fixture_result_path(
    result: &crate::schema::application_event::ResourceMutationResultDto,
) -> Option<yss_graph_document::GraphResourcePath> {
    let paths = match &result.projection_status {
        crate::schema::application_event::ProjectionStatusDto::Complete {
            expected_graph_paths,
        } => expected_graph_paths,
        crate::schema::application_event::ProjectionStatusDto::Incomplete {
            invalidated_graph_paths,
        } => invalidated_graph_paths,
    };
    paths
        .iter()
        .find(|path| path.starts_with("events/") || path.starts_with("functions/"))
        .and_then(|path| yss_graph_document::GraphResourcePath::new(path.clone()).ok())
}
