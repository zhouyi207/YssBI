//! Markdown tool adapters over the original document snapshot and File Edit/Save lifecycle.
use super::*;
use yss_harness_contract::model::{DocumentCharacterRange, DocumentSectionRef};

mod read;
mod text;
pub(in crate::automation) use read::read_document;

pub(super) fn edit_document(
    application: &ApplicationState,
    session: &ApplicationSession,
    resource: &ProjectResourceRef,
    version: &ResourceVersion,
    edit: ResourceEdit,
    control: &CapabilityControl,
) -> Result<(CommittedResourceMutation, DocumentEditReceipt)> {
    let project = session.project_instance_id().clone();
    let path = doc_path(resource)?;
    let snapshot = application
        .read_doc(project.clone(), path.clone())
        .map_err(file_error)?;
    if snapshot.version != file_version(version)? {
        return Err(conflict());
    }
    let (markdown, changes) = text::apply(snapshot.content.0, edit, control)?;
    let character_count = markdown.chars().count();
    control.check()?;
    let result = application
        .apply_doc_command(
            project,
            OperationId::new(),
            FileCommand::Edit {
                path,
                version: snapshot.version,
                edits: vec![yss_project_model::doc::DocEdit::SetMarkdown { markdown }],
            },
        )
        .map_err(file_error)?;
    let dirty = result.snapshot.as_ref().ok_or_else(unavailable)?.dirty;
    Ok((
        result.mutation,
        DocumentEditReceipt {
            changes,
            character_count,
            dirty,
        },
    ))
}

fn character_range(range: std::ops::Range<usize>) -> DocumentCharacterRange {
    DocumentCharacterRange {
        start: range.start,
        end: range.end,
    }
}
