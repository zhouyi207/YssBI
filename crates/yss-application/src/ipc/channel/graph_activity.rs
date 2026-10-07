use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;
use tokio::sync::{broadcast, oneshot};
use yss_ipc_contract::graph_editing::GraphActivityDto;

use crate::graph::editing::GraphActivity;
use crate::ipc::error::CommandError;

type ChannelSubscriptions = BTreeMap<String, (String, oneshot::Sender<()>)>;

#[derive(Clone, Default)]
pub struct GraphActivityChannels {
    sessions: Arc<Mutex<ChannelSubscriptions>>,
    windows: Arc<Mutex<BTreeSet<String>>>,
}

impl GraphActivityChannels {
    pub fn bind_window(&self, window: &tauri::WebviewWindow) {
        let label = window.label().to_owned();
        if !self.windows.lock().unwrap().insert(label.clone()) {
            return;
        }
        let owner = self.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let mut sessions = owner.sessions.lock().unwrap();
                let ids = sessions
                    .iter()
                    .filter(|(_, (window, _))| window == &label)
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>();
                for id in ids {
                    if let Some((_, stop)) = sessions.remove(&id) {
                        let _ = stop.send(());
                    }
                }
                owner.windows.lock().unwrap().remove(&label);
            }
        });
    }

    pub fn subscribe(
        &self,
        window: String,
        project_instance_id: String,
        mut receiver: broadcast::Receiver<GraphActivity>,
        subscription: crate::graph::editing::GraphActivitySubscription,
        channel: Channel<GraphActivityDto>,
    ) -> Result<String, CommandError> {
        let id = uuid::Uuid::new_v4().to_string();
        let (stop, mut stopped) = oneshot::channel();
        {
            let mut sessions = self.sessions.lock().unwrap();
            if sessions.len() >= 64 {
                return Err(CommandError::expected("graph_subscription_limit"));
            }
            sessions.insert(id.clone(), (window, stop));
        }
        let sessions = self.sessions.clone();
        let task_id = id.clone();
        tauri::async_runtime::spawn(async move {
            let _subscription = subscription;
            // The receiver is attached before the ready notice. Clients query snapshots after it.
            if channel
                .send(GraphActivityDto::Resync {
                    project_instance_id: project_instance_id.clone(),
                })
                .is_ok()
            {
                loop {
                    let message = tokio::select! {
                        _ = &mut stopped => break,
                        received = receiver.recv() => match received {
                            Ok(GraphActivity::Changed { graph_path, editing }) => GraphActivityDto::Changed { project_instance_id: project_instance_id.clone(), graph_path, editing: crate::ipc::schema::graph_editing::graph_editing_state_to_transport(&editing) },
                            Ok(GraphActivity::Execution(event)) => match super::execution::execution_event_to_transport(event) {
                                Ok(event) => GraphActivityDto::Execution { project_instance_id: project_instance_id.clone(), event },
                                Err(_) => GraphActivityDto::Resync { project_instance_id: project_instance_id.clone() },
                            },
                            Err(broadcast::error::RecvError::Lagged(_)) => GraphActivityDto::Resync { project_instance_id: project_instance_id.clone() },
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    };
                    if channel.send(message).is_err() {
                        break;
                    }
                }
            }
            sessions.lock().unwrap().remove(&task_id);
        });
        Ok(id)
    }

    pub fn unsubscribe(&self, window: &str, id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.get(id).is_some_and(|(owner, _)| owner == window)
            && let Some((_, stop)) = sessions.remove(id)
        {
            let _ = stop.send(());
        }
    }
}
