//! Constant projections and typed intentions over the same staged Graph document.
use super::*;
use yss_data_contract::{DataValue, TabularScalar};
use yss_graph_document::{ConstantId, GraphConstant};

pub(super) fn parse_id(id: &str) -> Result<ConstantId, CapabilityFailure> {
    uuid::Uuid::parse_str(id)
        .map(ConstantId::from_uuid)
        .map_err(|_| invalid_edit_identity("constantId"))
}

fn summary(constant: &GraphConstant) -> ConstantSummary {
    let preview = if let Some(table) = &constant.tabular {
        format!(
            "{} rows, {} columns",
            table.row_count(),
            table.columns().len()
        )
    } else {
        match &constant.data_value {
            DataValue::Null => "null".into(),
            DataValue::Bool(value) => value.to_string(),
            DataValue::Integer(value) => value.to_string(),
            DataValue::Unsigned(value) => value.to_string(),
            DataValue::Decimal(value) => value.as_str().into(),
            DataValue::String(value) => value.chars().take(120).collect(),
            DataValue::Bytes(values) => format!("{} bytes", values.len()),
            DataValue::List(values) => format!("{} items", values.len()),
            DataValue::Object(values) => format!("{} fields", values.len()),
        }
    };
    ConstantSummary {
        constant_id: constant.id.to_string(),
        name: constant.name.clone(),
        data_type: constant.data_type.clone(),
        preview,
        row_count: constant.tabular.as_ref().map(|table| table.row_count()),
        column_count: constant.tabular.as_ref().map(|table| table.columns().len()),
    }
}

pub(super) fn find(
    request: FindConstantsRequest,
    path: &GraphResourcePath,
    document: &GraphDocument,
    projection: &EditorProjectionModel,
    version: ResourceVersion,
    hash: String,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    let mut result = inspection::queries::base(
        &request,
        &request.graph,
        path,
        document,
        projection,
        version,
        hash,
    )?;
    let query = request.query.as_deref().unwrap_or("").to_lowercase();
    let constants = document
        .constants
        .values()
        .filter(|value| value.name.to_lowercase().contains(&query))
        .collect::<Vec<_>>();
    let offset = request.offset.min(constants.len());
    let items = constants
        .iter()
        .skip(offset)
        .take(request.limit)
        .map(|value| summary(value))
        .collect::<Vec<_>>();
    result.page = Some(GraphInspectionPagination {
        offset,
        total: constants.len(),
        next_offset: InspectionPage::known(offset, items.len(), constants.len()).next_offset,
    });
    result.view = GraphInspectionView::Constants;
    result.content = GraphInspectionItems::ConstantSummaries(items);
    Ok(AutomationCapabilityResult::GraphInspectionPage(result))
}

pub(super) fn inspect(
    request: InspectConstantsRequest,
    path: &GraphResourcePath,
    document: &GraphDocument,
    projection: &EditorProjectionModel,
    version: ResourceVersion,
    hash: String,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    let mut result = inspection::queries::base(
        &request,
        &request.graph,
        path,
        document,
        projection,
        version,
        hash,
    )?;
    let mut seen = BTreeSet::new();
    let items = request
        .constant_ids
        .iter()
        .filter(|id| seen.insert(*id))
        .map(|id| {
            let constant = document
                .constants
                .get(&parse_id(id)?)
                .ok_or_else(|| invalid_edit_identity("constantIds"))?;
            Ok(ConstantInspection {
                summary: summary(constant),
                description: constant.description.clone(),
                tags: constant.tags.clone(),
                value_path: request.value_path.clone(),
                value: read_value(constant, &request)?,
            })
        })
        .collect::<Result<Vec<_>, CapabilityFailure>>()?;
    result.view = GraphInspectionView::Constants;
    result.content = GraphInspectionItems::ConstantDetails(items);
    Ok(AutomationCapabilityResult::GraphInspectionPage(result))
}

fn read_value(
    constant: &GraphConstant,
    request: &InspectConstantsRequest,
) -> Result<ConstantReadValue, CapabilityFailure> {
    if let Some(table) = &constant.tabular {
        if !request.value_path.is_empty() {
            return Err(invalid_edit_identity("valuePath"));
        }
        if request.columns.iter().any(|name| {
            !table
                .columns()
                .iter()
                .any(|column| column.name().as_str() == name)
        }) {
            return Err(invalid_edit_identity("columns"));
        }
        let columns = table
            .columns()
            .iter()
            .filter(|column| {
                request.columns.is_empty()
                    || request
                        .columns
                        .iter()
                        .any(|name| name == column.name().as_str())
            })
            .collect::<Vec<_>>();
        let column_offset = request.column_offset.min(columns.len());
        let selected = columns
            .iter()
            .skip(column_offset)
            .take(request.column_limit)
            .collect::<Vec<_>>();
        let offset = request.offset.min(table.row_count());
        let count = request.limit.min(1000).min(table.row_count() - offset);
        let rows = (offset..offset + count)
            .map(|row| {
                selected
                    .iter()
                    .map(|column| column.values()[row].display_value())
                    .collect::<Vec<TabularScalar>>()
            })
            .collect();
        return Ok(ConstantReadValue::Table {
            columns: selected
                .iter()
                .map(|column| column.name().as_str().to_string())
                .collect(),
            rows,
            page: InspectionPage::known(offset, count, table.row_count()),
            column_page: InspectionPage::known(column_offset, selected.len(), columns.len()),
        });
    }
    if !request.columns.is_empty() || request.column_offset != 0 {
        return Err(invalid_edit_identity("columns"));
    }
    let mut value = &constant.data_value;
    for step in &request.value_path {
        value = match (value, step) {
            (DataValue::Object(fields), ConstantValuePath::Field { key }) => {
                fields.get(key.as_str())
            }
            (DataValue::List(items), ConstantValuePath::Item { index }) => items.get(*index),
            _ => None,
        }
        .ok_or_else(|| invalid_edit_identity("valuePath"))?;
    }
    let (value, offset, returned, total) = match value {
        DataValue::String(value) => {
            let total = value.chars().count();
            let offset = request.offset.min(total);
            let text = value
                .chars()
                .skip(offset)
                .take(request.limit)
                .collect::<String>();
            let returned = text.chars().count();
            (DataValue::String(text.into()), offset, returned, total)
        }
        DataValue::Bytes(values) => {
            let offset = request.offset.min(values.len());
            let count = request.limit.min(values.len() - offset);
            (
                DataValue::Bytes(values[offset..offset + count].to_vec()),
                offset,
                count,
                values.len(),
            )
        }
        DataValue::List(values) => {
            let offset = request.offset.min(values.len());
            let count = request.limit.min(100).min(values.len() - offset);
            (
                DataValue::List(values[offset..offset + count].to_vec()),
                offset,
                count,
                values.len(),
            )
        }
        DataValue::Object(values) => {
            let offset = request.offset.min(values.len());
            let count = request.limit.min(100).min(values.len() - offset);
            (
                DataValue::Object(
                    values
                        .iter()
                        .skip(offset)
                        .take(count)
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect(),
                ),
                offset,
                count,
                values.len(),
            )
        }
        value => {
            if request.offset != 0 {
                return Err(invalid_edit_identity("offset"));
            }
            (value.clone(), 0, 1, 1)
        }
    };
    Ok(ConstantReadValue::Value {
        value,
        page: InspectionPage::known(offset, returned, total),
    })
}

pub(super) fn create(
    editor: &mut GraphDocumentEditor<'_>,
    declaration: ConstantDeclaration,
    constants: &mut BTreeMap<String, String>,
    nodes: &mut BTreeMap<String, String>,
) -> Result<(), CapabilityFailure> {
    if constants.contains_key(&declaration.client_id) {
        return Err(invalid_edit_identity("clientId"));
    }
    let id = ConstantId::new();
    let constant = GraphConstant {
        id,
        name: declaration.name,
        data_type: declaration.value.data_type,
        data_value: declaration.value.data_value,
        tabular: declaration.value.tabular,
        description: declaration.description,
        tags: declaration.tags,
    };
    let _ = editor
        .apply(EditorGraphMutation::SetConstant {
            id,
            constant: Some(constant),
        })
        .map_err(map_graph_error)?;
    if let Some(node) = declaration.reference_node {
        let alias = node
            .client_id
            .unwrap_or_else(|| declaration.client_id.clone());
        if alias.is_empty() || alias.len() > 64 || nodes.contains_key(&alias) {
            return Err(invalid_edit_identity("clientId"));
        }
        let before = editor
            .document()
            .nodes
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let mutation = editor_mutation(
            GraphEditOperation::InsertConstantReference {
                id: id.to_string(),
                x: node.position.x,
                y: node.position.y,
                client_id: None,
            },
            &[],
        )?;
        let _ = editor.apply(mutation).map_err(map_graph_error)?;
        let created = *editor
            .document()
            .nodes
            .keys()
            .find(|id| !before.contains(id))
            .ok_or_else(|| graph_failure(CapabilityFailureCode::MutationRejected))?;
        if let Some(label) = node.label {
            let _ = editor
                .apply(EditorGraphMutation::SetNodeLabel {
                    node_id: created,
                    label: Some(label),
                })
                .map_err(map_graph_error)?;
        }
        nodes.insert(alias, created.to_string());
    }
    constants.insert(declaration.client_id, id.to_string());
    Ok(())
}

pub(super) fn update(
    editor: &mut GraphDocumentEditor<'_>,
    update: ConstantUpdate,
) -> Result<(), CapabilityFailure> {
    let id = parse_id(&update.constant_id)?;
    let mut constant = editor
        .document()
        .constants
        .get(&id)
        .cloned()
        .ok_or_else(|| invalid_edit_identity("constantId"))?;
    if let Some(name) = update.name {
        constant.name = name;
    }
    if let Some(value) = update.value {
        constant.data_type = value.data_type;
        constant.data_value = value.data_value;
        constant.tabular = value.tabular;
    }
    if let Some(description) = update.description {
        constant.description = description;
    }
    if let Some(tags) = update.tags {
        constant.tags = tags;
    }
    let _ = editor
        .apply(EditorGraphMutation::SetConstant {
            id,
            constant: Some(constant),
        })
        .map_err(map_graph_error)?;
    Ok(())
}
