use crate::ipc::error::CommandError;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;
use tokio::sync::{broadcast, oneshot};
use yss_ui_contract::UiEvent;

type Subscriptions = BTreeMap<String, (String, oneshot::Sender<()>)>;

#[derive(Clone, Default)]
pub(crate) struct PresentationChannels {
    subscriptions: Arc<Mutex<Subscriptions>>,
    windows: Arc<Mutex<BTreeSet<String>>>,
}

struct SubscriptionAttachment(Option<Box<dyn FnOnce() + Send>>);
impl Drop for SubscriptionAttachment {
    fn drop(&mut self) {
        if let Some(release) = self.0.take() {
            release();
        }
    }
}

impl PresentationChannels {
    pub(crate) fn subscribe(
        &self,
        window: &tauri::WebviewWindow,
        mut receiver: broadcast::Receiver<UiEvent>,
        release: impl FnOnce() + Send + 'static,
        channel: Channel<UiEvent>,
    ) -> Result<String, CommandError> {
        let attachment = SubscriptionAttachment(Some(Box::new(release)));
        let label = window.label().to_owned();
        if self.windows.lock().unwrap().insert(label.clone()) {
            let owner = self.clone();
            let label = label.clone();
            window.on_window_event(move |event| {
                if matches!(event, tauri::WindowEvent::Destroyed) {
                    owner
                        .subscriptions
                        .lock()
                        .unwrap()
                        .retain(|_, (window, _)| window != &label);
                    owner.windows.lock().unwrap().remove(&label);
                }
            });
        }
        let id = uuid::Uuid::new_v4().to_string();
        let (stop, mut stopped) = oneshot::channel();
        {
            let mut subscriptions = self.subscriptions.lock().unwrap();
            if subscriptions.len() >= 64 {
                return Err(CommandError::expected("ui_capacity_exceeded"));
            }
            subscriptions.insert(id.clone(), (label, stop));
        }
        let owner = self.clone();
        let task_id = id.clone();
        tauri::async_runtime::spawn(async move {
            let _attachment = attachment;
            if channel.send(UiEvent::Resync).is_ok() {
                loop {
                    let event = tokio::select! {
                        _ = &mut stopped => break,
                        event = receiver.recv() => match event {
                            Ok(event) => event,
                            Err(broadcast::error::RecvError::Lagged(_)) => UiEvent::Resync,
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    };
                    let ended = matches!(&event, UiEvent::SessionChanged);
                    if channel.send(event).is_err() || ended {
                        break;
                    }
                }
            }
            owner.subscriptions.lock().unwrap().remove(&task_id);
        });
        Ok(id)
    }

    pub(crate) fn unsubscribe(&self, window: &str, id: &str) {
        let mut subscriptions = self.subscriptions.lock().unwrap();
        if subscriptions
            .get(id)
            .is_some_and(|(owner, _)| owner == window)
        {
            subscriptions.remove(id);
        }
    }
}
