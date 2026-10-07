//! Ordered code/label inputs for the existing ConversionDomain parameter.
use super::{DetailsPanel, controls, parameters::ParameterDraft};
use gpui::{AnyElement, Context, Entity, IntoElement, Subscription, Window, div, prelude::*};
use gpui_component::{
    Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use yss_data_contract::{ConversionDomain, SemanticValue};

struct DomainRow {
    code: Entity<InputState>,
    label: Entity<InputState>,
    _subscription: Subscription,
}

impl DomainRow {
    fn new(value: SemanticValue, window: &mut Window, cx: &mut Context<DetailsPanel>) -> Self {
        let code = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("编码")
                .default_value(value.value)
        });
        let label = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("标签")
                .default_value(value.label)
        });
        let subscription = cx.observe(&code, |_, _, cx| cx.notify());
        Self {
            code,
            label,
            _subscription: subscription,
        }
    }
}

pub(super) struct DomainDraft {
    rows: Vec<DomainRow>,
    positive: Option<usize>,
    page: usize,
}

impl DomainDraft {
    pub fn new(
        value: Option<&serde_json::Value>,
        window: &mut Window,
        cx: &mut Context<DetailsPanel>,
    ) -> Self {
        let domain = value
            .and_then(|value| serde_json::from_value::<ConversionDomain>(value.clone()).ok())
            .unwrap_or_default();
        let positive = domain.positive_value.as_ref().and_then(|positive| {
            domain
                .values
                .iter()
                .position(|value| &value.value == positive)
        });
        Self {
            rows: domain
                .values
                .into_iter()
                .map(|value| DomainRow::new(value, window, cx))
                .collect(),
            positive,
            page: 0,
        }
    }

    pub fn value(&self, cx: &gpui::App) -> Result<serde_json::Value, String> {
        let values = self
            .rows
            .iter()
            .map(|row| SemanticValue {
                value: row.code.read(cx).value().to_string(),
                label: row.label.read(cx).value().to_string(),
            })
            .collect::<Vec<_>>();
        let positive_value = self
            .positive
            .and_then(|index| values.get(index))
            .map(|value| value.value.clone());
        let domain = ConversionDomain {
            values,
            positive_value,
        };
        if !domain.is_valid() {
            return Err("请检查重复编码、正值选择及映射长度".into());
        }
        serde_json::to_value(domain).map_err(|_| "无法提交编码映射".into())
    }

    fn remove(&mut self, row: usize) {
        if row >= self.rows.len() {
            return;
        }
        self.rows.remove(row);
        self.positive = self.positive.and_then(|index| {
            if index == row {
                None
            } else {
                Some(if index > row { index - 1 } else { index })
            }
        });
    }

    fn move_row(&mut self, row: usize, target: usize) {
        if row >= self.rows.len() || target >= self.rows.len() {
            return;
        }
        self.rows.swap(row, target);
        self.positive = self.positive.map(|index| {
            if index == row {
                target
            } else if index == target {
                row
            } else {
                index
            }
        });
        self.page = target / 25;
    }
}

impl DetailsPanel {
    pub(super) fn render_domain(
        &self,
        index: usize,
        draft: &DomainDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let page = draft.page.min(draft.rows.len().saturating_sub(1) / 25);
        let positive = draft
            .positive
            .map(|row| row.to_string())
            .unwrap_or_default();
        let label = draft
            .positive
            .and_then(|row| draft.rows.get(row))
            .map(|row| format!("正值 · {}", row.code.read(cx).value()))
            .unwrap_or_else(|| "不指定正值".into());
        let mut options = vec![(String::new(), "不指定正值".into())];
        options.extend(
            draft
                .rows
                .iter()
                .enumerate()
                .map(|(row, value)| (row.to_string(), value.code.read(cx).value().to_string())),
        );
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(controls::hint("每项填写编码和标签，顺序用于等级映射", cx))
            .children(
                draft
                    .rows
                    .iter()
                    .enumerate()
                    .skip(page * 25)
                    .take(25)
                    .map(|(row, value)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_2()
                            .border_1()
                            .rounded_md()
                            .child(Input::new(&value.code).small().disabled(busy))
                            .child(Input::new(&value.label).small().disabled(busy))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Button::new(gpui::SharedString::from(format!(
                                            "domain-up-{index}-{row}"
                                        )))
                                        .small()
                                        .ghost()
                                        .icon(IconName::ChevronUp)
                                        .tooltip("上移")
                                        .disabled(busy || row == 0)
                                        .on_click(
                                            cx.listener(move |view, _, _, cx| {
                                                if view.accepts_input(epoch, cx)
                                                    && row > 0
                                                    && let ParameterDraft::Domain(draft) =
                                                        &mut view.fields[index].draft
                                                {
                                                    draft.move_row(row, row - 1);
                                                    cx.notify();
                                                }
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new(gpui::SharedString::from(format!(
                                            "domain-down-{index}-{row}"
                                        )))
                                        .small()
                                        .ghost()
                                        .icon(IconName::ChevronDown)
                                        .tooltip("下移")
                                        .disabled(busy || row + 1 == draft.rows.len())
                                        .on_click(
                                            cx.listener(move |view, _, _, cx| {
                                                if view.accepts_input(epoch, cx)
                                                    && let ParameterDraft::Domain(draft) =
                                                        &mut view.fields[index].draft
                                                {
                                                    draft.move_row(row, row + 1);
                                                    cx.notify();
                                                }
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new(gpui::SharedString::from(format!(
                                            "domain-remove-{index}-{row}"
                                        )))
                                        .small()
                                        .ghost()
                                        .icon(IconName::Close)
                                        .tooltip("移除")
                                        .disabled(busy)
                                        .on_click(
                                            cx.listener(move |view, _, _, cx| {
                                                if view.accepts_input(epoch, cx)
                                                    && let ParameterDraft::Domain(draft) =
                                                        &mut view.fields[index].draft
                                                {
                                                    draft.remove(row);
                                                    cx.notify();
                                                }
                                            }),
                                        ),
                                    ),
                            )
                    }),
            )
            .child(controls::choice(
                ("domain-positive", index),
                label,
                Some(positive),
                options,
                busy,
                cx.listener(move |view, value: &String, _, cx| {
                    if view.accepts_input(epoch, cx)
                        && let ParameterDraft::Domain(draft) = &mut view.fields[index].draft
                    {
                        draft.positive = value.parse().ok();
                        cx.notify();
                    }
                }),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(("domain-add", index))
                            .small()
                            .ghost()
                            .icon(IconName::Plus)
                            .label("添加映射")
                            .disabled(busy || draft.rows.len() >= ConversionDomain::MAX_VALUES)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let ParameterDraft::Domain(draft) =
                                        &mut view.fields[index].draft
                                {
                                    draft.rows.push(DomainRow::new(
                                        SemanticValue {
                                            value: String::new(),
                                            label: String::new(),
                                        },
                                        window,
                                        cx,
                                    ));
                                    draft.page = draft.rows.len().saturating_sub(1) / 25;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        controls::apply(("domain-apply", index), busy)
                            .tooltip("应用完整映射")
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(epoch, cx) {
                                    view.apply_parameter(index, cx)
                                }
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(("domain-prev", index))
                            .small()
                            .ghost()
                            .icon(IconName::ChevronLeft)
                            .disabled(busy || page == 0)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let ParameterDraft::Domain(draft) =
                                        &mut view.fields[index].draft
                                {
                                    draft.page = page.saturating_sub(1);
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        Button::new(("domain-next", index))
                            .small()
                            .ghost()
                            .icon(IconName::ChevronRight)
                            .disabled(busy || (page + 1) * 25 >= draft.rows.len())
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let ParameterDraft::Domain(draft) =
                                        &mut view.fields[index].draft
                                {
                                    draft.page = page + 1;
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
}
