use gpui_kit::component::setting::{SettingField, SettingItem};
use gpui_kit::{Axis, Context, IntoElement, SharedString, div, prelude::*};

impl crate::settings::SettingsPanel {
    pub(in crate::settings) fn render_field<E: IntoElement>(
        &self,
        label: impl Into<SharedString>,
        description: impl Into<SharedString>,
        render: impl Fn(&Self, &mut Context<Self>) -> E + 'static,
        cx: &Context<Self>,
    ) -> SettingItem {
        let owner = cx.weak_entity();
        SettingItem::new(
            label,
            SettingField::render(move |options, _, cx| {
                let element = match owner.update(cx, |view, cx| render(view, cx).into_any_element())
                {
                    Ok(element) => element,
                    Err(_) => return div().into_any_element(),
                };
                div()
                    .w_64()
                    .max_w_full()
                    .when(options.layout() == Axis::Vertical, |field| field.w_full())
                    .child(element)
                    .into_any_element()
            }),
        )
        .description(description.into())
    }
}
