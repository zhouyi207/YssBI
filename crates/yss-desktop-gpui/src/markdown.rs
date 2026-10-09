//! Shared native rich-text policy; project-specific navigation stays with its caller.
pub(crate) mod links;
mod math;
mod typesetting;

use std::sync::Arc;

use gpui::{App, ClickEvent, MouseButton, Overflow, SharedString, StyleRefinement, Window};
use gpui_component::text::{SelectionFormat, TextView, TextViewPlugin, TextViewStyle};

type LinkHandler = dyn Fn(&SharedString, &mut Window, &mut App) + Send + Sync;

#[derive(Clone)]
pub(crate) struct Markdown {
    link_handler: Arc<LinkHandler>,
}

impl Default for Markdown {
    fn default() -> Self {
        Self {
            link_handler: Arc::new(links::open_external),
        }
    }
}

impl Markdown {
    pub fn on_link_click(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.link_handler = Arc::new(handler);
        self
    }
}

impl TextViewPlugin for Markdown {
    fn setup(self, view: TextView) -> TextView {
        let prose = self.clone();
        let extensions = math::extensions()
            .block_renderer("yss-display-math", move |node, window, cx| {
                math::render(node, &prose, window, cx)
            });
        let mut table = StyleRefinement::default();
        table.overflow.x = Some(Overflow::Scroll);
        view.markdown_extensions(extensions)
            .selectable(true)
            .selection_format(SelectionFormat::Plain)
            .scrollable(false)
            .style(TextViewStyle::default().table(table))
            .on_link_click(move |url, event, window, cx| {
                let activate = match event {
                    ClickEvent::Mouse(click) => {
                        matches!(click.up.button, MouseButton::Left | MouseButton::Middle)
                    }
                    ClickEvent::Keyboard(_) => true,
                    ClickEvent::Touch(click) => !click.long_press,
                };
                if activate {
                    (self.link_handler)(url, window, cx);
                }
            })
    }
}
