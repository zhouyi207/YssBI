use crate::node_system::analysis::{DiagnosticArguments, LocalizationBundle};
use crate::node_system::document::GraphResourcePath;
use crate::node_system::protocol::{I18nKey, NodeTypeId};
use crate::node_system::registry::{I18nManifest, NodeRegistry};
use serde::Serialize;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCatalogDto {
    pub locale: Box<str>,
    pub categories: Vec<LocalizedCategoryDto>,
    pub items: Vec<LocalizedCatalogItemDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedCategoryDto {
    pub category_id: Box<str>,
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
    pub aliases: Vec<Box<str>>,
    pub technical_terms: Vec<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinyin: Option<Box<str>>,
    pub creation: NodeCreationDescriptor,
    pub search_text: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum NodeCreationDescriptor {
    #[serde(rename = "static")]
    Static {
        #[serde(rename = "nodeTypeId")]
        node_type_id: Box<str>,
    },
    #[serde(rename = "resourceBound")]
    ResourceBound {
        #[serde(rename = "nodeTypeId")]
        node_type_id: Box<str>,
        resource: GraphResourcePath,
        #[serde(rename = "createArgs")]
        create_args: ResourceBoundCreateArgsDto,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResourceBoundCreateArgsDto {
    Function,
    Variable,
    Resource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogResourceEntry {
    pub name: Box<str>,
    pub node_type_id: NodeTypeId,
    pub resource: GraphResourcePath,
    pub create_args: ResourceBoundCreateArgsDto,
    pub technical_terms: Vec<Box<str>>,
    pub pinyin: Option<Box<str>>,
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

impl BuiltinCatalog {
    pub(crate) fn new(entries: &[(&'static str, &'static str, Message)]) -> Self {
        let mut bundles = BTreeMap::<Box<str>, Bundle>::new();
        for (locale, key, message) in entries {
            bundles.entry((*locale).into()).or_default().insert(
                I18nKey::new(*key).expect("built-in i18n key"),
                message.clone(),
            );
        }
        Self { bundles }
    }

    pub fn localization(&self, locale: &str) -> BuiltinLocalizationBundle<'_> {
        BuiltinLocalizationBundle {
            catalog: self,
            locale: normalize_locale(locale),
        }
    }

    pub fn localize(&self, registry: &NodeRegistry, locale: &str) -> LocalizedCatalogDto {
        self.localize_with_resources(registry, locale, &[])
    }

    pub fn localize_with_resources(
        &self,
        registry: &NodeRegistry,
        locale: &str,
        resources: &[CatalogResourceEntry],
    ) -> LocalizedCatalogDto {
        let locale = normalize_locale(locale);
        let categories = registry
            .categories()
            .iter()
            .map(|(id, category)| {
                let title = self.text(&locale, &category.title_key);
                LocalizedCategoryDto {
                    category_id: id.as_str().into(),
                    search_text: search([id.as_str(), title.as_ref()]),
                    title,
                }
            })
            .collect();
        let mut items = registry
            .iter()
            .filter(|(_, node)| !node.protocol.catalog.hidden)
            .map(|(id, node)| self.static_item(id, &node.protocol, &locale))
            .collect::<Vec<_>>();
        items.extend(resources.iter().filter_map(|entry| {
            let node = registry.get(&entry.node_type_id)?;
            (!node.protocol.catalog.hidden)
                .then(|| self.resource_item(entry, &node.protocol, &locale))
        }));
        LocalizedCatalogDto {
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
        let default_keys = self
            .bundles
            .get(DEFAULT_LOCALE)
            .map(|bundle| bundle.keys().cloned().collect::<BTreeSet<_>>())
            .unwrap_or_default();
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
        id: &NodeTypeId,
        protocol: &crate::node_system::protocol::NodeProtocol,
        locale: &str,
    ) -> LocalizedCatalogItemDto {
        let title = self.text(locale, &protocol.catalog.title_key);
        let description = protocol
            .catalog
            .description_key
            .as_ref()
            .map(|key| self.text(locale, key));
        let documentation = protocol
            .catalog
            .documentation_key
            .as_ref()
            .map(|key| self.text(locale, key));
        let aliases = protocol
            .catalog
            .aliases_key
            .as_ref()
            .map(|key| self.aliases(locale, key))
            .unwrap_or_default();
        let technical_terms = self.technical_terms(protocol);
        let search_text = search(
            [
                id.as_str(),
                protocol.catalog.category_id.as_str(),
                title.as_ref(),
            ]
            .into_iter()
            .chain(aliases.iter().map(AsRef::as_ref))
            .chain(technical_terms.iter().map(AsRef::as_ref)),
        );
        LocalizedCatalogItemDto {
            node_type_id: id.as_str().into(),
            title,
            description,
            documentation,
            category_id: protocol.catalog.category_id.as_str().into(),
            aliases,
            technical_terms,
            pinyin: None,
            creation: NodeCreationDescriptor::Static {
                node_type_id: id.as_str().into(),
            },
            search_text,
        }
    }

    fn resource_item(
        &self,
        entry: &CatalogResourceEntry,
        protocol: &crate::node_system::protocol::NodeProtocol,
        locale: &str,
    ) -> LocalizedCatalogItemDto {
        let system_title = self.text(locale, &protocol.catalog.title_key);
        let description = protocol
            .catalog
            .description_key
            .as_ref()
            .map(|key| self.text(locale, key));
        let documentation = protocol
            .catalog
            .documentation_key
            .as_ref()
            .map(|key| self.text(locale, key));
        let aliases = protocol
            .catalog
            .aliases_key
            .as_ref()
            .map(|key| self.aliases(locale, key))
            .unwrap_or_default();
        let mut technical_terms = self.technical_terms(protocol);
        technical_terms.extend(entry.technical_terms.iter().cloned());
        technical_terms.sort();
        technical_terms.dedup();
        let pinyin = locale
            .split('-')
            .next()
            .is_some_and(|language| language.eq_ignore_ascii_case("zh"))
            .then(|| entry.pinyin.clone())
            .flatten();
        let search_text = search(
            [
                entry.name.as_ref(),
                entry.node_type_id.as_str(),
                protocol.catalog.category_id.as_str(),
                system_title.as_ref(),
            ]
            .into_iter()
            .chain(aliases.iter().map(AsRef::as_ref))
            .chain(technical_terms.iter().map(AsRef::as_ref))
            .chain(pinyin.iter().map(AsRef::as_ref)),
        );
        LocalizedCatalogItemDto {
            node_type_id: entry.node_type_id.as_str().into(),
            title: entry.name.clone(),
            description,
            documentation,
            category_id: protocol.catalog.category_id.as_str().into(),
            aliases,
            technical_terms,
            pinyin,
            creation: NodeCreationDescriptor::ResourceBound {
                node_type_id: entry.node_type_id.as_str().into(),
                resource: entry.resource.clone(),
                create_args: entry.create_args,
            },
            search_text,
        }
    }

    fn technical_terms(
        &self,
        protocol: &crate::node_system::protocol::NodeProtocol,
    ) -> Vec<Box<str>> {
        protocol
            .catalog
            .aliases_key
            .as_ref()
            .and_then(|key| self.bundles.get(DEFAULT_LOCALE)?.get(key))
            .and_then(|message| match message {
                Message::Aliases(values) => Some(
                    values
                        .iter()
                        .map(|value| Box::<str>::from(*value))
                        .collect(),
                ),
                Message::Text(_) => None,
            })
            .unwrap_or_default()
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

impl LocalizationBundle for BuiltinLocalizationBundle<'_> {
    fn text(&self, key: &I18nKey, arguments: &DiagnosticArguments) -> Box<str> {
        let mut message = self.catalog.text(&self.locale, key).into_string();
        for (name, value) in arguments {
            message = message.replace(&format!("{{{name}}}"), value);
        }
        message.into()
    }
}

fn normalize_locale(locale: &str) -> String {
    locale.trim().replace('_', "-")
}

fn locale_chain(locale: &str) -> Vec<String> {
    let normalized = normalize_locale(locale);
    let language = normalized
        .split('-')
        .next()
        .unwrap_or(&normalized)
        .to_owned();
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
        '\u{ff01}'..='\u{ff5e}' => char::from_u32(character as u32 - 0xfee0).unwrap(),
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
