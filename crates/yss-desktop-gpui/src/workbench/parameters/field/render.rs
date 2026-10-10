//! Field layout consumes current projections; diagnostic text shares the Problems formatter.
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
};
use std::collections::BTreeMap;
use yss_graph_analysis_contract::DiagnosticLocation;
use yss_graph_editor::projection::{EditorDiagnosticModel, EditorDiagnosticSeverity};

impl ParameterForm {
    pub(in crate::workbench::parameters) fn render_parameters(
        &self,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut content = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .border_b_1()
            .border_color(cx.theme().border);
        if self.fields.is_empty() {
            return content
                .child(controls::hint(
                    crate::text::translate("native.workbench.noParameters"),
                    cx,
                ))
                .into_any_element();
        }
        let mut diagnostics: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for diagnostic in &self.diagnostics {
            if let DiagnosticLocation::Parameter { key, .. } = &diagnostic.location {
                diagnostics.entry(key).or_default().push(diagnostic);
            }
        }
        for group in &self.groups {
            let fields = group
                .fields
                .clone()
                .map(|index| {
                    self.render_parameter(
                        index,
                        diagnostics
                            .get(&self.fields[index].model.key)
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                        busy,
                        cx,
                    )
                })
                .collect::<Vec<_>>();
            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child(group.display.title.to_string()),
                    )
                    .children(
                        group
                            .display
                            .description
                            .as_deref()
                            .map(|text| controls::hint(text, cx)),
                    )
                    .children(fields),
            );
        }
        content.into_any_element()
    }

    fn render_parameter(
        &self,
        index: usize,
        diagnostics: &[&EditorDiagnosticModel],
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let field = &self.fields[index];
        let epoch = self.epoch;
        let editor = match &field.draft {
            ParameterDraft::Toggle => Checkbox::new(("parameter-toggle", index))
                .label(crate::text::translate("plugins.enable"))
                .checked(field.model.value == Some(Value::Bool(true)))
                .disabled(busy)
                .on_click(cx.listener(move |view, value: &bool, _, cx| {
                    if view.accepts_input(epoch, cx) {
                        view.commit_parameter(index, Value::Bool(*value), cx)
                    }
                }))
                .into_any_element(),
            ParameterDraft::Select | ParameterDraft::Constant => {
                self.render_parameter_choice(index, busy, cx)
            }
            ParameterDraft::Relational(draft) => self.render_relational(index, draft, busy, cx),
            ParameterDraft::Domain(draft) => self.render_domain(index, draft, busy, cx),
            ParameterDraft::List(draft) => {
                self.render_list_parameter(index, draft, busy, "conversion.addValue", cx)
            }
            ParameterDraft::Text { input, .. } => div()
                .flex()
                .items_start()
                .gap_1()
                .child(input.render(busy))
                .child(
                    controls::apply(("parameter-apply", index), busy).on_click(cx.listener(
                        move |view, _, _, cx| {
                            if view.accepts_input(epoch, cx) {
                                view.apply_parameter(index, cx)
                            }
                        },
                    )),
                )
                .into_any_element(),
        };
        let content = div()
            .on_action(cx.listener(
                move |view, _: &gpui_kit::component::input::Escape, window, cx| {
                    if view.accepts_input(epoch, cx) {
                        view.restore_parameter(index, window, cx);
                        cx.stop_propagation();
                    }
                },
            ))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .child(field.model.display.title.to_string()),
                    )
                    .when(
                        matches!(
                            field.draft,
                            ParameterDraft::Text { .. }
                                | ParameterDraft::List(_)
                                | ParameterDraft::Domain(_)
                                | ParameterDraft::Relational(_)
                        ),
                        |header| {
                            header.child(
                                Button::new(("parameter-restore", index))
                                    .small()
                                    .ghost()
                                    .icon(IconName::RotateCcw)
                                    .tooltip(crate::text::translate("common.restore"))
                                    .disabled(busy)
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        if view.accepts_input(epoch, cx) {
                                            view.restore_parameter(index, window, cx);
                                        }
                                    })),
                            )
                        },
                    )
                    .child(
                        Button::new(("parameter-reset", index))
                            .small()
                            .ghost()
                            .icon(IconName::Undo2)
                            .tooltip(crate::text::translate("native.workbench.resetParameter"))
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.accepts_input(epoch, cx) {
                                    view.restore_parameter(index, window, cx);
                                    view.commit_parameter(index, Value::Null, cx)
                                }
                            })),
                    ),
            )
            .children(
                field
                    .model
                    .display
                    .description
                    .as_deref()
                    .map(|text| controls::hint(text, cx)),
            )
            .child(self.reveal_editor(index, editor))
            .children(diagnostics.iter().map(|diagnostic| {
                let color = match diagnostic.severity {
                    EditorDiagnosticSeverity::Error => cx.theme().danger,
                    EditorDiagnosticSeverity::Warning => cx.theme().warning,
                    EditorDiagnosticSeverity::Information => cx.theme().muted_foreground,
                };
                div()
                    .text_xs()
                    .text_color(color)
                    .child(crate::text::graph_diagnostic(diagnostic))
            }))
            .children(field.error.as_ref().map(|error| {
                div()
                    .id(("parameter-error", index))
                    .role(gpui_kit::accesskit::Role::Alert)
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .into_any_element();
        self.reveal_field(index, content, cx)
    }
}
