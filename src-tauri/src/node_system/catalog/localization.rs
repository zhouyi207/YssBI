use crate::node_system::analysis::{DiagnosticArguments, LocalizationLookup};
use crate::node_system::protocol::{I18nKey, NodeTypeId};
use crate::node_system::registry::{I18nManifest, NodeRegistry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Debug, Clone)]
pub struct BuiltinLocalizationBundle<'a> {
    catalog: &'a BuiltinCatalog,
    locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedCatalog {
    pub locale: Box<str>,
    pub categories: Vec<LocalizedCategoryDto>,
    pub items: Vec<LocalizedCatalogItemDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCatalogDto {
    pub project_instance_id: Box<str>,
    pub registry_fingerprint: Box<str>,
    pub resource_publication_revision: u64,
    pub locale: Box<str>,
    pub categories: Vec<LocalizedCategoryDto>,
    pub items: Vec<LocalizedCatalogItemDto>,
}

impl LocalizedCatalog {
    pub(crate) fn into_dto(
        self,
        project_instance_id: &str,
        registry_fingerprint: String,
        resource_publication_revision: u64,
    ) -> LocalizedCatalogDto {
        LocalizedCatalogDto {
            project_instance_id: project_instance_id.into(),
            registry_fingerprint: registry_fingerprint.into(),
            resource_publication_revision,
            locale: self.locale,
            categories: self.categories,
            items: self.items,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCategoryDto {
    pub category_id: Box<str>,
    pub parent_category_id: Option<Box<str>>,
    pub order: i32,
    pub title: Box<str>,
    pub search_text: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCatalogItemDto {
    pub node_type_id: Box<str>,
    pub title: Box<str>,
    pub description: Option<Box<str>>,
    pub documentation: Option<Box<str>>,
    pub category_id: Box<str>,
    pub icon_id: Box<str>,
    pub style_id: Box<str>,
    pub aliases: Vec<Box<str>>,
    pub technical_terms: Vec<Box<str>>,
    pub backend_search_text: Vec<Box<str>>,
    pub resource_names: Vec<Box<str>>,
    pub ports: Vec<LocalizedPortDto>,
    pub parameters: Vec<LocalizedParameterDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_path: Option<CatalogResourcePath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_revision: Option<crate::node_system::document::ResourceRevision>,
    pub creation: NodeCreationDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedPortDto {
    pub key: Box<str>,
    pub label: Box<str>,
    pub direction: Box<str>,
    pub kind: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedParameterDto {
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
pub enum NodeCreationDescriptor {
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
        required_parameters: Box<[crate::node_system::protocol::ParameterKey]>,
    },
    #[serde(rename = "resourceBound")]
    ResourceBound {
        #[serde(rename = "nodeTypeId")]
        node_type_id: NodeTypeId,
        #[serde(rename = "resourcePath")]
        resource_path: CatalogResourcePath,
        #[serde(rename = "resourceRevision")]
        resource_revision: crate::node_system::document::ResourceRevision,
        #[serde(rename = "createArgs")]
        create_args: ResourceBoundCreateArgsDto,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResourceBoundCreateArgsDto {
    Function,
    Variable,
    Database,
}

impl<'de> Deserialize<'de> for ResourceBoundCreateArgsDto {
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
            "function" => Ok(Self::Function),
            "variable" => Ok(Self::Variable),
            "database" => Ok(Self::Database),
            kind => Err(serde::de::Error::unknown_variant(
                kind,
                &["function", "variable", "database"],
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogResourceEntry {
    pub name: Box<str>,
    pub node_type_id: NodeTypeId,
    pub resource_path: CatalogResourcePath,
    pub resource_revision: crate::node_system::document::ResourceRevision,
    pub create_args: ResourceBoundCreateArgsDto,
    pub technical_terms: Vec<Box<str>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct I18nBundleInventory {
    pub default_locale_missing: Vec<Box<str>>,
    pub missing_by_locale: BTreeMap<Box<str>, Vec<Box<str>>>,
    pub unused_by_locale: BTreeMap<Box<str>, Vec<Box<str>>>,
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

pub(crate) fn authoritative_static_descriptor(
    registry: &NodeRegistry,
    protocol: &crate::node_system::protocol::NodeProtocol,
) -> Option<NodeCreationDescriptor> {
    if protocol.catalog.hidden || protocol.managed_role.is_some() {
        return None;
    }
    let required_parameters = protocol
        .parameters
        .parameters
        .iter()
        .filter(|parameter| {
            parameter.default_value.is_none()
                && parameter
                    .constraints
                    .contains(&crate::node_system::protocol::ParameterConstraint::Required)
        })
        .collect::<Vec<_>>();
    if required_parameters.is_empty() {
        return Some(NodeCreationDescriptor::Static {
            node_type_id: protocol.type_id.clone(),
        });
    }
    if !required_parameters.iter().all(|parameter| {
        matches!(
            &parameter.value_type,
            crate::node_system::protocol::TypeExpr::Concrete(type_id)
                if registry.has_nominal_parameter_validator(type_id)
        )
    }) {
        return None;
    }
    Some(NodeCreationDescriptor::ParameterizedStatic {
        node_type_id: protocol.type_id.clone(),
        required_parameters: required_parameters
            .into_iter()
            .map(|parameter| parameter.key.clone())
            .collect(),
    })
}

impl BuiltinCatalog {
    pub(crate) fn new(
        entries: &[(&'static str, &'static str, Message)],
    ) -> Result<Self, crate::node_system::protocol::ProtocolError> {
        let mut bundles = BTreeMap::<Box<str>, Bundle>::new();
        for (locale, key, message) in entries {
            let key = I18nKey::new(*key).map_err(|source| {
                crate::node_system::protocol::ProtocolError::InvalidSemanticId {
                    value: (*key).into(),
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

    #[cfg(test)]
    pub(crate) fn remove_message_for_test(&mut self, locale: &str, key: &I18nKey) {
        if let Some(bundle) = self.bundles.get_mut(locale) {
            bundle.remove(key);
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_message_for_test(
        &mut self,
        locale: &str,
        key: I18nKey,
        message: Message,
    ) {
        self.bundles
            .entry(locale.into())
            .or_default()
            .insert(key, message);
    }

    #[cfg(test)]
    pub(crate) fn replace_text_for_test(&mut self, locale: &str, key: I18nKey, text: &'static str) {
        self.replace_message_for_test(locale, key, Message::Text(text));
    }

    pub fn localization(&self, locale: &str) -> BuiltinLocalizationBundle<'_> {
        BuiltinLocalizationBundle {
            catalog: self,
            locale: normalize_locale(locale),
        }
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
                LocalizedCategoryDto {
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
                let descriptor = authoritative_static_descriptor(registry, node.protocol())?;
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

    pub fn audit(
        &self,
        required: &I18nManifest,
        _alias_keys: &BTreeSet<I18nKey>,
    ) -> I18nBundleInventory {
        let required_keys = &required.keys;
        let default_keys = match self.bundles.get(DEFAULT_LOCALE) {
            Some(bundle) => bundle.keys().cloned().collect::<BTreeSet<_>>(),
            None => BTreeSet::new(),
        };
        let default_locale_missing = required_keys
            .difference(&default_keys)
            .map(|key| key.as_str().into())
            .collect();
        let mut missing_by_locale = BTreeMap::new();
        let mut unused_by_locale = BTreeMap::new();
        for (locale, bundle) in &self.bundles {
            let present = bundle.keys().cloned().collect::<BTreeSet<_>>();
            missing_by_locale.insert(
                locale.clone(),
                required_keys
                    .difference(&present)
                    .map(|key| key.as_str().into())
                    .collect(),
            );
            unused_by_locale.insert(
                locale.clone(),
                present
                    .difference(required_keys)
                    .map(|key| key.as_str().into())
                    .collect(),
            );
        }
        I18nBundleInventory {
            default_locale_missing,
            missing_by_locale,
            unused_by_locale,
        }
    }

    pub fn validate(
        &self,
        required: &I18nManifest,
        alias_keys: &BTreeSet<I18nKey>,
    ) -> Result<I18nBundleInventory, I18nBundleValidationError> {
        let inventory = self.audit(required, alias_keys);
        if !inventory.default_locale_missing.is_empty() {
            return Err(I18nBundleValidationError::MissingDefaultLocale {
                keys: inventory.default_locale_missing.clone(),
            });
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
        Ok(inventory)
    }

    fn static_item(
        &self,
        protocol: &crate::node_system::protocol::NodeProtocol,
        locale: &str,
        creation: NodeCreationDescriptor,
    ) -> LocalizedCatalogItemDto {
        let title = self.text(locale, &protocol.catalog.title_key);
        let description = protocol
            .catalog
            .description_key
            .as_ref()
            .map(|key| self.text(locale, key));
        let documentation =
            crate::node_system::catalog::documentation::documentation(&protocol.type_id, locale)
                .map(Into::into);
        let aliases = match protocol.catalog.aliases_key.as_ref() {
            Some(key) => self.aliases(locale, key),
            None => Vec::new(),
        };
        let technical_terms = self.technical_terms(protocol);
        let backend_search_text = [title.clone()]
            .into_iter()
            .chain(aliases.iter().cloned())
            .collect();
        LocalizedCatalogItemDto {
            node_type_id: protocol.type_id.as_str().into(),
            title,
            description,
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
        protocol: &crate::node_system::protocol::NodeProtocol,
        locale: &str,
    ) -> LocalizedCatalogItemDto {
        let description = protocol
            .catalog
            .description_key
            .as_ref()
            .map(|key| self.text(locale, key));
        let documentation =
            crate::node_system::catalog::documentation::documentation(&protocol.type_id, locale)
                .map(Into::into);
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
        LocalizedCatalogItemDto {
            node_type_id: entry.node_type_id.as_str().into(),
            title: entry.name.clone(),
            description,
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
            creation: NodeCreationDescriptor::ResourceBound {
                node_type_id: entry.node_type_id.clone(),
                resource_path: entry.resource_path.clone(),
                resource_revision: entry.resource_revision,
                create_args: entry.create_args,
            },
        }
    }

    fn project_ports(
        protocol: &crate::node_system::protocol::NodeProtocol,
    ) -> Vec<LocalizedPortDto> {
        protocol
            .interface
            .ports
            .iter()
            .map(|port| LocalizedPortDto {
                key: port.key.as_str().into(),
                label: port.title.clone(),
                direction: match port.direction {
                    crate::node_system::protocol::PortDirection::Input => "input".into(),
                    crate::node_system::protocol::PortDirection::Output => "output".into(),
                },
                kind: match port.kind {
                    crate::node_system::protocol::PortKind::Data => "data".into(),
                    crate::node_system::protocol::PortKind::Control => "control".into(),
                    crate::node_system::protocol::PortKind::Effect => "effect".into(),
                },
            })
            .collect()
    }

    fn localized_parameters(
        &self,
        protocol: &crate::node_system::protocol::NodeProtocol,
        locale: &str,
    ) -> Vec<LocalizedParameterDto> {
        protocol
            .parameters
            .parameters
            .iter()
            .map(|parameter| LocalizedParameterDto {
                key: parameter.key.as_str().into(),
                title: self.text(locale, &parameter.title_key),
                description: parameter
                    .description_key
                    .as_ref()
                    .map(|key| self.text(locale, key)),
            })
            .collect()
    }

    fn technical_terms(
        &self,
        protocol: &crate::node_system::protocol::NodeProtocol,
    ) -> Vec<Box<str>> {
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
        locale_chain(locale).into_iter().find_map(|candidate| {
            self.bundles
                .get(candidate.as_str())
                .or_else(|| {
                    (!candidate.contains('-'))
                        .then(|| {
                            self.bundles
                                .iter()
                                .find(|(name, _)| {
                                    name.split('-').next() == Some(candidate.as_str())
                                })
                                .map(|(_, bundle)| bundle)
                        })
                        .flatten()
                })?
                .get(key)
        })
    }

    fn text(&self, locale: &str, key: &I18nKey) -> Box<str> {
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

impl LocalizationLookup for BuiltinLocalizationBundle<'_> {
    fn text(&self, key: &I18nKey, arguments: &DiagnosticArguments) -> Box<str> {
        render_template(&self.catalog.text(&self.locale, key), arguments).into()
    }
}

fn render_template(template: &str, arguments: &DiagnosticArguments) -> String {
    let mut rendered = String::with_capacity(template.len());
    let mut cursor = 0;

    while let Some(relative_open) = template[cursor..].find('{') {
        let open = cursor + relative_open;
        rendered.push_str(&template[cursor..open]);
        let name_start = open + 1;
        let Some(relative_close) = template[name_start..].find('}') else {
            rendered.push_str(&template[open..]);
            return rendered;
        };
        let close = name_start + relative_close;
        let name = &template[name_start..close];
        if let Some(value) = arguments.get(name) {
            rendered.push_str(value);
        } else {
            rendered.push_str(&template[open..=close]);
        }
        cursor = close + 1;
    }

    rendered.push_str(&template[cursor..]);
    rendered
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

pub fn normalize_search_text(value: &str) -> Box<str> {
    let mut output = String::with_capacity(value.len());
    let mut separated = true;
    for original in value.chars() {
        if is_combining_mark(original) {
            continue;
        }
        let folded = fold_width(original);
        if let Some(replacement) = fold_latin_diacritic(folded) {
            push_search_char(&mut output, replacement, &mut separated);
            continue;
        }
        if folded == 'ß' || folded == 'ẞ' {
            push_search_char(&mut output, 's', &mut separated);
            push_search_char(&mut output, 's', &mut separated);
            continue;
        }
        for character in folded.to_lowercase() {
            push_search_char(&mut output, character, &mut separated);
        }
    }
    output.trim_end().into()
}

fn search<'a>(parts: impl IntoIterator<Item = &'a str>) -> Box<str> {
    normalize_search_text(
        &parts
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn push_search_char(output: &mut String, character: char, separated: &mut bool) {
    if character.is_alphanumeric() || character == '_' {
        output.push(character);
        *separated = false;
    } else if !*separated {
        output.push(' ');
        *separated = true;
    }
}

fn fold_width(character: char) -> char {
    match character {
        '\u{3000}' => ' ',
        '\u{ff01}'..='\u{ff5e}' => match char::from_u32(character as u32 - 0xfee0) {
            Some(folded) => folded,
            None => character,
        },
        _ => character,
    }
}

fn is_combining_mark(character: char) -> bool {
    ('\u{0300}'..='\u{036f}').contains(&character)
        || ('\u{1ab0}'..='\u{1aff}').contains(&character)
        || ('\u{1dc0}'..='\u{1dff}').contains(&character)
        || ('\u{20d0}'..='\u{20ff}').contains(&character)
        || ('\u{fe20}'..='\u{fe2f}').contains(&character)
}

fn fold_latin_diacritic(character: char) -> Option<char> {
    Some(match character {
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'Ç' | 'ç' => 'c',
        'È' | 'É' | 'Ê' | 'Ë' | 'è' | 'é' | 'ê' | 'ë' => 'e',
        'Ì' | 'Í' | 'Î' | 'Ï' | 'ì' | 'í' | 'î' | 'ï' => 'i',
        'Ñ' | 'ñ' => 'n',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'ù' | 'ú' | 'û' | 'ü' => 'u',
        'Ý' | 'Ÿ' | 'ý' | 'ÿ' => 'y',
        _ => return None,
    })
}

pub(crate) use Message::{Aliases, Text};
