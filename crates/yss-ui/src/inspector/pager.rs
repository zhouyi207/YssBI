use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Disableable, button::Button};
use gpui_kit::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*};
use std::rc::Rc;

type PageHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

/// Navigation for a bounded collection page. The owner stores the page; `on_change` receives
/// the requested zero-based page. A single page renders nothing.
#[derive(IntoElement)]
pub struct Pager {
    id: ElementId,
    page: usize,
    pages: usize,
    label: Option<SharedString>,
    disabled: bool,
    on_change: Option<PageHandler>,
}

impl Pager {
    pub fn new(id: impl Into<ElementId>, page: usize, pages: usize) -> Self {
        Self {
            id: id.into(),
            page,
            pages,
            label: None,
            disabled: false,
            on_change: None,
        }
    }

    /// Replaces the default "page / pages" position, for example with an item range.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn on_change(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Disableable for Pager {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Pager {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.pages <= 1 {
            return div().into_any_element();
        }
        let page = self.page.min(self.pages - 1);
        let on_change = self.on_change;
        let step = |target: usize, button: Button| match &on_change {
            Some(handler) => {
                let handler = handler.clone();
                button.on_click(move |_, window, cx| handler(&target, window, cx))
            }
            None => button,
        };
        div()
            .id(self.id)
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_0p5()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(step(
                page.saturating_sub(1),
                super::action_button(
                    "previous",
                    IconName::ChevronLeft,
                    crate::text::translate("databaseEditor.previousPage"),
                )
                .disabled(self.disabled || page == 0),
            ))
            .child(
                div().px_1().child(
                    self.label
                        .unwrap_or_else(|| format!("{} / {}", page + 1, self.pages).into()),
                ),
            )
            .child(step(
                (page + 1).min(self.pages - 1),
                super::action_button(
                    "next",
                    IconName::ChevronRight,
                    crate::text::translate("databaseEditor.nextPage"),
                )
                .disabled(self.disabled || page + 1 >= self.pages),
            ))
            .into_any_element()
    }
}
