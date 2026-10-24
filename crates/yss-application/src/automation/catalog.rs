use super::*;
use crate::graph::catalog::{LocalizedCatalogRequest, localized_node_catalog_in_session};
use yss_assistant_contract::{
    BrowseNodesRequest, InspectNodeTypeRequest, InspectionPage, NodeCatalogMatch, NodeCatalogPage,
    NodeTypeDefinition, NodeTypeInspection,
};
use yss_node_catalog::LocalizedCatalogItem;
use yss_node_protocol::NodeProtocol;

#[cfg(test)]
mod tests;

pub(super) fn port_definitions(
    protocol: &NodeProtocol,
) -> Vec<yss_assistant_contract::NodeCreationPortDefinition> {
    use yss_assistant_contract::{NodeCreationPortDefinition, NodePortCountPolicy};
    yss_node_catalog::node_creation_ports(protocol)
        .into_vec()
        .into_iter()
        .map(|port| NodeCreationPortDefinition {
            key: port.key.to_string(),
            title: port.title.into_string(),
            direction: match port.direction {
                yss_node_protocol::PortDirection::Input => "input",
                yss_node_protocol::PortDirection::Output => "output",
            }
            .into(),
            count: match port.count {
                yss_node_catalog::PortCountPolicy::Fixed => NodePortCountPolicy::Fixed,
                yss_node_catalog::PortCountPolicy::Derived => NodePortCountPolicy::Derived,
                yss_node_catalog::PortCountPolicy::Configurable {
                    min,
                    max,
                    member_templates,
                } => NodePortCountPolicy::Configurable {
                    min,
                    max,
                    member_templates: member_templates.iter().map(ToString::to_string).collect(),
                },
            },
        })
        .collect()
}

pub(super) fn configuration_schema(
    protocol: &NodeProtocol,
    item: &LocalizedCatalogItem,
) -> serde_json::Value {
    let mut schema = serde_json::Value::from(protocol.configuration_schema());
    let fields = schema["properties"]["parameters"]["properties"]
        .as_object_mut()
        .expect("protocol configuration has parameter properties");
    for label in &item.parameters {
        if let Some(field) = fields
            .get_mut(label.key.as_ref())
            .and_then(serde_json::Value::as_object_mut)
        {
            field.insert("title".into(), serde_json::json!(label.title));
            if let Some(description) = &label.description {
                field.insert("description".into(), serde_json::json!(description));
            }
        }
    }
    schema
}

pub(super) fn browse_nodes(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: BrowseNodesRequest,
) -> Result<NodeCatalogPage, CapabilityFailure> {
    let result = localized_node_catalog_in_session(
        application,
        captured,
        LocalizedCatalogRequest::new(captured.project_instance_id().clone(), request.locale),
    )
    .map_err(map_catalog_error)?;
    let (_, _, _, catalog) = result.into_fields();
    let normalized_query = request.query.to_lowercase();
    let query_terms = normalized_query
        .split(|c: char| c.is_whitespace() || matches!(c, '/' | ',' | '|'))
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    let mut matches = catalog
        .items
        .iter()
        .filter(|item| item.available)
        .filter(|item| {
            request
                .category
                .as_ref()
                .is_none_or(|category| category == item.category_id.as_ref())
        })
        .filter_map(|item| {
            let score = catalog_item_score(item, normalized_query.trim(), &query_terms);
            (score > 0).then_some((score, item))
        })
        .collect::<Vec<_>>();
    // A word shared by many nodes would otherwise list the whole catalog; keep the
    // matches that are close to the best one.
    let best = matches.iter().map(|(score, _)| *score).max().unwrap_or(0);
    matches.retain(|(score, _)| *score * 2 >= best);
    matches.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.node_type_id.cmp(&right.node_type_id))
            .then_with(|| left.resource_path.cmp(&right.resource_path))
    });
    let total = matches.len();
    let offset = request.offset.min(total);
    let matches = matches
        .into_iter()
        .skip(offset)
        .take(usize::from(
            request
                .limit
                .min(yss_assistant_contract::MAX_CATALOG_SEARCH_RESULTS),
        ))
        .map(|(_, item)| NodeCatalogMatch {
            type_id: item.node_type_id.to_string(),
            title: item.title.to_string(),
            category_id: item.category_id.to_string(),
            summary: item.documentation.as_deref().and_then(catalog_summary),
            resource_path: item
                .resource_path
                .as_ref()
                .map(|path| path.as_str().to_owned()),
        })
        .collect::<Vec<_>>();
    Ok(NodeCatalogPage {
        locale: catalog.locale.into_string(),
        page: InspectionPage::known(offset, matches.len(), total),
        matches,
    })
}

pub(super) fn inspect_node_type(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: InspectNodeTypeRequest,
) -> Result<NodeTypeInspection, CapabilityFailure> {
    let result = localized_node_catalog_in_session(
        application,
        captured,
        LocalizedCatalogRequest::new(captured.project_instance_id().clone(), request.locale),
    )
    .map_err(map_catalog_error)?;
    let (_, _, _, catalog) = result.into_fields();
    let mut types = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for type_id in request.type_ids {
        if !seen.insert(type_id.clone()) {
            continue;
        }
        let item = catalog
            .items
            .iter()
            .find(|item| item.available && item.node_type_id.as_ref() == type_id)
            .ok_or_else(|| {
                CapabilityFailure::new(yss_assistant_contract::failure_code::CATALOG_UNAVAILABLE)
                    .with_detail("typeId", &type_id)
            })?;
        let protocol = captured
            .graph()
            .registry()
            .protocol(&type_id.parse().map_err(|_| {
                CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                    .with_detail("field", "typeIds")
            })?)
            .ok_or_else(|| {
                CapabilityFailure::new(yss_assistant_contract::failure_code::CATALOG_UNAVAILABLE)
                    .with_detail("typeId", &type_id)
            })?;
        types.push(NodeTypeDefinition {
            type_id,
            title: item.title.to_string(),
            documentation: item.documentation.as_ref().map(ToString::to_string),
            configuration_schema: configuration_schema(protocol, item),
            ports: port_definitions(protocol),
        });
    }
    Ok(NodeTypeInspection {
        locale: catalog.locale.into_string(),
        types,
    })
}

/// Names weigh more than descriptive search text. Categories are filtered with the
/// `category` argument, not matched as words: most statistics nodes share one prefix.
fn catalog_item_score(item: &LocalizedCatalogItem, query: &str, terms: &[&str]) -> usize {
    if terms.is_empty() {
        return 1;
    }
    let lower =
        |values: &mut dyn Iterator<Item = &str>| values.map(str::to_lowercase).collect::<Vec<_>>();
    let names = lower(
        &mut [item.node_type_id.as_ref(), item.title.as_ref()]
            .into_iter()
            .chain(item.aliases.iter().map(AsRef::as_ref))
            .chain(item.resource_names.iter().map(AsRef::as_ref)),
    );
    let terms_text = lower(&mut item.technical_terms.iter().map(AsRef::as_ref));
    let text = lower(&mut item.backend_search_text.iter().map(AsRef::as_ref));
    let found = |fields: &[String], term: &str| {
        fields.iter().any(|value| contains_search_term(value, term))
    };
    let exact = if found(&names, query) {
        100
    } else if found(&terms_text, query) {
        60
    } else {
        0
    };
    exact
        + terms
            .iter()
            .map(|term| {
                if found(&names, term) {
                    4
                } else if found(&terms_text, term) {
                    3
                } else if found(&text, term) {
                    1
                } else {
                    0
                }
            })
            .sum::<usize>()
}

/// The first sentence of a node's documentation, without its Markdown heading.
fn catalog_summary(documentation: &str) -> Option<String> {
    const MAX_CHARACTERS: usize = 120;
    let paragraph = documentation
        .split("\n\n")
        .map(str::trim)
        .find(|paragraph| !paragraph.is_empty() && !paragraph.starts_with('#'))?;
    let flat = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    let sentence_end = flat
        .char_indices()
        .find(|(_, character)| matches!(character, '。' | '；' | '.' | ';'))
        .map_or(flat.len(), |(index, character)| {
            index + character.len_utf8()
        });
    let sentence = &flat[..sentence_end];
    let mut summary: String = sentence.chars().take(MAX_CHARACTERS).collect();
    if sentence.chars().count() > MAX_CHARACTERS {
        summary.push('…');
    }
    Some(summary)
}

fn contains_search_term(value: &str, term: &str) -> bool {
    // Preserve type IDs and names such as to_numeric. ASCII words must not match
    // the middle of another word (bin in Durbin); Chinese terms remain substrings.
    value.match_indices(term).any(|(start, _)| {
        let end = start + term.len();
        let starts_word = term
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric);
        let ends_word = term
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric);
        (!starts_word || start == 0 || !value.as_bytes()[start - 1].is_ascii_alphanumeric())
            && (!ends_word || end == value.len() || !value.as_bytes()[end].is_ascii_alphanumeric())
    })
}
