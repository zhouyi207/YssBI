//! Dialog-only semantic drafts use complete, revision-bound column values.
use super::{DatabaseEditor, DatabaseEvent};
mod fields;
mod inputs;

use gpui::{AppContext, Context, Entity, WeakEntity, Window, prelude::*};
use gpui_component::{WindowExt, dialog::DialogButtonProps, input::InputState};
use inputs::MappingInputs;
use std::collections::{BTreeMap, HashSet};
use yss_application::database::DatabaseMutation;
use yss_data_contract::{ColumnSemantic, ConversionDomain, SemanticType, SemanticValue};
use yss_database_schema::DatabaseColumnFact;
use yss_project_identity::OperationId;

struct SemanticDialog {
    owner: WeakEntity<DatabaseEditor>,
    revision: yss_project_identity::ResourceRevision,
    column: String,
    draft: ColumnSemantic,
    inputs: BTreeMap<usize, MappingInputs>,
    minimum: Entity<InputState>,
    maximum: Entity<InputState>,
    page: usize,
    loading: bool,
    values_ready: bool,
    saving: bool,
    error: Option<&'static str>,
}
impl DatabaseEditor {
    pub(super) fn semantic_dialog(
        &self,
        column: &DatabaseColumnFact,
        kind: SemanticType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut draft = ColumnSemantic::new(kind);
        if let Some(previous) = column.semantic() {
            if matches!(
                kind,
                SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
            ) {
                draft.values = previous.values.clone();
            }
            if kind == SemanticType::Binary {
                draft.positive_value = previous.positive_value.clone();
            }
            if kind == SemanticType::Numeric {
                draft.numeric = previous.numeric.clone();
            }
        }
        let owner = cx.entity().downgrade();
        let revision = self.revision;
        let name = column.name().as_str().to_owned();
        let numeric = draft.numeric.clone().unwrap_or_default();
        let editor = cx.new(|cx| SemanticDialog {
            owner,
            revision,
            column: name,
            draft,
            inputs: BTreeMap::new(),
            page: 0,
            minimum: cx.new(|cx| {
                InputState::new(window, cx).default_value(numeric.minimum.unwrap_or_default())
            }),
            maximum: cx.new(|cx| {
                InputState::new(window, cx).default_value(numeric.maximum.unwrap_or_default())
            }),
            loading: false,
            values_ready: !matches!(
                kind,
                SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
            ),
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
            let cancel = editor.clone();
            dialog
                .title(crate::text::translate("detail.data.confirmSemanticTitle"))
                .child(editor.clone())
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(crate::text::translate("common.confirm"))
                        .cancel_text(crate::text::translate("common.cancel"))
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    save.update(cx, |editor, cx| editor.confirm(window, cx));
                    false
                })
                .on_cancel(move |_, _, cx| !cancel.read(cx).saving)
        });
    }
}
impl SemanticDialog {
    fn domain(&self) -> bool {
        matches!(
            self.draft.kind,
            SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
        )
    }
    fn read_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.saving {
            return;
        }
        if !self.current(cx) {
            self.error = Some("detail.data.settingsStale");
            cx.notify();
            return;
        }
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
        self.error = None;
        self.loading = true;
        self.values_ready = false;
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, _, cx| {
                view.loading = false;
                if !view.current(cx) {
                    view.error = Some("detail.data.settingsStale");
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
                    view.error = Some("detail.data.mappingLoadFailed");
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn current(&self, cx: &gpui::App) -> bool {
        self.owner.upgrade().is_some_and(|owner| {
            let owner = owner.read(cx);
            owner.revision == self.revision && owner.ready && !owner.busy()
        })
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading
            || self.saving
            || !self.values_ready
            || self.error == Some("detail.data.settingsStale")
        {
            return;
        }
        if !self.current(cx) {
            self.error = Some("detail.data.settingsStale");
            cx.notify();
            return;
        }
        self.flush_inputs(cx);
        let mut seen = HashSet::new();
        self.error = if self.draft.kind == SemanticType::Binary && self.draft.values.len() != 2 {
            Some("detail.data.binaryCount")
        } else if self.draft.values.len() > ConversionDomain::MAX_VALUES {
            Some("detail.data.tooManyValues")
        } else if self
            .draft
            .values
            .iter()
            .any(|entry| !seen.insert(entry.value.as_str()))
        {
            Some("conversion.duplicateValues")
        } else {
            None
        };
        if self.error.is_some() {
            cx.notify();
            return;
        }
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let view = owner.read(cx);
        if view
            .meta
            .as_ref()
            .and_then(|meta| {
                meta.columns
                    .iter()
                    .find(|column| column.name().as_str() == self.column)
            })
            .and_then(|column| column.semantic())
            == Some(&self.draft)
        {
            window.close_dialog(cx);
            return;
        }
        let semantic = self.draft.clone();
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
                    dialog.error = Some(if dialog.current(cx) {
                        "detail.data.updateFailed"
                    } else {
                        "detail.data.settingsStale"
                    });
                } else {
                    window.close_dialog(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
