//! Relational fields own one draft; availability remains in the semantic projection.
mod columns;
mod predicate;
use super::{ParameterForm, field::list::ListDraft};
use columns::ColumnsDraft;
use gpui_kit::{AnyElement, Context, EntityId, Window};
use predicate::FilterDraft;
use serde_json::Value;
use yss_graph_editor::projection::{EditorParameterConfiguration, EditorParameterModel};

pub(super) enum RelationalDraft {
    Columns(ColumnsDraft),
    Filter(FilterDraft),
}

impl RelationalDraft {
    pub fn new(
        model: &EditorParameterModel,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> Self {
        match model
            .configuration
            .as_ref()
            .expect("relational configuration")
        {
            EditorParameterConfiguration::ProjectColumns {
                schema_known,
                value,
                ..
            } => Self::Columns(ColumnsDraft::new(
                &model.key,
                *schema_known,
                value,
                window,
                cx,
            )),
            EditorParameterConfiguration::FilterPredicate { .. } => {
                Self::Filter(FilterDraft::new(model, window, cx))
            }
            EditorParameterConfiguration::SelectOptions { .. } => {
                unreachable!("select has its own native control")
            }
        }
    }

    pub fn value(
        &self,
        configuration: &EditorParameterConfiguration,
        cx: &gpui_kit::App,
    ) -> Result<Value, String> {
        match (self, configuration) {
            (
                Self::Columns(draft),
                EditorParameterConfiguration::ProjectColumns { allow_empty, .. },
            ) => draft.value(*allow_empty, cx),
            (Self::Filter(draft), configuration) => draft.value(configuration, cx),
            _ => unreachable!("draft follows its parameter configuration"),
        }
    }

    pub fn list_mut(&mut self) -> Option<&mut ListDraft> {
        match self {
            Self::Columns(ColumnsDraft::Manual(draft)) => Some(draft),
            _ => None,
        }
    }

    pub fn owns_input(&self, id: EntityId) -> bool {
        match self {
            Self::Columns(ColumnsDraft::Manual(draft)) => draft.owns_input(id),
            Self::Columns(_) => false,
            Self::Filter(draft) => draft.owns_input(id),
        }
    }
}

impl ParameterForm {
    pub(super) fn render_relational(
        &self,
        index: usize,
        draft: &RelationalDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match draft {
            RelationalDraft::Columns(draft) => self.render_columns(index, draft, busy, cx),
            RelationalDraft::Filter(draft) => self.render_filter(index, draft, busy, cx),
        }
    }
}
