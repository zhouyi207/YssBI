use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use yss_i18n::{Backend, SimpleBackend, resolve_locale, translate};
use yss_node_protocol::{I18nKey, NodeTypeId};
use yss_node_registry::{I18nManifest, NodeRegistry};

const DEFAULT_LOCALE: &str = "en-US";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Message {
    Text(&'static str),
    Aliases(&'static [&'static str]),
}

type AliasBundle = BTreeMap<I18nKey, &'static [&'static str]>;

#[derive(Clone)]
pub struct BuiltinCatalog {
    messages: Arc<CatalogMessages>,
}

struct CatalogMessages {
    text: SimpleBackend,
    aliases: BTreeMap<&'static str, AliasBundle>,
}

impl std::fmt::Debug for BuiltinCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BuiltinCatalog")
            .field("locales", &self.messages.text.available_locales())
            .field("aliases", &self.messages.aliases)
            .finish()
    }
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
        let mut bundles = BTreeMap::<&'static str, BTreeMap<I18nKey, Message>>::new();
        for (locale, key, message) in entries {
            let key = I18nKey::new(key.as_str()).map_err(|source| {
                yss_node_protocol::ProtocolError::InvalidSemanticId {
                    value: key.as_str().into(),
                    source,
                }
            })?;
            bundles
                .entry(*locale)
                .or_default()
                .insert(key, message.clone());
        }
        let mut text = SimpleBackend::new();
        let mut aliases = BTreeMap::new();
        for (locale, bundle) in bundles {
            let mut translations = HashMap::new();
            let mut alias_bundle = AliasBundle::new();
            for (key, message) in bundle {
                match message {
                    Message::Text(value) => {
                        translations
                            .insert(Cow::Owned(key.as_str().to_owned()), Cow::Borrowed(value));
                    }
                    Message::Aliases(values) => {
                        alias_bundle.insert(key, values);
                    }
                }
            }
            text.add_translations(Cow::Borrowed(locale), translations);
            aliases.insert(locale, alias_bundle);
        }
        Ok(Self {
            messages: Arc::new(CatalogMessages { text, aliases }),
        })
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
        let requested_locale = locale.trim().replace('_', "-");
        let locale = resolve_locale(locale, DEFAULT_LOCALE);
        let categories = registry
            .categories()
            .iter()
            .map(|(id, category)| {
                let title = self.text(locale, &category.title_key);
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
                Some(self.static_item(node.protocol(), locale, descriptor))
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
                .then(|| self.resource_item(entry, node.protocol(), locale))
        }));
        LocalizedCatalog {
            locale: requested_locale.into(),
            categories,
            items,
        }
    }

    pub(crate) fn validate(
        &self,
        required: &I18nManifest,
        alias_keys: &BTreeSet<I18nKey>,
    ) -> Result<(), I18nBundleValidationError> {
        let default_aliases = self.messages.aliases.get(DEFAULT_LOCALE);
        let keys: Vec<_> = required
            .keys
            .iter()
            .filter(|key| {
                self.messages
                    .text
                    .translate(DEFAULT_LOCALE, key.as_str())
                    .is_none()
                    && default_aliases.is_none_or(|bundle| !bundle.contains_key(*key))
            })
            .map(|key| key.as_str().into())
            .collect();
        if !keys.is_empty() {
            return Err(I18nBundleValidationError::MissingDefaultLocale { keys });
        }
        for locale in self.messages.aliases.keys() {
            for key in alias_keys {
                if self.messages.text.translate(locale, key.as_str()).is_some() {
                    return Err(I18nBundleValidationError::AliasesNotArray {
                        locale: (*locale).into(),
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
            .and_then(|key| self.messages.aliases.get(DEFAULT_LOCALE)?.get(key))
        {
            Some(values) => values
                .iter()
                .map(|value| Box::<str>::from(*value))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Looks up node metadata text, returning the key when no translation exists.
    pub fn text(&self, locale: &str, key: &I18nKey) -> Box<str> {
        translate(&self.messages.text, locale, key.as_str(), DEFAULT_LOCALE)
            .into_owned()
            .into_boxed_str()
    }

    fn aliases(&self, locale: &str, key: &I18nKey) -> Vec<Box<str>> {
        let locale = resolve_locale(locale, DEFAULT_LOCALE);
        let values = self
            .messages
            .aliases
            .get(locale)
            .and_then(|bundle| bundle.get(key))
            .or_else(|| self.messages.aliases.get(DEFAULT_LOCALE)?.get(key));
        match values {
            Some(values) => values.iter().map(|value| (*value).into()).collect(),
            None => vec![key.as_str().into()],
        }
    }
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
            assert_eq!(actual.locale.as_ref(), requested.trim().replace('_', "-"));
            assert_eq!(actual.items, expected.items, "{requested}");
            assert_eq!(actual.categories, expected.categories, "{requested}");
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
            (" en_GB ", "Example"),
            ("zh-CN", "示例"),
            ("ZH_cn", "示例"),
            ("ZH-tw", "示例"),
            (" zh_Hant_TW ", "示例"),
            ("fr-FR", "Example"),
            ("", "Example"),
        ] {
            assert_eq!(catalog.text(locale, &key).as_ref(), expected, "{locale}");
        }
    }

    #[test]
    fn text_falls_back_to_default_then_preserves_missing_keys() {
        let key = I18nKey::new("nodes.example.documentation").unwrap();
        let missing = I18nKey::new("nodes.example.missing").unwrap();
        let catalog = BuiltinCatalog::new(&[(
            "en-US",
            key.as_str().to_owned(),
            Text("Example documentation"),
        )])
        .unwrap();
        assert_eq!(
            catalog.text("zh-CN", &key).as_ref(),
            "Example documentation"
        );
        assert_eq!(catalog.text("zh-CN", &missing).as_ref(), missing.as_str());
    }

    #[test]
    fn cloned_catalogs_keep_concurrent_request_locales_isolated() {
        let system = crate::build_builtin_node_system().unwrap();
        let english = system.catalog.localize(&system.registry, "en-US");
        let chinese = system.catalog.localize(&system.registry, "zh-CN");
        assert_ne!(english.items, chinese.items);
        std::thread::scope(|scope| {
            let registry = &system.registry;
            for (locales, expected) in [
                (["en-US", " EN_gb ", "unknown"], english),
                (["zh-CN", " ZH_tw ", "zh_Hant"], chinese),
            ] {
                let catalog = system.catalog.clone();
                scope.spawn(move || {
                    for locale in locales.into_iter().cycle().take(12) {
                        let actual = catalog.localize(registry, locale);
                        assert_eq!(actual.locale.as_ref(), locale.trim().replace('_', "-"));
                        assert_eq!(actual.items, expected.items, "{locale}");
                        assert_eq!(actual.categories, expected.categories, "{locale}");
                    }
                });
            }
        });
    }

    #[test]
    fn catalog_validation_preserves_required_defaults_and_typed_aliases() {
        let title = I18nKey::new("nodes.example.title").unwrap();
        let aliases = I18nKey::new("nodes.example.aliases").unwrap();
        let required = I18nManifest {
            keys: BTreeSet::from([title.clone(), aliases.clone()]),
        };
        let alias_keys = BTreeSet::from([aliases.clone()]);
        let mut entries = vec![
            ("zh-CN", title.as_str().to_owned(), Text("示例")),
            ("en-US", aliases.as_str().to_owned(), Aliases(&["example"])),
            ("zh-CN", aliases.as_str().to_owned(), Text("not an array")),
        ];
        let catalog = BuiltinCatalog::new(&entries).unwrap();
        assert_eq!(
            catalog.validate(&required, &alias_keys),
            Err(I18nBundleValidationError::MissingDefaultLocale {
                keys: vec![title.as_str().into()],
            })
        );
        entries.push(("en-US", title.as_str().to_owned(), Text("Example")));
        let catalog = BuiltinCatalog::new(&entries).unwrap();
        assert_eq!(
            catalog.validate(&required, &alias_keys),
            Err(I18nBundleValidationError::AliasesNotArray {
                locale: "zh-CN".into(),
                key: aliases.as_str().into(),
            })
        );
    }
}
