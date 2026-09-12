use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
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
    pending: Mutex<Pending>,
    expired: Mutex<VecDeque<String>>,
    alive: AtomicBool,
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
            pending: Mutex::new(BTreeMap::new()),
            expired: Mutex::new(VecDeque::new()),
            alive: AtomicBool::new(true),
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
                        .and_then(|mut pending| pending.remove(&response.id));
                    if let Some(pending) = pending {
                        let _ = pending.send(match response.error {
                            Some(error) => Err(error.data),
                            None => Ok(response.result.unwrap_or(Value::Null)),
                        });
                    } else if !peer
                        .expired
                        .lock()
                        .is_ok_and(|expired| expired.contains(&response.id))
                    {
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
        if !self.is_alive() {
            return Err(PluginFailure::new("plugin_process_exited"));
        }
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
            if pending.len() >= self.budget().pending_requests as usize {
                return Err(PluginFailure::new("plugin_resource_exhausted"));
            }
            pending.insert(id.clone(), sender);
        }
        let request = RpcRequest {
            jsonrpc: "2.0".into(),
            id: id.clone(),
            method: method.to_owned(),
            params,
        };
        let result = self.send(&request);
        if let Err(error) = result {
            self.close();
            return Err(error);
        }
        match receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(_) => {
                if let Ok(mut expired) = self.expired.lock() {
                    expired.push_back(id.clone());
                    while expired.len() > 128 {
                        expired.pop_front();
                    }
                }
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&id);
                }
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
        if self.outgoing.try_send(bytes).is_err() {
            self.outgoing_bytes.fetch_sub(size, Ordering::AcqRel);
            return Err(PluginFailure::new("plugin_resource_exhausted"));
        }
        Ok(())
    }
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
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
        self.pending
            .lock()
            .map_or(usize::MAX, |pending| pending.len())
    }
    pub fn close(&self) {
        self.alive.store(false, Ordering::Release);
        if let Ok(mut pending) = self.pending.lock() {
            for (_, sender) in std::mem::take(&mut *pending) {
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
