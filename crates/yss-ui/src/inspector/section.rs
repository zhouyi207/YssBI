use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{
    AnyElement, App, ElementId, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString,
    Window, div, percentage, prelude::*, relative,
};
use std::rc::Rc;

use crate::appearance::HEADER_HEIGHT;

/// Identity band of the inspected object. It shares the editor toolbar's height, so the two
/// bands form one continuous line across the window.
#[derive(IntoElement)]
pub struct InspectorHeader {
    icon: IconName,
    title: SharedString,
    detail: Option<SharedString>,
    tools: Vec<AnyElement>,
}

impl InspectorHeader {
    pub fn new(icon: IconName, title: impl Into<SharedString>) -> Self {
        Self {
            icon,
            title: title.into(),
            detail: None,
            tools: Vec::new(),
        }
    }

    /// Secondary identity, such as the definition an instance was created from.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn tool(mut self, tool: impl IntoElement) -> Self {
        self.tools.push(tool.into_any_element());
        self
    }
}

impl RenderOnce for InspectorHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .h(HEADER_HEIGHT)
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Icon::new(self.icon)
                    .size_4()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.title),
            )
            .when_some(self.detail, |header, detail| {
                header.child(
                    div()
                        .min_w_0()
                        .max_w(relative(0.5))
                        .truncate()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                )
            })
            .child(div().flex_1())
            .children(self.tools)
    }
}

type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// A titled inspector region separated by its own hairline. The header keeps one trailing lane:
/// the disclosure chevron when the owner makes the section collapsible, otherwise its tools.
#[derive(IntoElement)]
pub struct InspectorSection {
    id: ElementId,
    title: SharedString,
    count: Option<usize>,
    description: Option<SharedString>,
    tools: Vec<AnyElement>,
    disclosure: Option<(bool, ToggleHandler)>,
    children: Vec<AnyElement>,
}

impl InspectorSection {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            count: None,
            description: None,
            tools: Vec::new(),
            disclosure: None,
            children: Vec::new(),
        }
    }

    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Tools render on static sections only; a collapsible header keeps its lane for the chevron.
    pub fn tool(mut self, tool: impl IntoElement) -> Self {
        self.tools.push(tool.into_any_element());
        self
    }

    /// Controlled disclosure: the owner stores `open`, and `on_toggle` receives the requested state.
    pub fn collapsible(
        mut self,
        open: bool,
        on_toggle: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.disclosure = Some((open, Rc::new(on_toggle)));
        self
    }
}

impl ParentElement for InspectorSection {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for InspectorSection {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let open = self.disclosure.as_ref().is_none_or(|(open, _)| *open);
        let heading = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap_1p5()
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.title.clone()),
            )
            .when_some(self.count, |heading, count| {
                heading.child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(count.to_string()),
                )
            });
        let header = match self.disclosure {
            Some((open, on_toggle)) => Button::new("header")
                .ghost()
                .w_full()
                .h_8()
                .px_3()
                .rounded_none()
                .accessibility_label(self.title)
                .on_click(move |_, window, cx| on_toggle(&!open, window, cx))
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(heading)
                        .child(
                            Icon::new(IconName::ChevronDown)
                                .xsmall()
                                .text_color(cx.theme().muted_foreground)
                                .rotate(percentage(if open { 0.5 } else { 0. })),
                        ),
                )
                .into_any_element(),
            None => div()
                .h_8()
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .child(heading)
                .children(self.tools)
                .into_any_element(),
        };
        let has_content = self.description.is_some() || !self.children.is_empty();
        div()
            .id(self.id)
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(header)
            .when(open && has_content, |section| {
                section.child(
                    div()
                        .min_w_0()
                        .px_3()
                        .pb_3()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .when_some(self.description, |content, description| {
                            content.child(super::hint(description, cx))
                        })
                        .children(self.children),
                )
            })
    }
}
