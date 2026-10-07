use super::{Page, SaveSettings, SettingsPanel, models::provider_name};
use gpui::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
    sidebar::{Sidebar, SidebarHeader, SidebarMenu, SidebarMenuItem},
};
use yss_harness_contract::{LanguageModelAuthentication, LanguageModelSelection};

impl Render for SettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("native-settings")
            .key_context("Settings")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .bg(cx.theme().background)
            .on_action(cx.listener(|view, _: &SaveSettings, window, cx| view.save(window, cx)))
            .child(
                Sidebar::new("settings-sidebar")
                    .collapsible(false)
                    .w(px(180.))
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .header(SidebarHeader::new().child("设置"))
                    .child(
                        SidebarMenu::new()
                            .child(
                                SidebarMenuItem::new("AI")
                                    .active(self.page == Page::Overview)
                                    .disable(self.busy())
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.navigate(Page::Overview, window, cx)
                                    })),
                            )
                            .child(
                                SidebarMenuItem::new("供应商与模型")
                                    .active(self.page != Page::Overview)
                                    .disable(self.busy())
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.navigate(Page::Providers, window, cx)
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(self.header(cx))
                    .child(self.status(cx))
                    .child(
                        div()
                            .id(("settings-content", self.epoch))
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_5()
                            .child(match self.page {
                                Page::Overview => self.overview(cx),
                                Page::Providers => self.providers(cx),
                                Page::Provider => self.provider_form(cx),
                            }),
                    ),
            )
    }
}
impl SettingsPanel {
    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = self
            .editor
            .as_ref()
            .map(|draft| draft.name.read(cx).value().to_string())
            .unwrap_or_default();
        let mut header =
            div()
                .h(px(52.))
                .flex_shrink_0()
                .px_5()
                .flex()
                .items_center()
                .gap_1()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    Button::new("breadcrumb-ai")
                        .small()
                        .ghost()
                        .label("AI")
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.navigate(Page::Overview, window, cx)
                        })),
                );
        if self.page != Page::Overview {
            header = header
                .child(Icon::new(IconName::ChevronRight).size_3())
                .child(
                    Button::new("breadcrumb-providers")
                        .small()
                        .ghost()
                        .label("供应商")
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.navigate(Page::Providers, window, cx)
                        })),
                );
        }
        if self.page == Page::Provider {
            header =
                header
                    .child(Icon::new(IconName::ChevronRight).size_3())
                    .child(div().flex_1().min_w_0().text_sm().truncate().child(
                        if title.is_empty() {
                            "新供应商".into()
                        } else {
                            title
                        },
                    ))
                    .child(
                        Button::new("settings-save")
                            .small()
                            .primary()
                            .label("保存")
                            .disabled(self.busy() || !self.dirty())
                            .on_click(cx.listener(|view, _, window, cx| view.save(window, cx))),
                    );
        } else {
            let presets = self
                .catalog
                .as_ref()
                .map(|catalog| catalog.presets.clone())
                .unwrap_or_default();
            let generation = self.generation;
            let owner = cx.entity().downgrade();
            header = header.child(div().flex_1()).child(
                Button::new("add-provider")
                    .small()
                    .primary()
                    .label("添加供应商")
                    .disabled(self.busy() || self.catalog.is_none())
                    .dropdown_menu(move |mut menu, _, _| {
                        for preset in &presets {
                            let preset = preset.clone();
                            let owner = owner.clone();
                            menu = menu.item(PopupMenuItem::new(preset.name.clone()).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if view.generation == generation {
                                            view.edit_provider(
                                                None,
                                                Some(preset.clone()),
                                                window,
                                                cx,
                                            );
                                        }
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            );
        }
        header.into_any_element()
    }

    fn status(&self, cx: &mut Context<Self>) -> AnyElement {
        let message = self
            .error
            .clone()
            .or_else(|| self.task.map(str::to_owned))
            .or_else(|| self.feedback.clone());
        let Some(message) = message else {
            return div().into_any_element();
        };
        div()
            .px_5()
            .py_2()
            .flex()
            .items_center()
            .gap_3()
            .flex_shrink_0()
            .bg(cx.theme().muted)
            .text_sm()
            .text_color(if self.error.is_some() {
                cx.theme().danger
            } else {
                cx.theme().muted_foreground
            })
            .child(div().flex_1().child(message))
            .when(self.load_failed, |view| {
                view.child(
                    Button::new("settings-retry")
                        .small()
                        .ghost()
                        .label("重试")
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, _, window, cx| view.reload(window, cx))),
                )
            })
            .into_any_element()
    }

    fn overview(&self, cx: &mut Context<Self>) -> AnyElement {
        let catalog = self.catalog.clone();
        let mut label = "尚未选择默认模型".to_owned();
        let mut options = vec![];
        if let Some(catalog) = &catalog {
            for provider in &catalog.providers {
                for model in &provider.config.models {
                    let selection = LanguageModelSelection {
                        provider_id: provider.config.id.clone(),
                        model_id: model.id.clone(),
                    };
                    let title = format!("{} / {}", provider_name(&provider.config), model.name);
                    if catalog.default_model.as_ref() == Some(&selection) {
                        label = title.clone();
                    }
                    let enabled = provider.has_api_key
                        || provider.config.authentication == LanguageModelAuthentication::None;
                    options.push((selection, title, enabled));
                }
            }
        }
        let generation = self.generation;
        let owner = cx.entity().downgrade();
        super::models::fields::field(
            "默认模型",
            "用于未单独指定模型的新对话。",
            Button::new("settings-default-model")
                .label(label)
                .disabled(self.busy() || options.is_empty())
                .dropdown_menu(move |mut menu, _, _| {
                    for (selection, title, enabled) in &options {
                        let owner = owner.clone();
                        let selection = selection.clone();
                        menu = menu.item(
                            PopupMenuItem::new(title.clone())
                                .disabled(!enabled)
                                .on_click(move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if view.generation == generation {
                                            view.set_default(selection.clone(), window, cx);
                                        }
                                    });
                                }),
                        );
                    }
                    menu
                }),
            cx,
        )
    }

    fn providers(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div().flex().flex_col().gap_3();
        let Some(catalog) = &self.catalog else {
            return content.into_any_element();
        };
        if catalog.providers.is_empty() {
            content = content.child(
                div()
                    .py_8()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("添加供应商后配置连接和模型。"),
            );
        }
        for provider in &catalog.providers {
            let provider = provider.clone();
            let label = provider_name(&provider.config);
            let info = format!(
                "{} 个模型 · {}",
                provider.config.models.len(),
                if provider.config.authentication == LanguageModelAuthentication::None {
                    "无需认证"
                } else if provider.has_api_key {
                    "已保存凭据"
                } else {
                    "待配置凭据"
                }
            );
            content = content.child(
                div()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().truncate().child(label))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(info),
                            ),
                    )
                    .child(
                        Button::new(gpui::SharedString::from(format!(
                            "provider-edit-{}",
                            provider.config.id
                        )))
                        .small()
                        .ghost()
                        .label("编辑")
                        .disabled(self.busy())
                        .on_click(cx.listener(
                            move |view, _, window, cx| {
                                view.edit_provider(Some(provider.clone()), None, window, cx)
                            },
                        )),
                    ),
            );
        }
        content.into_any_element()
    }
}
