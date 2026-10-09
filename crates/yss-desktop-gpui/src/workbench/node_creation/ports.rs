use super::*;
use gpui_component::input::{InputEvent, InputState};
use yss_node_catalog::PortCountPolicy;
use yss_node_protocol::PortKey;

pub(super) struct PortCount {
    pub key: PortKey,
    pub title: String,
    pub policy: PortCountPolicy,
    pub baseline: Option<u16>,
    pub input: Option<Entity<InputState>>,
    pub error: Option<String>,
    _subscription: Option<Subscription>,
}

impl NodeCreationView {
    pub(super) fn install_ports(
        &mut self,
        form: &NodeCreationForm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut previous = std::mem::take(&mut self.ports)
            .into_iter()
            .map(|port| (port.key.clone(), port))
            .collect::<std::collections::BTreeMap<_, _>>();
        for port in &form.ports {
            if let PortCountPolicy::Configurable {
                member_templates, ..
            } = &port.count
                && member_templates.first() != Some(&port.key)
            {
                continue;
            }
            let title = match &port.count {
                PortCountPolicy::Configurable {
                    member_templates, ..
                } => member_templates
                    .iter()
                    .map(|key| {
                        form.ports
                            .iter()
                            .find(|port| &port.key == key)
                            .map_or(key.as_str(), |port| port.title.as_ref())
                    })
                    .collect::<Vec<_>>()
                    .join(" / "),
                _ => port.title.to_string(),
            };
            let count = form.port_counts.get(&port.key).copied();
            let field = if let Some(mut current) = previous
                .remove(&port.key)
                .filter(|current| current.policy == port.count && current.baseline == count)
            {
                current.title = title;
                current
            } else {
                PortCount::new(port, title, count, window, cx)
            };
            self.ports.push(field);
        }
    }

    pub(super) fn port_counts(&mut self, cx: &mut Context<Self>) -> Option<InitialPortCounts> {
        let mut values = self.form.as_ref()?.port_counts.clone();
        let mut valid = true;
        for port in &mut self.ports {
            let PortCountPolicy::Configurable {
                min,
                max,
                member_templates,
            } = &port.policy
            else {
                continue;
            };
            let value = port
                .input
                .as_ref()
                .expect("configurable port input")
                .read(cx)
                .value();
            let count = value
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|count| count >= min && max.is_none_or(|max| *count <= max));
            if let Some(count) = count {
                for key in member_templates {
                    values.insert(key.clone(), count);
                }
                port.error = None;
            } else {
                valid = false;
                port.error = Some(crate::text::format(
                    "native.workbench.portCountRange",
                    &[
                        ("min", min.to_string()),
                        ("max", max.unwrap_or(u16::MAX).to_string()),
                    ],
                ));
            }
        }
        cx.notify();
        valid.then_some(values)
    }
}

impl PortCount {
    fn new(
        port: &yss_node_catalog::NodeCreationPort,
        title: String,
        count: Option<u16>,
        window: &mut Window,
        cx: &mut Context<NodeCreationView>,
    ) -> Self {
        let input = matches!(port.count, PortCountPolicy::Configurable { .. }).then(|| {
            cx.new(|cx| {
                InputState::new(window, cx).default_value(count.unwrap_or_default().to_string())
            })
        });
        let subscription = input.as_ref().map(|input| {
            let id = input.entity_id();
            cx.subscribe_in(input, window, move |view, _, event, window, cx| {
                if !view.can_edit(cx) {
                    return;
                }
                let Some(port) = view.ports.iter_mut().find(|port| {
                    port.input
                        .as_ref()
                        .is_some_and(|input| input.entity_id() == id)
                }) else {
                    return;
                };
                match event {
                    InputEvent::Change => {
                        port.error = None;
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => {
                        if let Some(counts) = view.port_counts(cx)
                            && let Some(form) = &view.form
                        {
                            view.query(
                                Preparation {
                                    values: form.values.clone(),
                                    counts,
                                    ..Default::default()
                                },
                                window,
                                cx,
                            );
                        }
                    }
                    _ => {}
                }
            })
        });
        Self {
            key: port.key.clone(),
            title,
            policy: port.count.clone(),
            baseline: count,
            input,
            error: None,
            _subscription: subscription,
        }
    }
}
