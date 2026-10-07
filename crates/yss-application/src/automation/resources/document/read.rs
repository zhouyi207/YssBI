use super::*;
use yss_harness_contract::model::DocumentReadInput;
use yss_project_model::doc::DocView;

pub(in crate::automation) fn read_document(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: DocumentReadRequest,
    control: &CapabilityControl,
) -> Result<DocumentReadResult> {
    let resource = request.input.document().resource();
    let snapshot = application
        .read_doc(session.project_instance_id().clone(), doc_path(&resource)?)
        .map_err(file_error)?;
    let version = ResourceVersion {
        revision: snapshot.version.revision.get(),
        session_id: Some(snapshot.version.session_id),
    };
    if request
        .version
        .as_ref()
        .is_some_and(|expected| expected != &version)
    {
        return Err(conflict());
    }
    let view = snapshot.content.view();
    let scope = if let Some(section) = request.input.section() {
        view.section(section.heading_index, section.start)
            .map_err(|_| {
                invalid("section").with_detail(
                    "nextStep",
                    "Inspect the current document outline and use its complete section reference.",
                )
            })?
    } else if let Some(range) = request.input.range() {
        range.start..range.end
    } else {
        0..view.character_count()
    };
    let content = match &request.input {
        DocumentReadInput::Outline(value) => {
            let selected = view
                .outline(scope, value.offset, value.limit)
                .map_err(|_| invalid("section"))?;
            let headings = selected
                .headings
                .into_iter()
                .map(|(index, heading)| DocumentHeading {
                    section: section_ref(&view, index),
                    title: heading.title.chars().take(256).collect(),
                    title_complete: heading.title.chars().count() <= 256,
                    level: heading.level,
                    parent_heading_index: heading.parent_index,
                    body_start: heading.body_start,
                    end: heading.end,
                })
                .collect::<Vec<_>>();
            DocumentReadContent::Outline {
                character_count: view.character_count(),
                heading_count: view.headings().len(),
                page: InspectionPage::known(value.offset, headings.len(), selected.total),
                headings,
            }
        }
        DocumentReadInput::Text(value) => {
            let selected = view
                .read(scope, value.offset, value.limit)
                .map_err(|_| invalid("range"))?;
            let complete = selected.range == selected.scope;
            DocumentReadContent::Text {
                page: InspectionPage::known(
                    selected.offset,
                    selected.range.len(),
                    selected.scope.len(),
                ),
                markdown: selected.markdown.to_owned(),
                range: character_range(selected.range),
                scope: character_range(selected.scope),
                complete,
                section: value.section.clone(),
            }
        }
        DocumentReadInput::Search(value) => {
            let selected = view
                .search(
                    scope,
                    &value.query,
                    value.offset,
                    value.limit,
                    value.context_characters,
                )
                .map_err(|_| invalid("range"))?;
            let matches = selected
                .matches
                .into_iter()
                .map(|found| DocumentSearchMatch {
                    range: character_range(found.range),
                    context: found.context.to_owned(),
                    context_range: character_range(found.context_range),
                    section: found.heading_index.map(|index| section_ref(&view, index)),
                })
                .collect::<Vec<_>>();
            DocumentReadContent::Search {
                page: InspectionPage::known(value.offset, matches.len(), selected.total),
                matches,
            }
        }
    };
    control.check()?;
    check_version(session, &resource, &version)?;
    fit_result(DocumentReadResult {
        document: request.input.document().clone(),
        version,
        dirty: snapshot.dirty,
        content,
    })
}

fn section_ref(view: &DocView<'_>, index: usize) -> DocumentSectionRef {
    DocumentSectionRef {
        heading_index: index,
        start: view.headings()[index].start,
    }
}

fn fit_result(value: DocumentReadResult) -> Result<DocumentReadResult> {
    let mut result = AutomationCapabilityResult::DocumentRead(value);
    while let Err(error) = result.validate_budget(MAX_CAPABILITY_RESULT_BYTES) {
        let AutomationCapabilityResult::DocumentRead(value) = &mut result else {
            unreachable!()
        };
        let DocumentReadContent::Search { matches, page } = &mut value.content else {
            return Err(error);
        };
        if error.code != CapabilityFailureCode::ResultTooLarge || matches.len() <= 1 {
            return Err(error);
        }
        matches.truncate(matches.len() / 2);
        *page = InspectionPage::known(page.offset, matches.len(), page.total.unwrap());
    }
    let AutomationCapabilityResult::DocumentRead(value) = result else {
        unreachable!()
    };
    Ok(value)
}
