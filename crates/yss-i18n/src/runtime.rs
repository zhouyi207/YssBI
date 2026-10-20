use std::borrow::Cow;

use smallvec::SmallVec;

use crate::{Message, StaticCatalog, plural_category, resolve_locale};

/// The message under `key` in the locale that `locale` resolves to.
pub fn message(catalog: StaticCatalog, locale: &str, key: &str) -> Option<Message> {
    let locale = resolve_locale(locale);
    let (_, messages) = catalog.iter().find(|(candidate, _)| *candidate == locale)?;
    let index = messages
        .binary_search_by(|(candidate, _)| (*candidate).cmp(key))
        .ok()?;
    messages.get(index).map(|(_, message)| *message)
}

/// The text under `key`, or `None` when the catalog has no such text. Generated catalogs
/// define every key in every supported locale, so only keys derived from runtime codes miss.
pub fn lookup(catalog: StaticCatalog, locale: &str, key: &str) -> Option<&'static str> {
    match message(catalog, locale, key)? {
        Message::Text(text) => Some(text),
        Message::List(_) | Message::Plural(_) => None,
    }
}

/// The text under `key`, or the key itself so that a missing message stays visible.
pub fn translate<'a>(catalog: StaticCatalog, locale: &str, key: &'a str) -> &'a str {
    lookup(catalog, locale, key).unwrap_or(key)
}

pub fn list(catalog: StaticCatalog, locale: &str, key: &str) -> Option<&'static [&'static str]> {
    match message(catalog, locale, key)? {
        Message::List(items) => Some(items),
        Message::Text(_) | Message::Plural(_) => None,
    }
}

/// The form of a plural message that the rule of `locale` selects for `count`.
pub fn plural(
    catalog: StaticCatalog,
    locale: &str,
    key: &str,
    count: usize,
) -> Option<&'static str> {
    let Message::Plural(forms) = message(catalog, locale, key)? else {
        return None;
    };
    let category = plural_category(locale, count);
    forms
        .iter()
        .find(|(form, _)| *form == category)
        .map(|(_, text)| *text)
}

pub fn format<S: AsRef<str>>(template: &str, arguments: &[(&str, S)]) -> String {
    let mut names = SmallVec::<[&str; 8]>::with_capacity(arguments.len());
    let mut values = SmallVec::<[Cow<'_, str>; 8]>::with_capacity(arguments.len());
    for (name, value) in arguments {
        names.push(name);
        values.push(Cow::Borrowed(value.as_ref()));
    }
    rust_i18n::replace_patterns_cow(template, &names, &values)
}
