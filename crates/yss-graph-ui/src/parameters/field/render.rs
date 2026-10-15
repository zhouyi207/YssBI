//! Field layout consumes current projections; diagnostic text shares the Problems formatter.
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{Disableable, Sizable, switch::Switch};
use std::collections::BTreeMap;
use yss_graph_analysis_contract::DiagnosticLocation;
use yss_graph_editor::projection::EditorDiagnosticModel;
use yss_ui::inspector::{self, InspectorSection, PropertyField};
use yss_ui::text::translate;

impl ParameterForm {
    /// Each parameter group is its own section, so a node never shows a group title under a
    /// redundant "Parameters" heading.
    pub(in crate::parameters) fn render_parameters(
        &self,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.fields.is_empty() {
            return InspectorSection::new("parameters", translate("detail.sections.parameters"))
                .child(inspector::hint(
                    translate("native.workbench.noParameters"),
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
        let mut sections = div().min_w_0().flex().flex_col();
        for (group_index, group) in self.groups.iter().enumerate() {
            let mut section = InspectorSection::new(
                ("parameter-group", group_index),
                group.display.title.to_string(),
            )
            .when_some(group.display.description.as_deref(), |section, text| {
                section.description(text.to_owned())
            });
            for index in group.fields.clone() {
                section = section.child(
                    self.render_parameter(
                        index,
                        diagnostics
                            .get(&self.fields[index].model.key)
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                        busy,
                        cx,
                    ),
                );
            }
            sections = sections.child(section);
        }
        sections.into_any_element()
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
        let title = field.model.display.title.to_string();
        let editor = match &field.draft {
            ParameterDraft::Toggle => Switch::new(("parameter-toggle", index))
                .small()
                .checked(field.model.value == Some(Value::Bool(true)))
                .disabled(busy)
                .accessibility_label(title.clone())
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
                    inspector::apply_button(("parameter-apply", index), busy).on_click(
                        cx.listener(move |view, _, _, cx| {
                            if view.accepts_input(epoch, cx) {
                                view.apply_parameter(index, cx)
                            }
                        }),
                    ),
                )
                .into_any_element(),
        };
        // Discarding only means something while an unapplied draft or its error is showing.
        let restorable = matches!(
            field.draft,
            ParameterDraft::Text { .. }
                | ParameterDraft::List(_)
                | ParameterDraft::Domain(_)
                | ParameterDraft::Relational(_)
        ) && (field.dirty || field.error.is_some());
        let mut property = PropertyField::new(title)
            .when_some(
                field.model.display.description.as_deref(),
                |property, text| property.description(text.to_owned()),
            )
            .when(restorable, |property| {
                property.action(
                    inspector::action_button(
                        ("parameter-restore", index),
                        IconName::RotateCcw,
                        translate("common.restore"),
                    )
                    .disabled(busy)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if view.accepts_input(epoch, cx) {
                            view.restore_parameter(index, window, cx);
                        }
                    })),
                )
            })
            .action(
                inspector::action_button(
                    ("parameter-reset", index),
                    IconName::Undo2,
                    translate("native.workbench.resetParameter"),
                )
                .disabled(busy)
                .on_click(cx.listener(move |view, _, window, cx| {
                    if view.accepts_input(epoch, cx) {
                        view.restore_parameter(index, window, cx);
                        view.commit_parameter(index, Value::Null, cx)
                    }
                })),
            )
            .control(self.reveal_editor(index, editor))
            .when(
                matches!(field.draft, ParameterDraft::Toggle),
                PropertyField::inline,
            );
        for diagnostic in diagnostics {
            property = property.message(crate::feedback::diagnostic_message(diagnostic));
        }

        let content = div()
            .on_action(cx.listener(
                move |view, _: &gpui_kit::component::input::Escape, window, cx| {
                    if view.accepts_input(epoch, cx) {
                        view.restore_parameter(index, window, cx);
                        cx.stop_propagation();
                    }
                },
            ))
            .child(property);
        self.reveal_field(index, content, cx)
    }
}
