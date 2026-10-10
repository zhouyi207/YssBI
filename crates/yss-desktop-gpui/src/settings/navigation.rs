//! Searchable category navigation; provider pages belong to the AI category.
use super::{Page, SettingsPanel};
use crate::text::t;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Selectable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    sidebar::{Sidebar, SidebarHeader, SidebarMenu, SidebarMenuItem},
};
use gpui_kit::{AnyElement, Context, IntoElement, div, prelude::*, px};

impl Page {
    pub(super) fn category(self) -> Self {
        match self {
            Self::Providers | Self::Provider => Self::Overview,
            page => page,
        }
    }
}

impl SettingsPanel {
    pub(super) fn visible_categories(
        &self,
        cx: &gpui_kit::App,
    ) -> Vec<(Page, &'static str, IconName)> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        [
            (Page::Overview, "settings.sections.ai", IconName::Bot),
            (
                Page::Knowledge,
                "settings.knowledge.title",
                IconName::Search,
            ),
            (
                Page::Appearance,
                "settings.sections.appearance",
                IconName::Languages,
            ),
        ]
        .into_iter()
        .filter(|(page, key, _)| {
            let related = match page {
                Page::Appearance => ["settings.labels.language", "language.zhCN", "language.enUS"],
                Page::Knowledge => [
                    "settings.knowledge.document",
                    "settings.knowledge.description",
                    "settings.knowledge.rebuild",
                ],
                _ => [
                    "settings.models.providers",
                    "settings.models.models",
                    "settings.models.defaultModel",
                ],
            };
            std::iter::once(*key)
                .chain(related)
                .any(|key| t(key).to_lowercase().contains(&query))
        })
        .collect()
    }

    pub(super) fn navigation(
        &self,
        compact: bool,
        width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let search = div().p_3().child(
            Input::new(&self.search)
                .small()
                .prefix(Icon::new(IconName::Search).size_4())
                .cleanable(true)
                .disabled(self.busy()),
        );
        let categories = self.visible_categories(cx);
        if compact {
            let mut navigation = div().flex().gap_2().px_4().pb_2();
            for (page, key, icon) in categories {
                navigation = navigation.child(
                    Button::new(key)
                        .small()
                        .ghost()
                        .icon(icon)
                        .label(t(key))
                        .selected(self.page.category() == page)
                        .disabled(self.busy())
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.navigate_category(page, window, cx)
                        })),
                );
            }
            return div()
                .flex_shrink_0()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(search)
                .child(navigation)
                .into_any_element();
        }
        let mut menu = SidebarMenu::new();
        for (page, key, icon) in categories {
            menu = menu.child(
                SidebarMenuItem::new(t(key))
                    .icon(icon)
                    .active(self.page.category() == page)
                    .disable(self.busy())
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.navigate_category(page, window, cx)
                    })),
            );
        }
        Sidebar::new("settings-sidebar")
            .collapsible(false)
            .w(px(width))
            .border_r_1()
            .border_color(cx.theme().border)
            .header(SidebarHeader::new().p_0().child(search))
            .child(menu)
            .into_any_element()
    }

    fn navigate_category(
        &mut self,
        page: Page,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        // Selecting the current category keeps its nested page and unsubmitted input.
        if self.page.category() != page {
            self.navigate(page, window, cx);
        }
    }
}
