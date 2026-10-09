use super::*;
use yss_node_protocol::ParameterKey;

#[derive(Clone, Default)]
pub(super) struct Preparation {
    pub values: ParameterValues,
    pub counts: InitialPortCounts,
    pub field: Option<ParameterKey>,
    pub create: bool,
}

impl NodeCreationView {
    pub(super) fn query(
        &mut self,
        request: Preparation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.task = None;
        self.preparing = Some(request.clone());
        let Preparation {
            values,
            counts,
            field,
            create,
        } = request;
        self.error = None;
        self.language = crate::text::locale();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let language = self.language;
        let project = self.target.project.clone();
        let node_type = match &self.target.descriptor {
            NodeCreation::Static { node_type_id }
            | NodeCreation::ParameterizedStatic { node_type_id, .. }
            | NodeCreation::ResourceBound { node_type_id, .. } => node_type_id.clone(),
        };
        let task = self.services.run(move |services| {
            Ok(services
                .application
                .node_creation_form(&project, &node_type, values, counts, language)?)
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation || language != crate::text::locale() {
                    return;
                }
                view.preparing = None;
                if !view.current(cx) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(form) => {
                        view.parameters.update(cx, |parameters, cx| {
                            parameters.install(
                                &form.groups,
                                vec![],
                                view.form.is_some(),
                                window,
                                cx,
                            )
                        });
                        view.install_ports(&form, window, cx);
                        let needed = view.parameters.read(cx).needs_constants();
                        view.properties.update(cx, |properties, cx| {
                            properties.set_graph(view.target.graph.clone(), needed, window, cx)
                        });
                        if create {
                            view.creating = true;
                            cx.emit(CreationEvent::Create {
                                parameters: form.values.clone(),
                                port_counts: form.port_counts.clone(),
                            });
                        }
                        view.form = Some(form);
                    }
                    Err(_) => {
                        let message = crate::text::translate("canvas.nodePalette.createFailed");
                        if let Some(key) = &field {
                            view.parameters.update(cx, |parameters, cx| {
                                parameters.set_error(key, message.clone(), cx)
                            });
                        }
                        view.error = Some(message);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn refresh_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.language == crate::text::locale() || self.creating || !self.current(cx) {
            return;
        }
        let request = self.preparing.take().unwrap_or_else(|| Preparation {
            values: self
                .form
                .as_ref()
                .map(|form| form.values.clone())
                .unwrap_or_default(),
            counts: self
                .form
                .as_ref()
                .map(|form| form.port_counts.clone())
                .unwrap_or_default(),
            ..Default::default()
        });
        self.query(request, window, cx);
    }
}
