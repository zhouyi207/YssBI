//! Dialog-only semantic drafts use complete, revision-bound column values.
use super::{DatabaseEditor, DatabaseEvent};
use gpui::{AppContext, Context, Entity, IntoElement, Render, WeakEntity, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    dialog::DialogButtonProps,
    input::{Input, InputState},
    menu::{DropdownMenu, PopupMenuItem},
};
use std::collections::{BTreeMap, HashSet};
use yss_application::database::DatabaseMutation;
use yss_data_contract::{ColumnSemantic, NumericConstraints, SemanticType, SemanticValue};
use yss_database_schema::DatabaseColumnFact;
use yss_project_identity::OperationId;

struct SemanticDialog {
    owner: WeakEntity<DatabaseEditor>,
    revision: yss_project_identity::ResourceRevision,
    column: String,
    draft: ColumnSemantic,
    labels: BTreeMap<usize, Entity<InputState>>,
    minimum: Entity<InputState>,
    maximum: Entity<InputState>,
    page: usize,
    loading: bool,
    values_ready: bool,
    saving: bool,
    error: Option<String>,
}
impl DatabaseEditor {
    pub(super) fn semantic_dialog(
        &self,
        column: DatabaseColumnFact,
        kind: SemanticType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft = column
            .semantic()
            .filter(|semantic| semantic.kind == kind)
            .cloned()
            .unwrap_or_else(|| ColumnSemantic::new(kind));
        let owner = cx.entity().downgrade();
        let revision = self.revision;
        let name = column.name().as_str().to_owned();
        let numeric = draft.numeric.clone().unwrap_or_default();
        let editor = cx.new(|cx| SemanticDialog {
            owner,
            revision,
            column: name,
            draft,
            labels: BTreeMap::new(),
            page: 0,
            minimum: cx.new(|cx| {
                InputState::new(window, cx).default_value(numeric.minimum.unwrap_or_default())
            }),
            maximum: cx.new(|cx| {
                InputState::new(window, cx).default_value(numeric.maximum.unwrap_or_default())
            }),
            loading: false,
            values_ready: true,
            saving: false,
            error: None,
        });
        if matches!(
            kind,
            SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
        ) {
            editor.update(cx, |editor, cx| editor.read_values(window, cx));
        }
        window.open_dialog(cx, move |dialog, _, _| {
            let save = editor.clone();
            dialog
                .title("列语义设置")
                .child(editor.clone())
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("应用")
                        .cancel_text("取消")
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    save.update(cx, |editor, cx| editor.confirm(window, cx));
                    false
                })
        });
    }
}
impl SemanticDialog {
    fn read_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let view = owner.read(cx);
        let project = view.project.clone();
        let id = view.id.clone();
        let column = self.column.clone();
        let revision = self.revision;
        let job = view.services.run(move |services| {
            Ok(services
                .application
                .query_column_values_for_application(project, id, revision, column)?)
        });
        self.loading = true;
        self.values_ready = false;
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, _, cx| {
                view.loading = false;
                if !view.current(cx) {
                    view.error = Some("数据已变化，请关闭后重新打开列设置。".into());
                } else if let Some(values) = result {
                    view.values_ready = true;
                    let mut known = view
                        .draft
                        .values
                        .iter()
                        .map(|entry| entry.value.clone())
                        .collect::<HashSet<_>>();
                    for value in values {
                        if known.insert(value.clone()) {
                            view.draft.values.push(SemanticValue {
                                label: value.clone(),
                                value,
                            });
                        }
                    }
                } else {
                    view.error = Some("列取值未读取，请关闭后重试。".into());
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn current(&self, cx: &gpui::App) -> bool {
        self.owner.upgrade().is_some_and(|owner| {
            let owner = owner.read(cx);
            owner.revision == self.revision && owner.ready && !owner.busy()
        })
    }
    fn collect(&self, cx: &gpui::App) -> ColumnSemantic {
        let mut draft = self.draft.clone();
        for (index, input) in &self.labels {
            if let Some(value) = draft.values.get_mut(*index) {
                value.label = input.read(cx).value().to_string();
            }
        }
        if draft.kind == SemanticType::Numeric {
            let minimum = self.minimum.read(cx).value().to_string();
            let maximum = self.maximum.read(cx).value().to_string();
            draft.numeric = Some(NumericConstraints {
                integer: draft
                    .numeric
                    .as_ref()
                    .is_some_and(|numeric| numeric.integer),
                minimum: (!minimum.trim().is_empty()).then_some(minimum),
                maximum: (!maximum.trim().is_empty()).then_some(maximum),
            });
        }
        draft
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.saving || !self.values_ready {
            return;
        }
        if !self.current(cx) {
            self.error = Some("数据已变化，请重新打开设置。".into());
            cx.notify();
            return;
        }
        let semantic = self.collect(cx);
        if semantic.kind == SemanticType::Binary && semantic.values.len() != 2 {
            self.error = Some("二元语义需要恰好两个取值。".into());
            cx.notify();
            return;
        }
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let view = owner.read(cx);
        let project = view.project.clone();
        let id = view.id.clone();
        let services = view.services.clone();
        let column = self.column.clone();
        let revision = self.revision;
        owner.update(cx, |view, cx| {
            view.mutating = true;
            view.changed(cx);
        });
        self.saving = true;
        self.error = None;
        let publisher = services.clone();
        let job = services.run(move |services| {
            let receipt = services.application.mutate_database_for_application(
                project,
                id,
                revision,
                OperationId::new(),
                DatabaseMutation::SetColumnSemantic { column, semantic },
            )?;
            publisher.publish_resource(receipt.mutation);
            Ok(receipt.data.edit_state)
        });
        cx.spawn_in(window, async move |dialog, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let failed = result.is_none();
            let _ = owner.update_in(cx, |view, window, cx| {
                view.mutating = false;
                if let Some(edit) = result {
                    view.edit = Some(edit);
                    view.ready = false;
                }
                if view.refresh_again {
                    view.reload(false, window, cx);
                }
                cx.emit(DatabaseEvent::Changed);
                cx.notify();
            });
            let _ = dialog.update_in(cx, |dialog, window, cx| {
                dialog.saving = false;
                if failed {
                    dialog.error =
                        Some("语义设置未提交，输入已保留，请检查约束或数据变化。".into());
                } else {
                    window.close_dialog(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn reorder(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        let target = index as isize + delta;
        if target < 0 || target >= self.draft.values.len() as isize {
            return;
        }
        self.draft = self.collect(cx);
        self.labels.clear();
        self.draft.values.swap(index, target as usize);
        cx.notify();
    }
}
impl Render for SemanticDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.loading || self.saving;
        let mut view = div().flex().flex_col().gap_3().child(
            div()
                .text_sm()
                .child(format!("{} · {}", self.column, self.draft.kind)),
        );
        if self.draft.kind == SemanticType::Numeric {
            let checked = self
                .draft
                .numeric
                .as_ref()
                .is_some_and(|numeric| numeric.integer);
            view = view
                .child(
                    Checkbox::new("numeric-integer")
                        .label("仅整数")
                        .checked(checked)
                        .disabled(busy)
                        .on_click(cx.listener(|view, checked: &bool, _, cx| {
                            view.draft
                                .numeric
                                .get_or_insert_with(NumericConstraints::default)
                                .integer = *checked;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("最小值（可留空）"),
                )
                .child(Input::new(&self.minimum).disabled(busy))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("最大值（可留空）"),
                )
                .child(Input::new(&self.maximum).disabled(busy));
        }
        let start = self.page * 20;
        for index in start..(start + 20).min(self.draft.values.len()) {
            let entry = &self.draft.values[index];
            let input = self
                .labels
                .entry(index)
                .or_insert_with(|| {
                    cx.new(|cx| InputState::new(window, cx).default_value(entry.label.clone()))
                })
                .clone();
            view = view.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w_32().text_xs().truncate().child(entry.value.clone()))
                    .child(Input::new(&input).small().flex_1().disabled(busy))
                    .when(self.draft.kind == SemanticType::Ordinal, |view| {
                        view.child(
                            Button::new(("semantic-up", index))
                                .small()
                                .ghost()
                                .label("↑")
                                .disabled(busy || index == 0)
                                .on_click(
                                    cx.listener(move |view, _, _, cx| view.reorder(index, -1, cx)),
                                ),
                        )
                        .child(
                            Button::new(("semantic-down", index))
                                .small()
                                .ghost()
                                .label("↓")
                                .disabled(busy)
                                .on_click(
                                    cx.listener(move |view, _, _, cx| view.reorder(index, 1, cx)),
                                ),
                        )
                    }),
            );
        }
        if self.draft.values.len() > 20 {
            view = view.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("semantic-previous")
                            .small()
                            .ghost()
                            .label("上一页")
                            .disabled(busy || self.page == 0)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.page = view.page.saturating_sub(1);
                                cx.notify();
                            })),
                    )
                    .child(div().text_xs().child(format!(
                        "{} 个取值 · 第 {} 页",
                        self.draft.values.len(),
                        self.page + 1
                    )))
                    .child(
                        Button::new("semantic-next")
                            .small()
                            .ghost()
                            .label("下一页")
                            .disabled(busy || start + 20 >= self.draft.values.len())
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.page += 1;
                                cx.notify();
                            })),
                    ),
            );
        }
        if self.draft.kind == SemanticType::Binary {
            let values = self
                .draft
                .values
                .iter()
                .map(|value| value.value.clone())
                .collect::<Vec<_>>();
            let owner = cx.entity().downgrade();
            view = view.child(
                Button::new("positive-value")
                    .small()
                    .ghost()
                    .label(
                        self.draft
                            .positive_value
                            .clone()
                            .unwrap_or_else(|| "选择正值".into()),
                    )
                    .disabled(busy)
                    .dropdown_menu(move |mut menu, _, _| {
                        for value in &values {
                            let owner = owner.clone();
                            let value = value.clone();
                            menu = menu.item(PopupMenuItem::new(value.clone()).on_click(
                                move |_, _, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.draft.positive_value = Some(value.clone());
                                        cx.notify();
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            );
        }
        view.when(busy, |view| {
            view.child(div().text_xs().child(if self.loading {
                "正在读取整列取值…"
            } else {
                "正在提交…"
            }))
        })
        .when_some(self.error.clone(), |view, error| {
            view.child(div().text_xs().text_color(cx.theme().danger).child(error))
        })
    }
}
