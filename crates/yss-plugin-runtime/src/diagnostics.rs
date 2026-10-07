use std::collections::{BTreeSet, VecDeque};
use std::sync::{Mutex, PoisonError};
use yss_plugin_protocol::PluginDiagnostic;

const MAX_BYTES: usize = 64 * 1024;

struct Chunk {
    tasks: Vec<String>,
    text: String,
}
#[derive(Default)]
struct Buffer {
    active: BTreeSet<String>,
    chunks: VecDeque<Chunk>,
    bytes: usize,
    truncated: bool,
}
pub(super) struct DiagnosticBuffer {
    pub plugin: String,
    instance: String,
    data: Mutex<Buffer>,
}
impl DiagnosticBuffer {
    pub fn new(plugin: String, instance: String) -> Self {
        Self {
            plugin,
            instance,
            data: Mutex::new(Buffer::default()),
        }
    }
    pub fn task(&self, task: &str, active: bool) {
        let mut data = self.data.lock().unwrap_or_else(PoisonError::into_inner);
        if active {
            data.active.insert(task.into());
        } else {
            data.active.remove(task);
        }
    }
    pub fn push(&self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes)
            .chars()
            .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
            .collect::<String>();
        let mut data = self.data.lock().unwrap_or_else(PoisonError::into_inner);
        let tasks = data.active.iter().cloned().collect();
        data.bytes += text.len();
        data.chunks.push_back(Chunk { tasks, text });
        while data.bytes > MAX_BYTES || data.chunks.len() > 256 {
            if let Some(chunk) = data.chunks.pop_front() {
                data.bytes -= chunk.text.len();
            }
            data.truncated = true;
        }
    }
    pub fn snapshot(&self) -> Vec<PluginDiagnostic> {
        let data = self.data.lock().unwrap_or_else(PoisonError::into_inner);
        data.chunks
            .iter()
            .map(|chunk| PluginDiagnostic {
                plugin_id: self.plugin.clone(),
                instance_id: self.instance.clone(),
                task_ids: chunk.tasks.clone(),
                stderr: chunk.text.clone(),
                truncated: data.truncated,
            })
            .collect()
    }
}

#[cfg(test)]
#[test]
fn diagnostic_ring_retains_recent_output_and_the_task_identity_at_emission() {
    let buffer = DiagnosticBuffer::new("example.plugin".into(), "instance".into());
    buffer.task("first", true);
    for _ in 0..24 {
        buffer.push(&[b'a'; 4096]);
    }
    buffer.task("first", false);
    buffer.task("second", true);
    buffer.push(b"startup failed\n");
    buffer.task("second", false);
    let output = buffer.snapshot();
    assert!(output.iter().map(|entry| entry.stderr.len()).sum::<usize>() <= MAX_BYTES);
    assert!(output.iter().all(|entry| entry.truncated));
    assert_eq!(output.last().unwrap().task_ids, ["second"]);
    assert!(output.last().unwrap().stderr.contains("startup failed"));
}
