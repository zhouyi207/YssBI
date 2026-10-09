use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use yss_plugin_protocol::{PluginFailure, ResourceBudget, RpcRequest, RpcResponse, read_frame};

pub type Handler = Arc<dyn Fn(&Peer, RpcRequest) -> Result<Value, PluginFailure> + Send + Sync>;
type Pending = BTreeMap<String, mpsc::SyncSender<Result<Value, PluginFailure>>>;

pub struct Peer {
    outgoing: mpsc::SyncSender<Vec<u8>>,
    outgoing_bytes: Arc<AtomicUsize>,
    pending: Mutex<Option<Pending>>,
    next: AtomicU64,
    prefix: String,
    budget: Mutex<ResourceBudget>,
}

impl Peer {
    pub fn connect(
        input: impl Read + Send + 'static,
        output: impl Write + Send + 'static,
        budget: ResourceBudget,
        handler: Handler,
    ) -> Arc<Self> {
        let (outgoing, outgoing_receiver) = mpsc::sync_channel::<Vec<u8>>(32);
        let outgoing_bytes = Arc::new(AtomicUsize::new(0));
        let peer = Arc::new(Self {
            outgoing,
            outgoing_bytes: outgoing_bytes.clone(),
            pending: Mutex::new(Some(BTreeMap::new())),
            next: AtomicU64::new(1),
            prefix: uuid::Uuid::new_v4().to_string(),
            budget: Mutex::new(budget),
        });
        let weak_writer = Arc::downgrade(&peer);
        std::thread::spawn(move || {
            let mut output = output;
            while let Ok(bytes) = outgoing_receiver.recv() {
                let result = output
                    .write_all(&(bytes.len() as u32).to_be_bytes())
                    .and_then(|()| output.write_all(&bytes))
                    .and_then(|()| output.flush());
                outgoing_bytes.fetch_sub(bytes.len(), Ordering::AcqRel);
                if result.is_err() {
                    if let Some(peer) = weak_writer.upgrade() {
                        peer.close();
                    }
                    return;
                }
            }
        });
        let (sender, receiver) = mpsc::sync_channel::<RpcRequest>(16);
        let (control_sender, control_receiver) = mpsc::sync_channel::<RpcRequest>(8);
        for (receiver, count) in [(receiver, 4), (control_receiver, 2)] {
            let receiver = Arc::new(Mutex::new(receiver));
            for _ in 0..count {
                let weak = Arc::downgrade(&peer);
                let receiver = receiver.clone();
                let handler = handler.clone();
                std::thread::spawn(move || {
                    loop {
                        let request = match receiver.lock() {
                            Ok(receiver) => receiver.recv(),
                            Err(_) => return,
                        };
                        let Ok(request) = request else {
                            return;
                        };
                        let Some(peer) = weak.upgrade().filter(|peer| peer.is_alive()) else {
                            return;
                        };
                        let id = request.id.clone();
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            handler(&peer, request)
                        }))
                        .unwrap_or_else(|_| Err(PluginFailure::new("plugin_handler_failed")));
                        if peer.send(&RpcResponse::from_result(id, result)).is_err() {
                            peer.close();
                        }
                    }
                });
            }
        }
        let weak = Arc::downgrade(&peer);
        std::thread::spawn(move || {
            let mut input = input;
            while let Ok(Some(frame)) = read_frame(&mut input) {
                let Some(peer) = weak.upgrade().filter(|peer| peer.is_alive()) else {
                    break;
                };
                if frame.len() > peer.budget().frame_bytes as usize {
                    break;
                }
                let Ok(value) = serde_json::from_slice::<Value>(&frame) else {
                    break;
                };
                if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
                    break;
                }
                if value.get("method").is_some() {
                    let Ok(request) = serde_json::from_value::<RpcRequest>(value) else {
                        break;
                    };
                    if request.id.is_empty() || request.id.len() > 128 || request.method.len() > 128
                    {
                        break;
                    }
                    let queue = if matches!(
                        request.method.as_str(),
                        "lifecycle.initialize"
                            | "lifecycle.shutdown"
                            | "tasks.cancel"
                            | "tasks.get"
                            | "dependencies.inspect"
                    ) {
                        &control_sender
                    } else {
                        &sender
                    };
                    if let Err(error) = queue.try_send(request) {
                        let request = match error {
                            mpsc::TrySendError::Full(request)
                            | mpsc::TrySendError::Disconnected(request) => request,
                        };
                        if peer
                            .send(&RpcResponse::from_result(
                                request.id,
                                Err(PluginFailure::new("plugin_resource_exhausted")),
                            ))
                            .is_err()
                        {
                            peer.close();
                        }
                    }
                } else {
                    let Ok(response) = serde_json::from_value::<RpcResponse>(value) else {
                        break;
                    };
                    if response.result.is_some() == response.error.is_some() {
                        break;
                    }
                    let pending = peer
                        .pending
                        .lock()
                        .ok()
                        .and_then(|mut pending| pending.as_mut()?.remove(&response.id));
                    if let Some(pending) = pending {
                        let _ = pending.send(match response.error {
                            Some(error) => Err(error.data),
                            None => Ok(response.result.unwrap_or(Value::Null)),
                        });
                    } else {
                        break;
                    }
                }
            }
            if let Some(peer) = weak.upgrade() {
                peer.close();
            }
        });
        peer
    }

    pub fn call(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, PluginFailure> {
        let id = format!(
            "{}:{}",
            self.prefix,
            self.next.fetch_add(1, Ordering::Relaxed)
        );
        let (sender, receiver) = mpsc::sync_channel(1);
        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| PluginFailure::new("plugin_state_unavailable"))?;
            let pending = pending
                .as_mut()
                .ok_or_else(|| PluginFailure::new("plugin_process_exited"))?;
            if pending.len() >= self.budget().pending_requests as usize {
                return Err(PluginFailure::new("plugin_resource_exhausted"));
            }
            pending.insert(id.clone(), sender);
        }
        let request = RpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: method.to_owned(),
            params,
        };
        let result = self.send(&request);
        if let Err(error) = result {
            // No frame was queued: retire only this call's correlation.
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(pending) = pending.as_mut() {
                pending.remove(&request.id);
            }
            return Err(error);
        }
        match receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(_) => {
                self.close();
                Err(PluginFailure::new("plugin_request_timeout"))
            }
        }
    }
    fn send(&self, value: &impl serde::Serialize) -> Result<(), PluginFailure> {
        let bytes =
            serde_json::to_vec(value).map_err(|_| PluginFailure::new("plugin_response_invalid"))?;
        let budget = self.budget();
        if bytes.len() > budget.frame_bytes as usize {
            return Err(PluginFailure::new("plugin_payload_too_large"));
        }
        self.outgoing_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |queued| {
                queued
                    .checked_add(bytes.len())
                    .filter(|next| *next <= budget.queued_bytes as usize)
            })
            .map_err(|_| PluginFailure::new("plugin_resource_exhausted"))?;
        let size = bytes.len();
        match self.outgoing.try_send(bytes) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => {
                self.outgoing_bytes.fetch_sub(size, Ordering::AcqRel);
                Err(PluginFailure::new("plugin_resource_exhausted"))
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.outgoing_bytes.fetch_sub(size, Ordering::AcqRel);
                self.close();
                Err(PluginFailure::new("plugin_process_exited"))
            }
        }
    }
    pub fn is_alive(&self) -> bool {
        self.pending.lock().is_ok_and(|pending| pending.is_some())
    }
    pub fn budget(&self) -> ResourceBudget {
        self.budget
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
    pub fn apply_budget(&self, granted: ResourceBudget) -> Result<(), PluginFailure> {
        granted.validate()?;
        let mut current = self
            .budget
            .lock()
            .map_err(|_| PluginFailure::new("plugin_state_unavailable"))?;
        if granted.frame_bytes > current.frame_bytes
            || granted.pending_requests > current.pending_requests
            || granted.queued_bytes > current.queued_bytes
            || granted.active_tasks > current.active_tasks
            || granted.snapshot_bytes > current.snapshot_bytes
            || granted.private_storage_bytes > current.private_storage_bytes
            || granted.views > current.views
        {
            return Err(PluginFailure::new("plugin_budget_invalid"));
        }
        *current = granted;
        Ok(())
    }
    pub fn pending_count(&self) -> usize {
        self.pending.lock().map_or(usize::MAX, |pending| {
            pending.as_ref().map_or(0, BTreeMap::len)
        })
    }
    pub fn close(&self) {
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(pending) = pending {
            for (_, sender) in pending {
                let _ = sender.send(Err(PluginFailure::new("plugin_process_exited")));
            }
        }
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(budget: ResourceBudget) -> (Arc<Peer>, mpsc::Receiver<Vec<u8>>) {
        budget.validate().unwrap();
        let (outgoing, frames) = mpsc::sync_channel(32);
        let peer = Arc::new(Peer {
            outgoing,
            outgoing_bytes: Arc::new(AtomicUsize::new(0)),
            pending: Mutex::new(Some(BTreeMap::new())),
            next: AtomicU64::new(1),
            prefix: "test".into(),
            budget: Mutex::new(budget),
        });
        (peer, frames)
    }

    fn waiting_call(peer: &Arc<Peer>) -> std::thread::JoinHandle<Result<Value, PluginFailure>> {
        let caller = peer.clone();
        std::thread::spawn(move || caller.call("tasks.get", Value::Null, Duration::from_secs(5)))
    }

    fn local_rejection_preserves_calls(params: Value, expected: &str, queued_bytes: u32) {
        let (peer, frames) = fixture(ResourceBudget {
            frame_bytes: 1024,
            queued_bytes,
            ..ResourceBudget::default()
        });
        let waiting = waiting_call(&peer);
        let first = frames.recv_timeout(Duration::from_secs(5)).unwrap();
        let first_request: RpcRequest = serde_json::from_slice(&first).unwrap();
        let rejected = peer
            .call("tasks.get", params, Duration::from_secs(5))
            .unwrap_err();
        let remained_alive = peer.is_alive();
        let remaining = peer.pending_count();
        let continued = waiting_call(&peer);
        let next = remained_alive.then(|| frames.recv_timeout(Duration::from_secs(5)).unwrap());
        peer.close();
        assert_eq!(
            waiting.join().unwrap().unwrap_err().code,
            "plugin_process_exited"
        );
        assert_eq!(
            continued.join().unwrap().unwrap_err().code,
            "plugin_process_exited"
        );
        assert_eq!(rejected.code, expected);
        assert!(
            remained_alive,
            "a local admission rejection closed the connection"
        );
        assert_eq!(remaining, 1, "only the rejected request should retire");
        let next: RpcRequest = serde_json::from_slice(&next.unwrap()).unwrap();
        assert_eq!(next.method, "tasks.get");
        assert_ne!(next.id, first_request.id);
        assert!(matches!(frames.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[test]
    fn oversized_request_is_a_local_admission_rejection() {
        local_rejection_preserves_calls(
            Value::String("x".repeat(1024)),
            "plugin_payload_too_large",
            4096,
        );
    }

    #[test]
    fn queued_byte_limit_is_a_local_admission_rejection() {
        local_rejection_preserves_calls(
            Value::String("x".repeat(924)),
            "plugin_resource_exhausted",
            1024,
        );
    }

    #[test]
    fn disconnected_output_is_a_connection_failure() {
        let (peer, frames) = fixture(ResourceBudget::default());
        let waiting = waiting_call(&peer);
        frames.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(frames);
        let failed = peer
            .call("tasks.get", Value::Null, Duration::from_secs(5))
            .unwrap_err();
        let closed = !peer.is_alive();
        let remaining = peer.pending_count();
        peer.close();
        assert_eq!(
            waiting.join().unwrap().unwrap_err().code,
            "plugin_process_exited"
        );
        assert_eq!(failed.code, "plugin_process_exited");
        assert!(closed);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn close_retires_admitted_requests_and_rejects_later_calls() {
        let (peer, frames) = fixture(ResourceBudget::default());
        let caller = peer.clone();
        let request = std::thread::spawn(move || {
            caller.call("tasks.get", Value::Null, Duration::from_secs(5))
        });
        let frame = frames.recv_timeout(Duration::from_secs(5)).unwrap();
        let sent: RpcRequest = serde_json::from_slice(&frame).unwrap();
        assert_eq!(sent.method, "tasks.get");
        assert_eq!(peer.pending_count(), 1);

        peer.close();
        assert!(!peer.is_alive());
        assert_eq!(peer.pending_count(), 0);
        assert_eq!(
            request.join().unwrap().unwrap_err().code,
            "plugin_process_exited"
        );
        assert_eq!(
            peer.call("tasks.get", Value::Null, Duration::ZERO)
                .unwrap_err()
                .code,
            "plugin_process_exited"
        );
        assert!(matches!(frames.try_recv(), Err(mpsc::TryRecvError::Empty)));
        peer.close();
        assert_eq!(peer.pending_count(), 0);
    }

    #[test]
    fn timeout_closes_admission_and_wakes_other_pending_calls() {
        let (peer, frames) = fixture(ResourceBudget::default());
        let caller = peer.clone();
        let waiting = std::thread::spawn(move || {
            caller.call("tasks.get", Value::Null, Duration::from_secs(5))
        });
        frames.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(peer.pending_count(), 1);

        let timeout = peer
            .call("tasks.cancel", Value::Null, Duration::ZERO)
            .unwrap_err();
        assert_eq!(timeout.code, "plugin_request_timeout");
        assert!(!peer.is_alive());
        assert_eq!(peer.pending_count(), 0);
        assert_eq!(
            waiting.join().unwrap().unwrap_err().code,
            "plugin_process_exited"
        );
        assert_eq!(
            peer.call("tasks.get", Value::Null, Duration::ZERO)
                .unwrap_err()
                .code,
            "plugin_process_exited"
        );
    }
}
