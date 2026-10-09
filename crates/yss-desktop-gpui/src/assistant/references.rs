//! Draft and recorded references share the accepted project catalog and opening route.
mod picker;

use super::{ConversationEvent, ConversationPanel};
use crate::project::resources::ResourceCatalog;
use gpui::{AnyElement, Context, SharedString, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    tooltip::Tooltip,
};
use gpui_kit_assets::IconName;
use std::sync::Arc;
use yss_project_identity::ProjectResourceRef;

impl ConversationPanel {
    pub(crate) fn set_resource_catalog(
        &mut self,
        catalog: Option<Arc<ResourceCatalog>>,
        cx: &mut Context<Self>,
    ) {
        let catalog = catalog
            .filter(|catalog| &catalog.project == self.session.project.project_instance_id());
        let same = match (&self.resource_catalog, &catalog) {
            (Some(old), Some(new)) => Arc::ptr_eq(old, new),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.resource_catalog = catalog;
            cx.notify();
        }
    }

    fn reference_available(&self, resource: &ProjectResourceRef) -> bool {
        self.resource_catalog
            .as_ref()
            .is_some_and(|catalog| catalog.get(resource).is_some())
    }

    fn toggle_reference(&mut self, resource: &ProjectResourceRef, cx: &mut Context<Self>) {
        if self.references.contains(resource) {
            self.references.retain(|item| item != resource);
        } else if self.reference_available(resource) {
            self.references.push(resource.clone());
        }
        cx.notify();
    }

    fn open_reference(&self, resource: &ProjectResourceRef, cx: &mut Context<Self>) {
        if self.reference_available(resource) {
            cx.emit(ConversationEvent::OpenResource(resource.clone()));
        }
    }

    pub(super) fn reference_chips<'a>(
        &self,
        scope: impl Into<SharedString>,
        resources: impl IntoIterator<Item = (&'a ProjectResourceRef, Option<&'a str>)>,
        removable: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut chips = div().id(scope.into()).min_w_0().flex().flex_wrap().gap_1();
        for (index, (resource, recorded_name)) in resources.into_iter().enumerate() {
            let current = self
                .resource_catalog
                .as_ref()
                .and_then(|catalog| catalog.get(resource));
            let name = recorded_name
                .or_else(|| current.map(|entry| entry.name.as_str()))
                .unwrap_or(&resource.id)
                .to_owned();
            let available = current.is_some();
            let path = resource.id.clone();
            let open = resource.clone();
            let remove = resource.clone();
            let remove_label =
                crate::text::format("panel.assistantRemoveReference", &[("name", name.clone())]);
            chips = chips.child(
                div()
                    .id(("reference", index))
                    .flex()
                    .items_center()
                    .min_w_0()
                    .max_w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .tooltip(move |window, cx| {
                        let path = path.clone();
                        Tooltip::element(move |_, cx| {
                            div().flex().flex_col().gap_1().child(path.clone()).when(
                                !available,
                                |tip| {
                                    tip.child(div().text_color(cx.theme().warning).child(
                                        crate::text::t("panel.assistantResourceUnavailable"),
                                    ))
                                },
                            )
                        })
                        .build(window, cx)
                    })
                    .child(
                        Button::new(("open-reference", index))
                            .xsmall()
                            .ghost()
                            .max_w(px(180.))
                            .icon(IconName::AtSign)
                            .label(name)
                            .disabled(!available)
                            .on_click(
                                cx.listener(move |view, _, _, cx| view.open_reference(&open, cx)),
                            ),
                    )
                    .when(!available, |chip| {
                        chip.child(
                            Icon::new(IconName::TriangleAlert)
                                .xsmall()
                                .text_color(cx.theme().warning),
                        )
                    })
                    .when(removable, |chip| {
                        chip.child(
                            Button::new(("remove-reference", index))
                                .xsmall()
                                .ghost()
                                .icon(IconName::X)
                                .tooltip(remove_label.clone())
                                .accessibility_label(remove_label)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.references.retain(|item| item != &remove);
                                    cx.notify();
                                })),
                        )
                    }),
            );
        }
        chips.into_any_element()
    }
}
