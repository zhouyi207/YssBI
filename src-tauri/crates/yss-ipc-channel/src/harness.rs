use std::collections::BTreeMap;
use std::sync::Mutex;
use tauri::ipc::Channel;
use yss_harness_contract::{
    HarnessEventEnvelope, HarnessEventSinkPort, HarnessSessionId, PersistenceFailure,
    PersistenceFuture,
};
use yss_ipc_contract::harness::HarnessEventDto;

#[derive(Default)]
pub struct HarnessChannelHub {
    subscriptions: Mutex<BTreeMap<String, HarnessSubscription>>,
}

struct HarnessSubscription {
    session_id: HarnessSessionId,
    channel: Channel<HarnessEventDto>,
    last_sequence: u64,
    replaying: bool,
    pending: BTreeMap<u64, HarnessEventDto>,
}

const MAX_PENDING_HARNESS_EVENTS: usize = 256;

impl HarnessSubscription {
    fn enqueue(&mut self, event: HarnessEventDto) -> bool {
        if event.sequence > self.last_sequence {
            self.pending.insert(event.sequence, event);
        }
        if !self.flush() {
            return false;
        }
        if self.pending.len() > MAX_PENDING_HARNESS_EVENTS {
            // A durable event beyond the gap makes the client request replay. Do not keep
            // accumulating an unbounded live queue behind a missing sequence.
            if let Some((_, event)) = self.pending.pop_last() {
                let _ = self.channel.send(event);
            }
            return false;
        }
        true
    }

    fn flush(&mut self) -> bool {
        if self.replaying {
            return true;
        }
        while let Some(sequence) = self.last_sequence.checked_add(1) {
            let Some(event) = self.pending.remove(&sequence) else {
                break;
            };
            if self.channel.send(event).is_err() {
                return false;
            }
            self.last_sequence = sequence;
        }
        true
    }
}

impl HarnessChannelHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(
        &self,
        session_id: HarnessSessionId,
        channel: Channel<HarnessEventDto>,
        after_sequence: u64,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.subscriptions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                id.clone(),
                HarnessSubscription {
                    session_id,
                    channel,
                    last_sequence: after_sequence,
                    replaying: true,
                    pending: BTreeMap::new(),
                },
            );
        id
    }

    pub fn complete_replay(
        &self,
        subscription_id: &str,
        events: Vec<HarnessEventEnvelope>,
    ) -> bool {
        let mut subscriptions = self
            .subscriptions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(subscription) = subscriptions.get_mut(subscription_id) else {
            return false;
        };
        subscription.replaying = false;
        for event in events {
            if !subscription.enqueue(HarnessEventDto::from(&event)) {
                subscriptions.remove(subscription_id);
                return false;
            }
        }
        if subscription.flush() {
            true
        } else {
            subscriptions.remove(subscription_id);
            false
        }
    }

    pub fn unsubscribe(&self, subscription_id: &str) -> bool {
        self.subscriptions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(subscription_id)
            .is_some()
    }
}

impl HarnessEventSinkPort for HarnessChannelHub {
    fn publish<'a>(
        &'a self,
        event: &'a HarnessEventEnvelope,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let dto = HarnessEventDto::from(event);
            self.subscriptions
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .retain(|_, subscription| {
                    subscription.session_id != event.session_id || subscription.enqueue(dto.clone())
                });
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use yss_harness_contract::{HarnessEvent, UnixMillis};

    #[test]
    fn live_events_wait_for_replay_and_are_delivered_once_in_sequence() {
        let delivered = Arc::new(Mutex::new(Vec::<u64>::new()));
        let received = delivered.clone();
        let channel = Channel::new(move |body| {
            let tauri::ipc::InvokeResponseBody::Json(body) = body else {
                panic!("expected JSON")
            };
            received.lock().unwrap().push(
                serde_json::from_str::<serde_json::Value>(&body).unwrap()["sequence"]
                    .as_u64()
                    .unwrap(),
            );
            Ok(())
        });
        let session = HarnessSessionId::try_new("session-1").unwrap();
        let event = |sequence| HarnessEventEnvelope {
            sequence,
            session_id: session.clone(),
            turn_id: None,
            occurred_at: UnixMillis::from_existing(1000),
            event: HarnessEvent::SessionCreated,
        };
        let hub = HarnessChannelHub::new();
        let id = hub.subscribe(session.clone(), channel.clone(), 0);
        tauri::async_runtime::block_on(hub.publish(&event(3))).unwrap();
        assert!(delivered.lock().unwrap().is_empty());
        assert!(hub.complete_replay(&id, vec![event(1), event(2), event(3)]));
        tauri::async_runtime::block_on(hub.publish(&event(3))).unwrap();
        tauri::async_runtime::block_on(hub.publish(&event(5))).unwrap();
        tauri::async_runtime::block_on(hub.publish(&event(4))).unwrap();
        assert_eq!(*delivered.lock().unwrap(), [1, 2, 3, 4, 5]);
        for sequence in 1000..=1000 + MAX_PENDING_HARNESS_EVENTS as u64 {
            tauri::async_runtime::block_on(hub.publish(&event(sequence))).unwrap();
        }
        assert_eq!(
            delivered.lock().unwrap().last().copied(),
            Some(1000 + MAX_PENDING_HARNESS_EVENTS as u64)
        );
        assert!(!hub.unsubscribe(&id));
        delivered.lock().unwrap().clear();
        let id = hub.subscribe(session.clone(), channel, 0);
        let history = (1..=MAX_PENDING_HARNESS_EVENTS as u64 * 2)
            .map(event)
            .collect();
        assert!(hub.complete_replay(&id, history));
        assert_eq!(
            delivered.lock().unwrap().len(),
            MAX_PENDING_HARNESS_EVENTS * 2
        );
    }
}
