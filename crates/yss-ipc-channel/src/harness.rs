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
    draining: bool,
    closing: bool,
    pending: BTreeMap<u64, HarnessEventDto>,
}

struct HarnessDelivery {
    channel: Channel<HarnessEventDto>,
    event: HarnessEventDto,
    terminal: bool,
}

const MAX_PENDING_HARNESS_EVENTS: usize = 256;

impl HarnessSubscription {
    fn insert(&mut self, event: HarnessEventDto) {
        if !self.closing && event.sequence > self.last_sequence {
            self.pending.insert(event.sequence, event);
        }
    }

    fn bound_pending(&mut self) {
        if self.pending.len() > MAX_PENDING_HARNESS_EVENTS {
            // The same drainer sends this gap after any in-flight event. Stop accepting
            // events immediately so a blocked Channel cannot retain a growing queue.
            if let Some((sequence, event)) = self.pending.pop_last() {
                self.pending.clear();
                self.pending.insert(sequence, event);
                self.closing = true;
            }
        }
    }

    fn take_delivery(&mut self) -> Option<HarnessDelivery> {
        let event = if self.closing {
            self.pending.pop_last()?.1
        } else {
            if self.replaying {
                return None;
            }
            let sequence = self.last_sequence.checked_add(1)?;
            let event = self.pending.remove(&sequence)?;
            // Reserving the sequence also deduplicates concurrent/reentrant publication
            // while send is in flight. Send failure removes this subscription.
            self.last_sequence = sequence;
            event
        };
        Some(HarnessDelivery {
            channel: self.channel.clone(),
            event,
            terminal: self.closing,
        })
    }

    fn begin_drain(&mut self) -> Option<HarnessDelivery> {
        if self.draining {
            return None;
        }
        let delivery = self.take_delivery()?;
        self.draining = true;
        Some(delivery)
    }

    fn enqueue(&mut self, event: HarnessEventDto) -> Option<HarnessDelivery> {
        self.insert(event);
        // A newly filled gap may make a full queue drainable. Reserve its first event
        // before applying the bound to the remaining, unsent events.
        let delivery = self.begin_drain();
        self.bound_pending();
        delivery.or_else(|| self.begin_drain())
    }

    fn take_replay_delivery(
        &mut self,
        replay: &mut impl Iterator<Item = HarnessEventEnvelope>,
    ) -> Option<HarnessDelivery> {
        let mut delivery = self.take_delivery();
        while delivery.is_none() {
            self.insert(HarnessEventDto::from(&replay.next()?));
            delivery = self.take_delivery();
            self.bound_pending();
            if delivery.is_none() {
                delivery = self.take_delivery();
            }
        }
        delivery
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
                    draining: false,
                    closing: false,
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
        {
            let mut subscriptions = self
                .subscriptions
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(subscription) = subscriptions.get_mut(subscription_id) else {
                return false;
            };
            if subscription.closing || subscription.draining {
                return false;
            }
            subscription.replaying = false;
            subscription.draining = true;
        }
        self.drain(subscription_id, None, events.into_iter())
    }

    fn drain(
        &self,
        subscription_id: &str,
        mut first: Option<HarnessDelivery>,
        mut replay: impl Iterator<Item = HarnessEventEnvelope>,
    ) -> bool {
        loop {
            let delivery = {
                let mut subscriptions = self
                    .subscriptions
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let Some(subscription) = subscriptions.get_mut(subscription_id) else {
                    return false;
                };
                let Some(delivery) = first
                    .take()
                    .or_else(|| subscription.take_replay_delivery(&mut replay))
                else {
                    subscription.draining = false;
                    return true;
                };
                delivery
            };
            if delivery.channel.send(delivery.event).is_err() || delivery.terminal {
                self.unsubscribe(subscription_id);
                return false;
            }
        }
    }

    pub fn unsubscribe(&self, subscription_id: &str) -> bool {
        let removed = {
            self.subscriptions
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .remove(subscription_id)
        };
        // The last Channel can invoke an external on_drop callback during release.
        removed.is_some()
    }
}

impl HarnessEventSinkPort for HarnessChannelHub {
    fn publish<'a>(
        &'a self,
        event: &'a HarnessEventEnvelope,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let dto = HarnessEventDto::from(event);
            let deliveries = {
                let mut subscriptions = self
                    .subscriptions
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                subscriptions
                    .iter_mut()
                    .filter(|(_, subscription)| subscription.session_id == event.session_id)
                    .filter_map(|(id, subscription)| {
                        subscription
                            .enqueue(dto.clone())
                            .map(|delivery| (id.clone(), delivery))
                    })
                    .collect::<Vec<_>>()
            };
            for (id, delivery) in deliveries {
                self.drain(&id, Some(delivery), std::iter::empty());
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;
    use yss_harness_contract::{HarnessEvent, UnixMillis};

    fn event(session_id: &HarnessSessionId, sequence: u64) -> HarnessEventEnvelope {
        HarnessEventEnvelope {
            sequence,
            session_id: session_id.clone(),
            turn_id: None,
            occurred_at: UnixMillis::from_existing(1000),
            event: HarnessEvent::SessionCreated,
        }
    }

    fn delivered_sequence(body: tauri::ipc::InvokeResponseBody) -> u64 {
        let tauri::ipc::InvokeResponseBody::Json(body) = body else {
            panic!("expected JSON")
        };
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["sequence"]
            .as_u64()
            .unwrap()
    }

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

    #[test]
    fn delivery_allows_reentrant_publish_and_unsubscribe_without_duplicates() {
        let hub = Arc::new(HarnessChannelHub::new());
        let session = HarnessSessionId::try_new("reentrant").unwrap();
        let subscription_id = Arc::new(Mutex::new(String::new()));
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let callback_hub = hub.clone();
        let callback_session = session.clone();
        let callback_id = subscription_id.clone();
        let received = delivered.clone();
        let channel = Channel::new(move |body| {
            assert!(callback_hub.subscriptions.try_lock().is_ok());
            let sequence = delivered_sequence(body);
            received.lock().unwrap().push(sequence);
            if sequence == 1 {
                for sequence in [1, 3, 2] {
                    tauri::async_runtime::block_on(
                        callback_hub.publish(&event(&callback_session, sequence)),
                    )
                    .unwrap();
                }
            } else if sequence == 3 {
                assert!(callback_hub.unsubscribe(&callback_id.lock().unwrap()));
                tauri::async_runtime::block_on(callback_hub.publish(&event(&callback_session, 4)))
                    .unwrap();
            }
            Ok(())
        });
        let id = hub.subscribe(session.clone(), channel, 0);
        *subscription_id.lock().unwrap() = id.clone();
        assert!(!hub.complete_replay(&id, vec![event(&session, 1)]));
        assert_eq!(*delivered.lock().unwrap(), [1, 2, 3]);
        assert!(!hub.unsubscribe(&id));
    }

    #[test]
    fn blocked_delivery_bounds_live_events_and_closes_after_one_gap_or_send_failure() {
        let hub = Arc::new(HarnessChannelHub::new());
        let session = HarnessSessionId::try_new("blocked").unwrap();
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let received = delivered.clone();
        let (entered, waiting) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let released = Mutex::new(released);
        let channel = Channel::new(move |body| {
            let sequence = delivered_sequence(body);
            received.lock().unwrap().push(sequence);
            if sequence == 1 {
                entered.send(()).unwrap();
                released
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
            }
            Ok(())
        });
        let id = hub.subscribe(session.clone(), channel, 0);
        let worker_hub = hub.clone();
        let worker_id = id.clone();
        let worker_session = session.clone();
        let worker = std::thread::spawn(move || {
            worker_hub.complete_replay(&worker_id, vec![event(&worker_session, 1)])
        });
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        if hub.subscriptions.try_lock().is_err() {
            release.send(()).unwrap();
            worker.join().unwrap();
            panic!("Channel delivery held the subscriptions lock");
        }
        tauri::async_runtime::block_on(hub.publish(&event(&session, 1))).unwrap();
        for sequence in 1000..=1000 + MAX_PENDING_HARNESS_EVENTS as u64 {
            tauri::async_runtime::block_on(hub.publish(&event(&session, sequence))).unwrap();
        }
        assert_eq!(hub.subscriptions.lock().unwrap()[&id].pending.len(), 1);
        release.send(()).unwrap();
        assert!(!worker.join().unwrap());
        assert_eq!(
            *delivered.lock().unwrap(),
            [1, 1000 + MAX_PENDING_HARNESS_EVENTS as u64]
        );
        assert!(!hub.unsubscribe(&id));

        let failed = hub.subscribe(
            session.clone(),
            Channel::new(|_| Err(std::io::Error::other("injected send failure").into())),
            0,
        );
        assert!(!hub.complete_replay(&failed, vec![event(&session, 1)]));
        assert!(!hub.unsubscribe(&failed));
    }
}
