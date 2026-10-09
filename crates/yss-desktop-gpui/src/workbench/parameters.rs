//! Shared parameter drafts emit edits; their host owns validation and submission.
mod domain;
mod field;
mod relational;

use super::{controls, graph_properties::GraphProperties};
use field::{ParameterDraft, ParameterField};
use gpui::{App, Context, Entity, EventEmitter, Render, Subscription, Window};
use std::collections::BTreeMap;
use yss_graph_editor::projection::{
    EditorDiagnosticModel, EditorParameterDisplay, EditorParameterGroupModel,
};
use yss_node_protocol::{ParameterKey, ParameterValues};

pub(super) struct ParameterChange {
    pub key: ParameterKey,
    pub value: serde_json::Value,
}

struct Group {
    display: EditorParameterDisplay,
    fields: std::ops::Range<usize>,
}

pub(super) struct ParameterForm {
    fields: Vec<ParameterField>,
    groups: Vec<Group>,
    diagnostics: Vec<EditorDiagnosticModel>,
    epoch: u64,
    can_edit: Box<dyn Fn(&App) -> bool>,
    properties: Entity<GraphProperties>,
    _properties_observer: Subscription,
    _host_observer: Subscription,
}

impl ParameterForm {
    pub fn new<T: 'static>(
        properties: Entity<GraphProperties>,
        host: &Entity<T>,
        can_edit: impl Fn(&App) -> bool + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let observer = cx.observe(&properties, |_, _, cx| cx.notify());
        let host_observer = cx.observe(host, |_, _, cx| cx.notify());
        Self {
            fields: vec![],
            groups: vec![],
            diagnostics: vec![],
            epoch: 0,
            can_edit: Box::new(can_edit),
            properties,
            _properties_observer: observer,
            _host_observer: host_observer,
        }
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.fields.clear();
        self.groups.clear();
        self.diagnostics.clear();
        self.epoch = self.epoch.wrapping_add(1);
        cx.notify();
    }

    pub fn install(
        &mut self,
        groups: &[EditorParameterGroupModel],
        diagnostics: Vec<EditorDiagnosticModel>,
        preserve: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut previous = if preserve {
            std::mem::take(&mut self.fields)
                .into_iter()
                .map(|field| (field.model.key.clone(), field))
                .collect::<BTreeMap<_, _>>()
        } else {
            BTreeMap::new()
        };
        self.fields.clear();
        self.groups.clear();
        self.diagnostics = diagnostics;
        self.epoch = self.epoch.wrapping_add(1);
        for group in groups {
            let start = self.fields.len();
            self.fields.extend(group.parameters.iter().map(|model| {
                if let Some(mut field) = previous
                    .remove(&model.key)
                    .filter(|field| field.accepts_projection(model))
                {
                    field.model = model.clone();
                    field
                } else {
                    ParameterField::new(model.clone(), window, cx)
                }
            }));
            self.groups.push(Group {
                display: group.display.clone(),
                fields: start..self.fields.len(),
            });
        }
        cx.notify();
    }

    pub fn needs_constants(&self) -> bool {
        self.fields
            .iter()
            .any(|field| matches!(field.draft, ParameterDraft::Constant))
    }

    pub fn set_error(&mut self, key: &ParameterKey, error: String, cx: &mut Context<Self>) {
        if let Some(field) = self.fields.iter_mut().find(|field| &field.model.key == key) {
            field.error = Some(error);
            cx.notify();
        }
    }

    /// Include unsubmitted inputs when creating, while leaving unchanged defaults implicit.
    pub fn changes(&mut self, cx: &mut Context<Self>) -> Option<ParameterValues> {
        let mut changes = ParameterValues::new();
        let mut valid = true;
        for field in &mut self.fields {
            if !field.dirty {
                continue;
            }
            if matches!(
                field.draft,
                ParameterDraft::Toggle | ParameterDraft::Select | ParameterDraft::Constant
            ) {
                continue;
            }
            match field.value(cx) {
                Ok(value) => {
                    field.error = None;
                    if field.model.value.as_ref() != Some(&value) {
                        changes.insert(field.model.key.clone(), value);
                    }
                }
                Err(error) => {
                    field.error = Some(error);
                    valid = false;
                }
            }
        }
        cx.notify();
        valid.then_some(changes)
    }

    fn accepts_input(&self, epoch: u64, cx: &App) -> bool {
        epoch == self.epoch && (self.can_edit)(cx)
    }

    fn apply_parameter(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.fields[index].value(cx) {
            Ok(value) => self.commit_parameter(index, value, cx),
            Err(error) => {
                self.fields[index].error = Some(error);
                cx.notify();
            }
        }
    }

    fn commit_parameter(&mut self, index: usize, value: serde_json::Value, cx: &mut Context<Self>) {
        let field = &mut self.fields[index];
        field.error = None;
        if field.model.value.as_ref() != Some(&value) || value.is_null() {
            cx.emit(ParameterChange {
                key: field.model.key.clone(),
                value,
            });
        }
        cx.notify();
    }
}

impl EventEmitter<ParameterChange> for ParameterForm {}

impl Render for ParameterForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.render_parameters(!(self.can_edit)(cx), cx)
    }
}
