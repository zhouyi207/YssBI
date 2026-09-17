mod debug;
mod math;
pub(super) mod reroute;
mod support;
mod value;

use super::builtin::{BuiltinAssemblyError, ProviderFragment};
use support::{category, i18n, semantic};

use yss_node_protocol::TypeConstructorId;
use yss_node_registry::TypeConstructorRegistration;

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut fragment = ProviderFragment::default();
    fragment
        .type_constructors
        .push(TypeConstructorRegistration {
            id: semantic("core.data_series", TypeConstructorId::new)?,
            title_key: i18n("types.data_series.title")?,
            arity: 1,
        });
    fragment
        .categories
        .push(category("debug", "categories.debug.title", 60)?);
    for (key, en, zh) in [
        ("types.categorical.title", "Categorical", "分类"),
        ("types.data_series.title", "DataSeries", "数据序列"),
        ("categories.debug.title", "Debug", "调试"),
    ] {
        let key = i18n(key)?;
        fragment.text("en-US", key.clone(), en);
        fragment.text("zh-CN", key, zh);
    }

    value::register(&mut fragment)?;
    math::register(&mut fragment)?;
    debug::register(&mut fragment)?;
    reroute::register(&mut fragment)?;
    Ok(fragment)
}
