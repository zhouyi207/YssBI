//! Shared parameter drafts emit edits; their host owns validation and submission.
mod domain;
mod field;
mod relational;
mod reveal;
#[cfg(test)]
mod tests;

use super::graph_properties::GraphProperties;
use field::{ParameterDraft, ParameterField};
use gpui_kit::{App, Context, Entity, EventEmitter, Render, Subscription, Window};
use std::collections::BTreeMap;
use yss_graph_editor::projection::{
    EditorDiagnosticModel, EditorParameterDisplay, EditorParameterGroupModel,
};
use yss_node_protocol::{ParameterKey, ParameterValues};

pub(super) struct ParameterChange {
    pub key: ParameterKey,
    pub value: serde_json::Value,
    pub base: yss_graph_editor::projection::EditorParameterModel,
}

struct Group {
    display: EditorParameterDisplay,
    fields: std::ops::Range<usize>,
}

pub(super) struct ParameterForm {
    fields: Vec<ParameterField>,
    groups: Vec<Group>,
    diagnostics: Vec<EditorDiagnosticModel>,
    session: Option<yss_project::GraphEditVersion>,
    epoch: u64,
    notifications: yss_ui::notifications::Notifications,
    reveal: Option<reveal::Reveal>,
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
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observer = cx.observe(&properties, |_, _, cx| cx.notify());
        let host_observer = cx.observe(host, |_, _, cx| cx.notify());
        Self {
            fields: vec![],
            groups: vec![],
            diagnostics: vec![],
            session: None,
            epoch: 0,
            notifications: yss_ui::notifications::Notifications::new(window),
            reveal: None,
            can_edit: Box::new(can_edit),
            properties,
            _properties_observer: observer,
            _host_observer: host_observer,
        }
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
        self.reveal = None;
        self.fields.clear();
        self.groups.clear();
        self.diagnostics = diagnostics;
        self.epoch = self.epoch.wrapping_add(1);
        let creation = self.session.is_none();
        for group in groups {
            let start = self.fields.len();
            self.fields.extend(group.parameters.iter().map(|model| {
                if let Some(mut field) = previous.remove(&model.key) {
                    if creation
                        && field.dirty
                        && field
                            .value(cx)
                            .is_ok_and(|value| model.value.as_ref() == Some(&value))
                    {
                        field.dirty = false;
                    }
                    if field.accepts_projection(model) {
                        field.model = model.clone();
                        field.latest = None;
                        field
                    } else if field.dirty {
                        field.latest = Some(model.clone());
                        field
                    } else {
                        ParameterField::new(model.clone(), window, cx)
                    }
                } else {
                    ParameterField::new(model.clone(), window, cx)
                }
            }));
            self.groups.push(Group {
                display: group.display.clone(),
                fields: start..self.fields.len(),
            });
        }
        for (_, field) in previous.into_iter().filter(|(_, field)| field.dirty) {
            let start = self.fields.len();
            self.groups.push(Group {
                display: field.model.display.clone(),
                fields: start..start + 1,
            });
            self.fields.push(field);
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
            self.notifications.push(
                gpui_kit::component::notification::Notification::error(error.clone())
                    .title(field.model.display.title.to_string()),
                cx,
            );
            field.error = Some(error);
            cx.notify();
        }
    }

    /// Include unsubmitted inputs when creating, while leaving unchanged defaults implicit.
    pub fn changes(&mut self, cx: &mut Context<Self>) -> Option<ParameterValues> {
        let mut changes = ParameterValues::new();
        let mut first_error = None;
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
                    first_error.get_or_insert_with(|| {
                        gpui_kit::component::notification::Notification::error(error.clone())
                            .title(field.model.display.title.to_string())
                    });
                    field.error = Some(error);
                }
            }
        }
        cx.notify();
        if let Some(error) = first_error {
            self.notifications.push(error, cx);
            None
        } else {
            Some(changes)
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        self.fields.iter().any(|field| field.dirty)
    }

    pub(crate) fn bind_session(&mut self, version: yss_project::GraphEditVersion) {
        if self
            .session
            .is_some_and(|base| base.session_id != version.session_id)
        {
            for field in self.fields.iter_mut().filter(|field| field.dirty) {
                field.identity_conflict = true;
            }
        }
        self.session = Some(version);
    }

    pub(crate) fn prepare(
        &mut self,
        node: Option<&yss_graph_editor::projection::EditorNodeModel>,
        cx: &mut Context<Self>,
    ) -> Option<ParameterValues> {
        let mut values = ParameterValues::new();
        let mut valid = true;
        for field in self.fields.iter_mut().filter(|field| field.dirty) {
            let current = node
                .into_iter()
                .flat_map(|node| node.parameter_groups.iter())
                .flat_map(|group| group.parameters.iter())
                .find(|model| model.key == field.model.key);
            let value = if !field.identity_conflict
                && current.is_some_and(|model| field.accepts_projection(model))
            {
                field.value(cx)
            } else {
                Err(yss_ui::text::translate(
                    "native.workbench.parameterDraftConflict",
                ))
            };
            match value {
                Ok(value) => {
                    field.error = None;
                    values.insert(field.model.key.clone(), value);
                }
                Err(error) => {
                    field.error = Some(error);
                    valid = false;
                }
            }
        }
        cx.notify();
        valid.then_some(values)
    }

    pub(crate) fn accept(&mut self, values: &ParameterValues, cx: &mut Context<Self>) {
        for field in &mut self.fields {
            if let Some(value) = values.get(&field.model.key)
                && field
                    .value(cx)
                    .as_ref()
                    .is_ok_and(|current| current == value)
            {
                field.dirty = false;
                field.error = None;
            }
        }
        cx.notify();
    }

    fn accepts_input(&self, epoch: u64, cx: &App) -> bool {
        epoch == self.epoch && (self.can_edit)(cx)
    }

    fn apply_parameter(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.fields[index].value(cx) {
            Ok(value) => self.commit_parameter(index, value, cx),
            Err(error) => {
                self.notifications.push(
                    gpui_kit::component::notification::Notification::error(error.clone())
                        .title(self.fields[index].model.display.title.to_string()),
                    cx,
                );
                self.fields[index].error = Some(error);
                cx.notify();
            }
        }
    }

    fn commit_parameter(&mut self, index: usize, value: serde_json::Value, cx: &mut Context<Self>) {
        if self.fields[index].identity_conflict || self.fields[index].latest.is_some() {
            self.set_error(
                &self.fields[index].model.key.clone(),
                yss_ui::text::translate("native.workbench.parameterDraftConflict"),
                cx,
            );
            return;
        }
        let field = &mut self.fields[index];
        field.error = None;
        if field.model.value.as_ref() != Some(&value) || value.is_null() {
            cx.emit(ParameterChange {
                key: field.model.key.clone(),
                value,
                base: field.model.clone(),
            });
        } else {
            field.dirty = false;
            field.error = None;
        }
        cx.notify();
    }
}

impl EventEmitter<ParameterChange> for ParameterForm {}

impl Render for ParameterForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        self.render_parameters(!(self.can_edit)(cx), cx)
    }
}
