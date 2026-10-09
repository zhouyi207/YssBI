use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use yss_node_protocol::{I18nKey, NodeTypeId};
use yss_node_registry::{I18nManifest, NodeRegistry};

const DEFAULT_LOCALE: &str = "en-US";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Message {
    Text(&'static str),
    Aliases(&'static [&'static str]),
}

type Bundle = BTreeMap<I18nKey, Message>;

#[derive(Debug, Clone)]
pub struct BuiltinCatalog {
    bundles: BTreeMap<Box<str>, Bundle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedCatalog {
    pub locale: Box<str>,
    pub categories: Vec<LocalizedCategory>,
    pub items: Vec<LocalizedCatalogItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCategory {
    pub category_id: Box<str>,
    pub parent_category_id: Option<Box<str>>,
    pub order: i32,
    pub title: Box<str>,
    pub search_text: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCatalogItem {
    pub available: bool,
    pub node_type_id: Box<str>,
    pub title: Box<str>,
    pub documentation: Option<Box<str>>,
    pub category_id: Box<str>,
    pub icon_id: Box<str>,
    pub style_id: Box<str>,
    pub aliases: Vec<Box<str>>,
    pub technical_terms: Vec<Box<str>>,
    pub backend_search_text: Vec<Box<str>>,
    pub resource_names: Vec<Box<str>>,
    pub ports: Vec<LocalizedPort>,
    pub parameters: Vec<LocalizedParameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_path: Option<CatalogResourcePath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_revision: Option<u64>,
    pub creation: NodeCreation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedPort {
    pub key: Box<str>,
    pub label: Box<str>,
    pub direction: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedParameter {
    pub key: Box<str>,
    pub title: Box<str>,
    pub description: Option<Box<str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CatalogResourcePath(Box<str>);

impl CatalogResourcePath {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum NodeCreation {
    #[serde(rename = "static")]
    Static {
        #[serde(rename = "nodeTypeId")]
        node_type_id: NodeTypeId,
    },
    #[serde(rename = "parameterizedStatic")]
    ParameterizedStatic {
        #[serde(rename = "nodeTypeId")]
        node_type_id: NodeTypeId,
        #[serde(rename = "requiredParameters")]
        required_parameters: Box<[yss_node_protocol::ParameterKey]>,
    },
    #[serde(rename = "resourceBound")]
    ResourceBound {
        #[serde(rename = "nodeTypeId")]
        node_type_id: NodeTypeId,
        #[serde(rename = "resourcePath")]
        resource_path: CatalogResourcePath,
        #[serde(rename = "resourceRevision")]
        resource_revision: u64,
        #[serde(rename = "createArgs")]
        create_args: ResourceBoundCreateArgs,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResourceBoundCreateArgs {
    #[serde(rename = "function_graph")]
    FunctionGraph,
    Database,
}

impl<'de> Deserialize<'de> for ResourceBoundCreateArgs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            kind: Box<str>,
        }

        match Wire::deserialize(deserializer)?.kind.as_ref() {
            "function_graph" => Ok(Self::FunctionGraph),
            "database" => Ok(Self::Database),
            kind => Err(serde::de::Error::unknown_variant(
                kind,
                &["function_graph", "database"],
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogResourceEntry {
    pub name: Box<str>,
    pub node_type_id: NodeTypeId,
    pub resource_path: CatalogResourcePath,
    pub resource_revision: u64,
    pub create_args: ResourceBoundCreateArgs,
    pub technical_terms: Vec<Box<str>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum I18nBundleValidationError {
    MissingDefaultLocale { keys: Vec<Box<str>> },
    AliasesNotArray { locale: Box<str>, key: Box<str> },
}

impl std::fmt::Display for I18nBundleValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDefaultLocale { keys } => write!(
                formatter,
                "default locale is missing keys: {}",
                keys.iter()
                    .map(AsRef::as_ref)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::AliasesNotArray { locale, key } => write!(
                formatter,
                "locale '{locale}' aliases key '{key}' is not an array"
            ),
        }
    }
}

impl std::error::Error for I18nBundleValidationError {}

pub fn authoritative_static_descriptor(
    protocol: &yss_node_protocol::NodeProtocol,
) -> Option<NodeCreation> {
    if protocol.catalog.hidden
        || protocol.managed_role.is_some()
        || matches!(
            protocol.instance_display,
            yss_node_protocol::NodeInstanceDisplaySpec::ResourceParameter { .. }
        )
    {
        return None;
    }
    let required_parameters = protocol
        .parameters
        .iter()
        .filter(|parameter| {
            parameter.default_value.is_none()
                && !matches!(
                    parameter.editor,
                    yss_node_protocol::ParameterEditorSpec::GraphConstant
                )
                && parameter
                    .constraints
                    .contains(&yss_node_protocol::ParameterConstraint::Required)
        })
        .collect::<Vec<_>>();
    if required_parameters.is_empty() {
        return Some(NodeCreation::Static {
            node_type_id: protocol.type_id.clone(),
        });
    }
    Some(NodeCreation::ParameterizedStatic {
        node_type_id: protocol.type_id.clone(),
        required_parameters: required_parameters
            .into_iter()
            .map(|parameter| parameter.key.clone())
            .collect(),
    })
}

impl BuiltinCatalog {
    pub(crate) fn new(
        entries: &[(&'static str, String, Message)],
    ) -> Result<Self, yss_node_protocol::ProtocolError> {
        let mut bundles = BTreeMap::<Box<str>, Bundle>::new();
        for (locale, key, message) in entries {
            let key = I18nKey::new(key.as_str()).map_err(|source| {
                yss_node_protocol::ProtocolError::InvalidSemanticId {
                    value: key.as_str().into(),
                    source,
                }
            })?;
            bundles
                .entry((*locale).into())
                .or_default()
                .insert(key, message.clone());
        }
        Ok(Self { bundles })
    }

    pub fn localize(&self, registry: &NodeRegistry, locale: &str) -> LocalizedCatalog {
        self.localize_with_resources(registry, locale, &[])
    }

    pub fn localize_with_resources(
        &self,
        registry: &NodeRegistry,
        locale: &str,
        resources: &[CatalogResourceEntry],
    ) -> LocalizedCatalog {
        let locale = normalize_locale(locale);
        let categories = registry
            .categories()
            .iter()
            .map(|(id, category)| {
                let title = self.text(&locale, &category.title_key);
                LocalizedCategory {
                    category_id: id.as_str().into(),
                    parent_category_id: category
                        .parent
                        .as_ref()
                        .map(|parent| parent.as_str().into()),
                    order: category.order,
                    search_text: search([title.as_ref()]),
                    title,
                }
            })
            .collect();
        let mut items = registry
            .iter()
            .filter_map(|(_, node)| {
                let descriptor = authoritative_static_descriptor(node.protocol())?;
                Some(self.static_item(node.protocol(), &locale, descriptor))
            })
            .collect::<Vec<_>>();
        let mut resources = resources.iter().collect::<Vec<_>>();
        resources.sort_by(|left, right| {
            left.resource_path
                .cmp(&right.resource_path)
                .then_with(|| left.node_type_id.as_str().cmp(right.node_type_id.as_str()))
        });
        items.extend(resources.into_iter().filter_map(|entry| {
            let node = registry.get(&entry.node_type_id)?;
            (!node.protocol().catalog.hidden)
                .then(|| self.resource_item(entry, node.protocol(), &locale))
        }));
        LocalizedCatalog {
            locale: locale.into(),
            categories,
            items,
        }
    }

    pub(crate) fn validate(
        &self,
        required: &I18nManifest,
        alias_keys: &BTreeSet<I18nKey>,
    ) -> Result<(), I18nBundleValidationError> {
        let default_bundle = self.bundles.get(DEFAULT_LOCALE);
        let keys: Vec<_> = required
            .keys
            .iter()
            .filter(|key| default_bundle.is_none_or(|bundle| !bundle.contains_key(*key)))
            .map(|key| key.as_str().into())
            .collect();
        if !keys.is_empty() {
            return Err(I18nBundleValidationError::MissingDefaultLocale { keys });
        }
        for (locale, bundle) in &self.bundles {
            for key in alias_keys {
                if matches!(bundle.get(key), Some(Message::Text(_))) {
                    return Err(I18nBundleValidationError::AliasesNotArray {
                        locale: locale.clone(),
                        key: key.as_str().into(),
                    });
                }
            }
        }
        Ok(())
    }

    fn static_item(
        &self,
        protocol: &yss_node_protocol::NodeProtocol,
        locale: &str,
        creation: NodeCreation,
    ) -> LocalizedCatalogItem {
        let title = self.text(locale, &protocol.catalog.title_key);
        let documentation = super::node_documentation(&protocol.type_id, locale);
        let aliases = match protocol.catalog.aliases_key.as_ref() {
            Some(key) => self.aliases(locale, key),
            None => Vec::new(),
        };
        let technical_terms = self.technical_terms(protocol);
        let backend_search_text = [title.clone()]
            .into_iter()
            .chain(aliases.iter().cloned())
            .collect();
        LocalizedCatalogItem {
            node_type_id: protocol.type_id.as_str().into(),
            available: true,
            title,
            documentation,
            category_id: protocol.catalog.category_id.as_str().into(),
            icon_id: protocol.catalog.icon_id.as_str().into(),
            style_id: protocol.catalog.style_id.as_str().into(),
            aliases,
            technical_terms,
            backend_search_text,
            resource_names: Vec::new(),
            ports: Self::project_ports(protocol),
            parameters: self.localized_parameters(protocol, locale),
            resource_path: None,
            resource_revision: None,
            creation,
        }
    }

    fn resource_item(
        &self,
        entry: &CatalogResourceEntry,
        protocol: &yss_node_protocol::NodeProtocol,
        locale: &str,
    ) -> LocalizedCatalogItem {
        let title = entry.name.clone();
        let documentation = super::node_documentation(&protocol.type_id, locale);
        let aliases = match protocol.catalog.aliases_key.as_ref() {
            Some(key) => self.aliases(locale, key),
            None => Vec::new(),
        };
        let mut technical_terms = self.technical_terms(protocol);
        technical_terms.extend(entry.technical_terms.iter().cloned());
        technical_terms.sort();
        technical_terms.dedup();
        let backend_search_text = aliases.clone();
        let resource_names = vec![entry.name.clone()];
        LocalizedCatalogItem {
            node_type_id: entry.node_type_id.as_str().into(),
            available: true,
            title,
            documentation,
            category_id: protocol.catalog.category_id.as_str().into(),
            icon_id: protocol.catalog.icon_id.as_str().into(),
            style_id: protocol.catalog.style_id.as_str().into(),
            aliases,
            technical_terms,
            backend_search_text,
            resource_names,
            ports: Self::project_ports(protocol),
            parameters: self.localized_parameters(protocol, locale),
            resource_path: Some(entry.resource_path.clone()),
            resource_revision: Some(entry.resource_revision),
            creation: NodeCreation::ResourceBound {
                node_type_id: entry.node_type_id.clone(),
                resource_path: entry.resource_path.clone(),
                resource_revision: entry.resource_revision,
                create_args: entry.create_args,
            },
        }
    }

    fn project_ports(protocol: &yss_node_protocol::NodeProtocol) -> Vec<LocalizedPort> {
        protocol
            .interface
            .ports
            .iter()
            .map(|port| LocalizedPort {
                key: port.key.as_str().into(),
                label: port.title.clone(),
                direction: match port.direction {
                    yss_node_protocol::PortDirection::Input => "input".into(),
                    yss_node_protocol::PortDirection::Output => "output".into(),
                },
            })
            .collect()
    }

    fn localized_parameters(
        &self,
        protocol: &yss_node_protocol::NodeProtocol,
        locale: &str,
    ) -> Vec<LocalizedParameter> {
        protocol
            .parameters
            .iter()
            .map(|parameter| LocalizedParameter {
                key: parameter.key.as_str().into(),
                title: self.text(locale, &parameter.title_key),
                description: parameter
                    .description_key
                    .as_ref()
                    .map(|key| self.text(locale, key)),
            })
            .collect()
    }

    fn technical_terms(&self, protocol: &yss_node_protocol::NodeProtocol) -> Vec<Box<str>> {
        match protocol
            .catalog
            .aliases_key
            .as_ref()
            .and_then(|key| self.bundles.get(DEFAULT_LOCALE)?.get(key))
        {
            Some(Message::Aliases(values)) => values
                .iter()
                .map(|value| Box::<str>::from(*value))
                .collect(),
            Some(Message::Text(_)) | None => Vec::new(),
        }
    }

    fn message(&self, locale: &str, key: &I18nKey) -> Option<&Message> {
        if let Some(message) = self.bundles.get(locale).and_then(|bundle| bundle.get(key)) {
            return Some(message);
        }
        locale_chain(locale).into_iter().find_map(|candidate| {
            self.bundles
                .get(candidate.as_str())
                .or_else(|| {
                    self.bundles
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(candidate.as_str()))
                        .map(|(_, bundle)| bundle)
                })
                .or_else(|| {
                    (!candidate.contains('-'))
                        .then(|| {
                            self.bundles
                                .iter()
                                .find(|(name, _)| {
                                    name.split('-').next().is_some_and(|language| {
                                        language.eq_ignore_ascii_case(candidate.as_str())
                                    })
                                })
                                .map(|(_, bundle)| bundle)
                        })
                        .flatten()
                })?
                .get(key)
        })
    }

    /// Looks up node metadata text, returning the key when no translation exists.
    pub fn text(&self, locale: &str, key: &I18nKey) -> Box<str> {
        match self.message(locale, key) {
            Some(Message::Text(value)) => (*value).into(),
            _ => key.as_str().into(),
        }
    }

    fn aliases(&self, locale: &str, key: &I18nKey) -> Vec<Box<str>> {
        match self.message(locale, key) {
            Some(Message::Aliases(values)) => values.iter().map(|value| (*value).into()).collect(),
            _ => vec![key.as_str().into()],
        }
    }
}

fn normalize_locale(locale: &str) -> String {
    locale.trim().replace('_', "-")
}

fn locale_chain(locale: &str) -> Vec<String> {
    let normalized = normalize_locale(locale);
    let language = match normalized.split('-').next() {
        Some(language) => language.to_owned(),
        None => normalized.clone(),
    };
    let mut chain = vec![normalized];
    if !language.is_empty() && language != chain[0] {
        chain.push(language);
    }
    if !chain
        .iter()
        .any(|item| item.eq_ignore_ascii_case(DEFAULT_LOCALE))
    {
        chain.push(DEFAULT_LOCALE.into());
    }
    chain
}

fn search<'a>(parts: impl IntoIterator<Item = &'a str>) -> Box<str> {
    crate::normalize_catalog_search_text(&parts.into_iter().collect::<Vec<_>>().join(" "))
}

pub(crate) use Message::{Aliases, Text};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_help_uses_the_same_locale_as_its_metadata() {
        let system = crate::build_builtin_node_system().unwrap();
        for (baseline, requested) in [("zh-CN", "ZH_CN"), ("en-US", "zhx")] {
            let expected = system.catalog.localize(&system.registry, baseline);
            let actual = system.catalog.localize(&system.registry, requested);
            assert_eq!(actual.items.len(), expected.items.len());
            for (actual, expected) in actual.items.iter().zip(&expected.items) {
                assert_eq!(actual.node_type_id, expected.node_type_id);
                assert_eq!(actual.title, expected.title, "{}", actual.node_type_id);
                assert_eq!(
                    actual.documentation, expected.documentation,
                    "{}: {requested}",
                    actual.node_type_id
                );
            }
        }
    }

    #[test]
    fn locale_tags_keep_text_across_case_and_separator_variants() {
        let key = I18nKey::new("nodes.example.title").unwrap();
        let catalog = BuiltinCatalog::new(&[
            ("en-US", key.as_str().to_owned(), Text("Example")),
            ("zh-CN", key.as_str().to_owned(), Text("示例")),
        ])
        .unwrap();
        for (locale, expected) in [
            ("en-US", "Example"),
            ("EN-us", "Example"),
            ("EN_US", "Example"),
            ("zh-CN", "示例"),
            ("ZH_cn", "示例"),
            ("ZH-tw", "示例"),
            ("fr-FR", "Example"),
        ] {
            assert_eq!(catalog.text(locale, &key).as_ref(), expected, "{locale}");
        }
    }
}
